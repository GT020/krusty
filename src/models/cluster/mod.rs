//! Namespace filter shared by repositories and view-models.

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum NamespaceScope {
    #[default]
    All,
    One(String),
}

impl NamespaceScope {
    pub fn namespace(&self) -> Option<&str> {
        match self {
            Self::All => None,
            Self::One(namespace) => Some(namespace),
        }
    }

    pub fn label(&self) -> String {
        match self {
            Self::All => "All namespaces".to_string(),
            Self::One(namespace) => namespace.clone(),
        }
    }

    pub fn is_all(&self) -> bool {
        matches!(self, Self::All)
    }
}
