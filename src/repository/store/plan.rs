use std::collections::BTreeSet;

use crate::models::NamespaceScope;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ClusterCapabilities {
    pub global_watch: bool,
    pub accessible_namespaces: Vec<String>,
}

/// How a store should keep its namespaces fresh.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StoreWatchPlan {
    /// One watch for the whole cluster.
    All,
    /// One watch per namespace.
    PerNamespace(Vec<String>),
    /// A view asked for all namespaces but cluster-wide watch is not allowed,
    /// and no namespace-scoped subscription can be watched instead.
    UnavailableAll,
}

/// Compute the shared watch plan for the active subscription scopes.
///
/// A cluster-wide watch is used only when all namespaces are requested *and*
/// the cluster permits global watch. Otherwise each requested namespace gets
/// its own watch. If an all-namespace subscription is present without global
/// watch permission but other namespace-scoped subscriptions exist, the plan
/// keeps the namespace watches; the all-namespace view itself reports the
/// permission error.
pub fn watch_plan<I>(scopes: I, capabilities: &ClusterCapabilities) -> StoreWatchPlan
where
    I: IntoIterator<Item = NamespaceScope>,
{
    let mut namespaces = BTreeSet::new();
    let mut requested_all = false;

    for scope in scopes {
        match scope {
            NamespaceScope::All => requested_all = true,
            NamespaceScope::One(namespace) => {
                namespaces.insert(namespace);
            }
            NamespaceScope::Many(many) => {
                namespaces.extend(many);
            }
        }
    }

    if requested_all {
        if capabilities.global_watch {
            return StoreWatchPlan::All;
        }
        if namespaces.is_empty() {
            return StoreWatchPlan::UnavailableAll;
        }
    }

    StoreWatchPlan::PerNamespace(namespaces.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn capabilities(global_watch: bool) -> ClusterCapabilities {
        ClusterCapabilities {
            global_watch,
            accessible_namespaces: vec!["ns-1".into(), "ns-2".into()],
        }
    }

    #[test]
    fn all_namespaces_use_one_global_watch() {
        assert_eq!(
            watch_plan([NamespaceScope::All], &capabilities(true)),
            StoreWatchPlan::All
        );
    }

    #[test]
    fn all_namespaces_without_permission_are_unavailable() {
        assert_eq!(
            watch_plan([NamespaceScope::All], &capabilities(false)),
            StoreWatchPlan::UnavailableAll
        );
    }

    #[test]
    fn namespace_scopes_are_deduplicated_and_sorted() {
        assert_eq!(
            watch_plan(
                [
                    NamespaceScope::One("ns-2".into()),
                    NamespaceScope::One("ns-1".into()),
                    NamespaceScope::One("ns-2".into()),
                ],
                &capabilities(false),
            ),
            StoreWatchPlan::PerNamespace(vec!["ns-1".into(), "ns-2".into()])
        );
    }

    #[test]
    fn multi_namespace_scopes_watch_each_namespace() {
        assert_eq!(
            watch_plan(
                [NamespaceScope::Many(vec!["ns-2".into(), "ns-1".into()])],
                &capabilities(false),
            ),
            StoreWatchPlan::PerNamespace(vec!["ns-1".into(), "ns-2".into()])
        );
    }

    #[test]
    fn all_plus_namespace_falls_back_to_namespace_watches() {
        assert_eq!(
            watch_plan(
                [NamespaceScope::All, NamespaceScope::One("ns-1".into()),],
                &capabilities(false),
            ),
            StoreWatchPlan::PerNamespace(vec!["ns-1".into()])
        );
    }

    #[test]
    fn all_plus_namespace_uses_global_watch_when_allowed() {
        assert_eq!(
            watch_plan(
                [NamespaceScope::All, NamespaceScope::One("ns-1".into()),],
                &capabilities(true),
            ),
            StoreWatchPlan::All
        );
    }

    #[test]
    fn no_scopes_produce_an_empty_namespace_plan() {
        assert_eq!(
            watch_plan([], &capabilities(true)),
            StoreWatchPlan::PerNamespace(Vec::new())
        );
    }
}
