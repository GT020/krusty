//! Text filter applied to resource list rows.
//!
//! The filter matches the resource name only, so the same search box can serve
//! pods, deployments, namespaces, nodes, and custom resources alike.

use std::sync::Arc;

use regex::{Regex, RegexBuilder};

use super::Item;

#[derive(Clone, Debug)]
pub struct ResourceFilter {
    query: String,
    case_sensitive: bool,
    regex: bool,
    literal: String,
    matcher: Option<Result<Regex, String>>,
}

impl Default for ResourceFilter {
    fn default() -> Self {
        Self::new(String::new(), false, false)
    }
}

impl PartialEq for ResourceFilter {
    fn eq(&self, other: &Self) -> bool {
        self.query == other.query
            && self.case_sensitive == other.case_sensitive
            && self.regex == other.regex
    }
}

impl Eq for ResourceFilter {}

impl ResourceFilter {
    pub fn new(query: impl Into<String>, case_sensitive: bool, regex: bool) -> Self {
        let query = query.into();
        let literal = if case_sensitive {
            query.clone()
        } else {
            query.to_lowercase()
        };
        let matcher = if regex && !query.is_empty() {
            Some(
                RegexBuilder::new(&query)
                    .case_insensitive(!case_sensitive)
                    .build()
                    .map_err(|error| error.to_string()),
            )
        } else {
            None
        };

        Self {
            query,
            case_sensitive,
            regex,
            literal,
            matcher,
        }
    }

    pub fn case_sensitive(&self) -> bool {
        self.case_sensitive
    }

    pub fn regex(&self) -> bool {
        self.regex
    }

    pub fn is_empty(&self) -> bool {
        self.query.is_empty()
    }

    /// Validation error for an invalid regular expression.
    pub fn error(&self) -> Option<&str> {
        match &self.matcher {
            Some(Err(error)) => Some(error),
            _ => None,
        }
    }

    pub fn matches(&self, item: &Item) -> bool {
        if self.is_empty() {
            return true;
        }

        if let Some(matcher) = &self.matcher {
            let Ok(matcher) = matcher else {
                return false;
            };
            return matcher.is_match(&item.name);
        }

        if self.case_sensitive {
            item.name.contains(&self.query)
        } else {
            item.name.to_lowercase().contains(&self.literal)
        }
    }

    pub fn apply(&self, items: &[Arc<Item>]) -> Arc<[Arc<Item>]> {
        if self.is_empty() {
            return items.to_vec().into();
        }

        items
            .iter()
            .filter(|item| self.matches(item))
            .cloned()
            .collect::<Vec<_>>()
            .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(name: &str, namespace: &str) -> Item {
        Item::new(name.to_string(), Some(namespace.to_string()))
    }

    #[test]
    fn an_empty_query_matches_everything() {
        let filter = ResourceFilter::default();
        assert!(filter.matches(&item("api", "default")));
    }

    #[test]
    fn literal_search_is_case_insensitive_by_default() {
        let filter = ResourceFilter::new("API", false, false);
        assert!(filter.matches(&item("api-server", "default")));
        assert!(!filter.matches(&item("worker", "default")));
    }

    #[test]
    fn literal_search_can_be_case_sensitive() {
        let filter = ResourceFilter::new("API", true, false);
        assert!(!filter.matches(&item("api-server", "default")));
        assert!(filter.matches(&item("API-server", "default")));
    }

    #[test]
    fn search_only_matches_the_resource_name() {
        let filter = ResourceFilter::new("kube-system", false, false);
        assert!(!filter.matches(&item("api", "kube-system")));
    }

    #[test]
    fn regex_search_matches_the_resource_name() {
        let filter = ResourceFilter::new("^api-.*", false, true);
        assert!(filter.matches(&item("api-server", "default")));
        assert!(!filter.matches(&item("worker", "default")));
    }

    #[test]
    fn regex_search_reports_invalid_patterns() {
        let filter = ResourceFilter::new("[", false, true);
        assert!(filter.error().is_some());
        assert!(!filter.matches(&item("anything", "default")));
    }

    #[test]
    fn invalid_regex_does_not_reject_empty_queries() {
        let filter = ResourceFilter::new("", false, true);
        assert!(filter.error().is_none());
        assert!(filter.matches(&item("anything", "default")));
    }
}
