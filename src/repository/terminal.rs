//! Persistent terminal sessions owned by the repository, not by a view.
//!
//! A terminal view attaches to a session and can detach without killing the
//! shell. Closing the terminal tab destroys the session explicitly.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use futures::SinkExt as _;
use gpui_kit::{App, BorrowAppContext as _};
use k8s_openapi::api::core::v1::Pod;
use kube::Client;
use kube::api::{Api, AttachParams, TerminalSize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::sync::{broadcast, mpsc, watch};

use crate::cluster::ClusterState;
use crate::repository::Repository;

const HISTORY_BYTES: usize = 1024 * 1024;
const OUTPUT_CAPACITY: usize = 256;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct TerminalKey {
    epoch: u64,
    namespace: String,
    pod: String,
    container: String,
    shell: String,
}

#[derive(Clone, Debug)]
pub(crate) enum TerminalEvent {
    Data(Vec<u8>),
    Error(String),
    Ended,
}

pub(crate) struct TerminalAttachment {
    pub history: Vec<u8>,
    pub updates: broadcast::Receiver<TerminalEvent>,
}

pub(crate) struct TerminalSession {
    key: TerminalKey,
    history: Mutex<VecDeque<u8>>,
    output: broadcast::Sender<TerminalEvent>,
    input: mpsc::UnboundedSender<Vec<u8>>,
    resize: mpsc::UnboundedSender<(u16, u16)>,
    cancel: watch::Sender<bool>,
    finished: AtomicBool,
}

impl TerminalSession {
    pub(crate) fn key(&self) -> &TerminalKey {
        &self.key
    }

    pub(crate) fn attach(&self) -> TerminalAttachment {
        let history = self
            .history
            .lock()
            .map(|history| history.iter().copied().collect())
            .unwrap_or_default();
        TerminalAttachment {
            history,
            updates: self.output.subscribe(),
        }
    }

    pub(crate) fn input_handle(&self) -> mpsc::UnboundedSender<Vec<u8>> {
        self.input.clone()
    }

    pub(crate) fn resize_handle(&self) -> mpsc::UnboundedSender<(u16, u16)> {
        self.resize.clone()
    }

    pub(crate) fn is_finished(&self) -> bool {
        self.finished.load(Ordering::Relaxed)
    }

    pub(crate) fn destroy(&self) {
        let _ = self.cancel.send(true);
    }

    fn push_data(&self, bytes: Vec<u8>) {
        if let Ok(mut history) = self.history.lock() {
            history.extend(bytes.iter().copied());
            let overflow = history.len().saturating_sub(HISTORY_BYTES);
            if overflow > 0 {
                history.drain(..overflow);
            }
        }
        let _ = self.output.send(TerminalEvent::Data(bytes));
    }

    fn push_error(&self, message: String) {
        let _ = self.output.send(TerminalEvent::Error(message));
    }

    fn finish(&self) {
        self.finished.store(true, Ordering::Relaxed);
        let _ = self.output.send(TerminalEvent::Ended);
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = self.cancel.send(true);
    }
}

#[derive(Default)]
pub(crate) struct TerminalSessionStore {
    sessions: HashMap<TerminalKey, Arc<TerminalSession>>,
}

impl TerminalSessionStore {
    fn retain_epoch(&mut self, epoch: u64) {
        self.sessions.retain(|key, session| {
            if key.epoch == epoch {
                true
            } else {
                session.destroy();
                false
            }
        });
    }

    fn get(&self, key: &TerminalKey) -> Option<Arc<TerminalSession>> {
        self.sessions.get(key).cloned()
    }

    fn insert(&mut self, key: TerminalKey, session: Arc<TerminalSession>) {
        self.sessions.insert(key, session);
    }

    fn remove(&mut self, key: &TerminalKey) -> Option<Arc<TerminalSession>> {
        self.sessions.remove(key)
    }
}

/// Open or reuse a terminal session for one pod container shell.
pub(crate) fn open(
    cx: &mut App,
    namespace: &str,
    name: &str,
    container: &str,
    shell: &str,
) -> Result<Arc<TerminalSession>, String> {
    if container.is_empty() {
        return Err("This pod has no containers to exec into.".to_string());
    }

    let state = cx.global::<ClusterState>();
    let client = state.client().ok_or_else(|| "not connected".to_string())?;
    let tokio = state.tokio();
    let epoch = state.context_epoch();
    let key = TerminalKey {
        epoch,
        namespace: namespace.to_string(),
        pod: name.to_string(),
        container: container.to_string(),
        shell: shell.to_string(),
    };

    if let Some(session) = cx.update_global::<Repository, _>(|repository, _| {
        repository.terminal.retain_epoch(epoch);
        repository.terminal.get(&key)
    }) && !session.is_finished()
    {
        return Ok(session);
    }

    if let Some(old) =
        cx.update_global::<Repository, _>(|repository, _| repository.terminal.remove(&key))
    {
        old.destroy();
    }

    let (output, _) = broadcast::channel(OUTPUT_CAPACITY);
    let (input_tx, input_rx) = mpsc::unbounded_channel();
    let (resize_tx, resize_rx) = mpsc::unbounded_channel();
    let (cancel, cancel_rx) = watch::channel(false);
    let session = Arc::new(TerminalSession {
        key: key.clone(),
        history: Mutex::new(VecDeque::new()),
        output,
        input: input_tx,
        resize: resize_tx,
        cancel,
        finished: AtomicBool::new(false),
    });

    cx.update_global::<Repository, _>(|repository, _| {
        repository.terminal.insert(key.clone(), session.clone());
    });

    tokio.spawn(run_session(
        client,
        key,
        session.clone(),
        input_rx,
        resize_rx,
        cancel_rx,
    ));
    Ok(session)
}

/// Explicitly destroy the session for a key. Detaching a view must not call
/// this; closing the terminal tab or switching clusters must.
pub(crate) fn destroy(cx: &mut App, key: &TerminalKey) {
    let session =
        cx.update_global::<Repository, _>(|repository, _| repository.terminal.remove(key));
    if let Some(session) = session {
        session.destroy();
    }
}

async fn run_session(
    client: Client,
    key: TerminalKey,
    session: Arc<TerminalSession>,
    mut input: mpsc::UnboundedReceiver<Vec<u8>>,
    mut resize: mpsc::UnboundedReceiver<(u16, u16)>,
    mut cancel: watch::Receiver<bool>,
) {
    let api: Api<Pod> = Api::namespaced(client, &key.namespace);
    let params = AttachParams {
        container: Some(key.container.clone()),
        stdin: true,
        stdout: true,
        stderr: false,
        tty: true,
        ..AttachParams::default()
    };
    tracing::debug!(
        namespace = %key.namespace,
        pod = %key.pod,
        container = %key.container,
        shell = %key.shell,
        "terminal: connecting"
    );
    let mut process = match api.exec(&key.pod, [key.shell.as_str()], &params).await {
        Ok(process) => process,
        Err(error) => {
            session.push_error(format!("Could not start exec session: {error}"));
            session.finish();
            return;
        }
    };
    tracing::debug!(pod = %key.pod, container = %key.container, "terminal: connected");

    let Some(mut stdin) = process.stdin() else {
        session.push_error("Exec session has no stdin channel.".to_string());
        process.abort();
        session.finish();
        return;
    };
    let Some(stdout) = process.stdout() else {
        session.push_error("Exec session has no stdout channel.".to_string());
        process.abort();
        session.finish();
        return;
    };
    let stderr = process.stderr();

    let writer = tokio::spawn(async move {
        while let Some(bytes) = input.recv().await {
            if stdin.write_all(&bytes).await.is_err() || stdin.flush().await.is_err() {
                break;
            }
        }
    });
    let stdout_task = tokio::spawn(read_output(stdout, session.clone()));
    let stderr_task = stderr.map(|stderr| tokio::spawn(read_output(stderr, session.clone())));

    let mut resize_task = process.terminal_size().map(|size_sender| {
        tokio::spawn(async move {
            let mut size_sender = size_sender;
            while let Some((cols, rows)) = resize.recv().await {
                let size = TerminalSize {
                    width: cols,
                    height: rows,
                };
                if size_sender.send(size).await.is_err() {
                    break;
                }
            }
        })
    });

    let mut status_future = process.take_status();
    let status = tokio::select! {
        _ = cancel.changed() => {
            process.abort();
            writer.abort();
            stdout_task.abort();
            if let Some(task) = stderr_task { task.abort(); }
            if let Some(task) = resize_task.take() { task.abort(); }
            session.finish();
            return;
        }
        status = async {
            match status_future.take() {
                Some(status) => status.await,
                None => None,
            }
        } => status,
    };
    process.abort();
    writer.abort();
    stdout_task.abort();
    if let Some(task) = stderr_task {
        task.abort();
    }
    if let Some(task) = resize_task.take() {
        task.abort();
    }
    match status {
        Some(status) if status.status.as_deref() == Some("Success") => {
            session.push_data(b"\r\n[session ended]\r\n".to_vec());
        }
        Some(status) => {
            let message = status
                .message
                .as_deref()
                .filter(|message| !message.is_empty())
                .map(|message| format!("Exec session ended: {message}"))
                .unwrap_or_else(|| "Exec session ended".to_string());
            session.push_error(message);
        }
        None => {
            session.push_data(b"\r\n[session closed]\r\n".to_vec());
        }
    }
    session.finish();
}

async fn read_output<R>(mut reader: R, session: Arc<TerminalSession>)
where
    R: AsyncRead + Unpin,
{
    let mut buffer = [0u8; 8192];
    loop {
        match reader.read(&mut buffer).await {
            Ok(0) => return,
            Ok(read) => session.push_data(buffer[..read].to_vec()),
            Err(error) => {
                session.push_error(format!("Exec stream error: {error}"));
                return;
            }
        }
    }
}
