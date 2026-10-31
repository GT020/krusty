//! Retry policy for Kubernetes list/watch loops.
//!
//! Hard failures (RBAC denials, missing resource types) are retried a couple
//! of times and then stop the loop instead of hammering the API server.
//! Soft failures (network blips, timeouts, 429, 5xx) retry forever with
//! capped exponential backoff.

use std::time::Duration;

/// Total attempts for a hard failure before the watch loop gives up.
const MAX_HARD_ATTEMPTS: u32 = 2;

/// Delay between attempts for a hard failure.
const HARD_RETRY_DELAY: Duration = Duration::from_millis(1500);

/// First backoff step for soft failures; doubles on each consecutive failure.
const SOFT_BACKOFF_BASE: Duration = Duration::from_millis(500);

/// Longest backoff step for soft failures.
const SOFT_BACKOFF_MAX: Duration = Duration::from_secs(30);

/// What a watch loop should do after a failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryAction {
    /// Wait for the given duration, then retry.
    Retry(Duration),
    /// Permanent failure: stop the loop.
    GiveUp,
}

/// Longest a single list/watch-establishment request may take before it feeds
/// the retry policy. kube leaves `read_timeout` unset, so without this a
/// stalled connection hangs until the OS gives up on the TCP connection
/// (~40s+). Watch streams and log follows are unaffected.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);

/// Run one API request with a timeout so a stalled connection fails fast.
pub async fn bounded<T>(
    request: impl std::future::Future<Output = Result<T, kube::Error>>,
) -> Result<T, RequestFailure> {
    match tokio::time::timeout(REQUEST_TIMEOUT, request).await {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => Err(RequestFailure::Api(error)),
        Err(_) => Err(RequestFailure::TimedOut),
    }
}

/// A failed Kubernetes API request.
#[derive(Debug)]
pub enum RequestFailure {
    /// The API server answered with an error status.
    Api(kube::Error),
    /// The request produced nothing within [`REQUEST_TIMEOUT`].
    TimedOut,
}

impl RequestFailure {
    /// Classify into the retry policy; timeouts are always soft.
    pub fn action(&self, retry: &mut Retry) -> RetryAction {
        match self {
            Self::Api(error) => retry.failed(error),
            Self::TimedOut => retry.failed_soft(),
        }
    }
}

impl std::fmt::Display for RequestFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Api(error) => write!(f, "{error}"),
            Self::TimedOut => write!(f, "no response for {}s", REQUEST_TIMEOUT.as_secs()),
        }
    }
}

/// True when the HTTP status will never succeed on retry for a list/watch.
pub fn is_hard_status(code: u16) -> bool {
    matches!(code, 400 | 401 | 403 | 404 | 405 | 422)
}

/// True when the error is a permanent API rejection (see [`is_hard_status`]).
/// Everything else — network, TLS, timeouts, 429, 5xx — is soft.
pub fn is_hard(error: &kube::Error) -> bool {
    match error {
        kube::Error::Api(status) => is_hard_status(status.code),
        _ => false,
    }
}

/// Tracks consecutive failures for one watch loop.
#[derive(Default)]
pub struct Retry {
    soft_failures: u32,
    hard_failures: u32,
}

impl Retry {
    pub fn new() -> Self {
        Self::default()
    }

    /// A list call succeeded: connectivity works, so the soft backoff resets.
    /// Hard failures stay counted: a resource can be listable but not
    /// watchable, and list success must not clear that signal.
    pub fn list_succeeded(&mut self) {
        self.soft_failures = 0;
    }

    /// The watch stream delivered a real event: fully healthy.
    pub fn watch_succeeded(&mut self) {
        self.soft_failures = 0;
        self.hard_failures = 0;
    }

    /// Decide what to do after a call failed with `error`.
    pub fn failed(&mut self, error: &kube::Error) -> RetryAction {
        self.fail(is_hard(error))
    }

    /// Decide what to do after a watch `Error` event carrying only a status.
    pub fn failed_status(&mut self, code: u16) -> RetryAction {
        self.fail(is_hard_status(code))
    }

    /// A failure with nothing to classify (stream ended, read error): always
    /// soft.
    pub fn failed_soft(&mut self) -> RetryAction {
        self.fail(false)
    }

    fn fail(&mut self, hard: bool) -> RetryAction {
        if hard {
            self.hard_failures = self.hard_failures.saturating_add(1);
            if self.hard_failures >= MAX_HARD_ATTEMPTS {
                return RetryAction::GiveUp;
            }
            return RetryAction::Retry(HARD_RETRY_DELAY);
        }
        self.soft_failures = self.soft_failures.saturating_add(1);
        RetryAction::Retry(self.backoff())
    }

    fn backoff(&self) -> Duration {
        let shift = self.soft_failures.saturating_sub(1).min(6);
        (SOFT_BACKOFF_BASE * (1u32 << shift)).min(SOFT_BACKOFF_MAX)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hard_failures_give_up_after_two_attempts() {
        let mut retry = Retry::new();
        assert_eq!(
            retry.failed_status(403),
            RetryAction::Retry(HARD_RETRY_DELAY)
        );
        assert_eq!(retry.failed_status(403), RetryAction::GiveUp);
    }

    #[test]
    fn list_success_does_not_clear_hard_failures() {
        let mut retry = Retry::new();
        retry.failed_status(403);
        retry.list_succeeded();
        // A resource can be listable but not watchable: the second 403
        // still gives up instead of looping forever.
        assert_eq!(retry.failed_status(403), RetryAction::GiveUp);
    }

    #[test]
    fn watch_success_resets_everything() {
        let mut retry = Retry::new();
        retry.failed_status(403);
        retry.watch_succeeded();
        assert_eq!(
            retry.failed_status(403),
            RetryAction::Retry(HARD_RETRY_DELAY)
        );
    }

    #[test]
    fn soft_failures_back_off_exponentially_with_cap() {
        let mut retry = Retry::new();
        let mut delays = Vec::new();
        for _ in 0..10 {
            let RetryAction::Retry(delay) = retry.failed_status(500) else {
                panic!("soft failures never give up");
            };
            delays.push(delay);
        }
        assert_eq!(
            delays,
            vec![
                Duration::from_millis(500),
                Duration::from_secs(1),
                Duration::from_secs(2),
                Duration::from_secs(4),
                Duration::from_secs(8),
                Duration::from_secs(16),
                Duration::from_secs(30),
                Duration::from_secs(30),
                Duration::from_secs(30),
                Duration::from_secs(30),
            ]
        );
    }

    #[test]
    fn list_success_resets_soft_backoff() {
        let mut retry = Retry::new();
        retry.failed_status(500);
        retry.failed_status(500);
        retry.list_succeeded();
        assert_eq!(
            retry.failed_status(500),
            RetryAction::Retry(Duration::from_millis(500))
        );
    }

    #[test]
    fn status_classification() {
        for code in [400, 401, 403, 404, 405, 422] {
            assert!(is_hard_status(code), "{code} should be hard");
        }
        for code in [410, 429, 500, 502, 503] {
            assert!(!is_hard_status(code), "{code} should be soft");
        }
    }

    #[test]
    fn timeouts_are_soft_and_back_off() {
        let mut retry = Retry::new();
        assert_eq!(
            RequestFailure::TimedOut.action(&mut retry),
            RetryAction::Retry(Duration::from_millis(500))
        );
        assert_eq!(
            RequestFailure::TimedOut.action(&mut retry),
            RetryAction::Retry(Duration::from_secs(1))
        );
    }
}
