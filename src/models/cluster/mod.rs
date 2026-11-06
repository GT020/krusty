//! Namespace filter shared by repositories and view-models.

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum NamespaceScope {
    #[default]
    All,
    One(String),
    /// Two or more namespaces. Names are kept sorted and free of duplicates so
    /// the scope can be used as a stable map key.
    Many(Vec<String>),
}

impl NamespaceScope {
    /// Build a scope from a set of namespaces, normalising order and
    /// duplicates. Zero namespaces means "all", one means [`Self::One`], and two
    /// or more mean [`Self::Many`].
    pub fn from_namespaces(namespaces: impl IntoIterator<Item = String>) -> Self {
        let mut namespaces: Vec<String> = namespaces
            .into_iter()
            .filter(|namespace| !namespace.is_empty())
            .collect();
        namespaces.sort();
        namespaces.dedup();

        match namespaces.len() {
            0 => Self::All,
            1 => Self::One(namespaces.remove(0)),
            _ => Self::Many(namespaces),
        }
    }

    pub fn namespace(&self) -> Option<&str> {
        match self {
            Self::One(namespace) => Some(namespace),
            _ => None,
        }
    }

    /// Namespaces selected by this scope. Empty for [`Self::All`].
    pub fn selected_namespaces(&self) -> Vec<String> {
        match self {
            Self::All => Vec::new(),
            Self::One(namespace) => vec![namespace.clone()],
            Self::Many(namespaces) => namespaces.clone(),
        }
    }

    /// Iterate over the selected namespaces, or nothing for [`Self::All`].
    pub fn namespaces(&self) -> impl Iterator<Item = &str> {
        let namespaces: &[String] = match self {
            Self::All => &[],
            Self::One(namespace) => std::slice::from_ref(namespace),
            Self::Many(namespaces) => namespaces,
        };
        namespaces.iter().map(String::as_str)
    }

    pub fn label(&self) -> String {
        match self {
            Self::All => "All namespaces".to_string(),
            Self::One(namespace) => namespace.clone(),
            Self::Many(namespaces) => namespaces.join(", "),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scopes_are_normalised_from_namespace_lists() {
        assert_eq!(NamespaceScope::from_namespaces([]), NamespaceScope::All);
        assert_eq!(
            NamespaceScope::from_namespaces(["default".to_string()]),
            NamespaceScope::One("default".to_string())
        );
        assert_eq!(
            NamespaceScope::from_namespaces([
                "kube-system".to_string(),
                "default".to_string(),
                "kube-system".to_string(),
            ]),
            NamespaceScope::Many(vec!["default".to_string(), "kube-system".to_string()])
        );
    }

    #[test]
    fn selected_namespaces_lists_each_scope() {
        assert!(NamespaceScope::All.selected_namespaces().is_empty());
        assert_eq!(
            NamespaceScope::One("default".to_string()).selected_namespaces(),
            ["default"]
        );
        assert_eq!(
            NamespaceScope::Many(vec!["default".to_string(), "kube-system".to_string()])
                .selected_namespaces(),
            ["default", "kube-system"]
        );
    }

    #[test]
    fn labels_describe_the_scope() {
        assert_eq!(NamespaceScope::All.label(), "All namespaces");
        assert_eq!(
            NamespaceScope::One("default".to_string()).label(),
            "default"
        );
        assert_eq!(
            NamespaceScope::Many(vec!["default".to_string(), "kube-system".to_string()]).label(),
            "default, kube-system"
        );
    }
}
