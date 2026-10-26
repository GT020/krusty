//! Display row for a Kubernetes object, shared by every resource kind.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub uid: String,
    pub resource_version: String,
    pub name: String,
    pub namespace: String,
    pub ready: String,
    pub restarts: usize,
    pub capacity: String,
    pub status: String,
    pub version: String,
    pub created_at: Option<i64>,
}

impl Item {
    pub fn new(name: String, namespace: Option<String>) -> Self {
        Self {
            uid: String::new(),
            resource_version: String::new(),
            name,
            namespace: namespace.unwrap_or_default(),
            ready: String::new(),
            restarts: 0,
            capacity: String::new(),
            status: String::new(),
            version: String::new(),
            created_at: None,
        }
    }

    pub fn is_ok(&self) -> bool {
        matches!(
            self.status.as_str(),
            "running"
                | "bound"
                | "ready"
                | "succeeded"
                | "active"
                | "suspended"
                | "account"
                | "normal"
        )
    }

    pub fn age(&self) -> String {
        let Some(created) = self.created_at else {
            return String::new();
        };
        let secs = (jiff::Timestamp::now().as_second() - created).max(0) as u64;
        if secs >= 86_400 {
            format!("{}d", secs / 86_400)
        } else if secs >= 3_600 {
            format!("{}h", secs / 3_600)
        } else {
            format!("{}m", secs / 60)
        }
    }
}
