//! One table delegate and column layout for every resource list.

use gpui_kit::component::table::{Column, TableDelegate, TableState};
use gpui_kit::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Styled as _, Window,
    div, px,
};

use crate::models::resources::resource::ResourceModel;
use crate::models::{Item, ResourceKind};

#[derive(Clone, Copy)]
enum Align {
    Left,
    Center,
    Right,
}

struct ColumnSpec {
    key: &'static str,
    label: &'static str,
    width: f32,
    align: Align,
}

const NAME: ColumnSpec = ColumnSpec {
    key: "name",
    label: "Name",
    width: 280.0,
    align: Align::Left,
};
const NAMESPACE: ColumnSpec = ColumnSpec {
    key: "namespace",
    label: "Namespace",
    width: 150.0,
    align: Align::Left,
};
const READY: ColumnSpec = ColumnSpec {
    key: "ready",
    label: "Ready",
    width: 90.0,
    align: Align::Center,
};
const STATUS: ColumnSpec = ColumnSpec {
    key: "status",
    label: "Status",
    width: 130.0,
    align: Align::Left,
};
const AGE: ColumnSpec = ColumnSpec {
    key: "age",
    label: "Age",
    width: 90.0,
    align: Align::Right,
};

const fn spec(key: &'static str, label: &'static str, width: f32, align: Align) -> ColumnSpec {
    ColumnSpec {
        key,
        label,
        width,
        align,
    }
}

macro_rules! cols {
    ($($item:expr),* $(,)?) => {
        &const { [$($item),*] }
    };
}

fn columns(kind: ResourceKind) -> &'static [ColumnSpec] {
    match kind {
        ResourceKind::Pods => cols![
            spec("name", "Name", 260.0, Align::Left),
            NAMESPACE,
            spec("ready", "Ready", 80.0, Align::Center),
            spec("restarts", "Restarts", 90.0, Align::Center),
            STATUS,
            AGE,
        ],
        ResourceKind::Nodes => cols![
            spec("name", "Name", 220.0, Align::Left),
            spec("status", "Status", 110.0, Align::Left),
            spec("version", "Version", 130.0, Align::Left),
            spec("capacity", "CPU", 90.0, Align::Right),
            AGE,
        ],
        ResourceKind::Namespaces => cols![
            spec("name", "Name", 300.0, Align::Left),
            spec("status", "Status", 120.0, Align::Left),
            AGE,
        ],
        ResourceKind::PersistentVolumes => cols![
            NAME,
            spec("capacity", "Capacity", 120.0, Align::Right),
            STATUS,
            AGE,
        ],
        ResourceKind::StorageClasses => cols![
            NAME,
            spec("version", "Provisioner", 220.0, Align::Left),
            spec("capacity", "Reclaim policy", 140.0, Align::Left),
            spec("ready", "Binding mode", 150.0, Align::Left),
            AGE,
        ],
        ResourceKind::ConfigMaps | ResourceKind::Secrets => cols![
            spec("name", "Name", 300.0, Align::Left),
            NAMESPACE,
            spec("capacity", "Keys", 90.0, Align::Right),
            AGE,
        ],
        ResourceKind::ServiceAccounts => cols![
            NAME,
            NAMESPACE,
            spec("capacity", "Secrets", 90.0, Align::Right),
            AGE,
        ],
        ResourceKind::Deployments
        | ResourceKind::StatefulSets
        | ResourceKind::ReplicaSets
        | ResourceKind::DaemonSets
        | ResourceKind::Jobs => cols![NAME, NAMESPACE, READY, STATUS, AGE],
        ResourceKind::ReplicationControllers => cols![
            NAME,
            NAMESPACE,
            spec("ready", "Ready", 90.0, Align::Right),
            spec("status", "Status", 110.0, Align::Left),
            AGE,
        ],
        ResourceKind::CronJobs => cols![
            NAME,
            NAMESPACE,
            spec("version", "Schedule", 150.0, Align::Left),
            spec("capacity", "Active", 80.0, Align::Center),
            spec("status", "Suspend", 100.0, Align::Left),
            AGE,
        ],
        ResourceKind::HorizontalPodAutoscalers => cols![
            NAME,
            NAMESPACE,
            spec("ready", "Replicas", 90.0, Align::Right),
            spec("capacity", "Max", 80.0, Align::Right),
            spec("status", "Status", 110.0, Align::Left),
            AGE,
        ],
        ResourceKind::PodDisruptionBudgets => cols![
            NAME,
            NAMESPACE,
            spec("ready", "Healthy", 100.0, Align::Right),
            spec("capacity", "Allowed", 90.0, Align::Right),
            spec("status", "Status", 110.0, Align::Left),
            AGE,
        ],
        ResourceKind::Services => cols![
            spec("name", "Name", 260.0, Align::Left),
            NAMESPACE,
            spec("status", "Type", 130.0, Align::Left),
            spec("version", "Port", 90.0, Align::Left),
            AGE,
        ],
        ResourceKind::Ingresses => cols![
            spec("name", "Name", 260.0, Align::Left),
            NAMESPACE,
            spec("version", "Host", 260.0, Align::Left),
            AGE,
        ],
        ResourceKind::Endpoints => cols![
            NAME,
            NAMESPACE,
            spec("capacity", "Addresses", 100.0, Align::Right),
            spec("status", "Status", 100.0, Align::Left),
            AGE,
        ],
        ResourceKind::EndpointSlices => cols![
            NAME,
            NAMESPACE,
            spec("capacity", "Endpoints", 100.0, Align::Right),
            spec("version", "Address Type", 120.0, Align::Left),
            spec("status", "Status", 100.0, Align::Left),
            AGE,
        ],
        ResourceKind::NetworkPolicies => cols![
            NAME,
            NAMESPACE,
            spec("capacity", "Policy Types", 180.0, Align::Left),
            spec("status", "Status", 100.0, Align::Left),
            AGE,
        ],
        ResourceKind::PersistentVolumeClaims => cols![
            NAME,
            NAMESPACE,
            spec("capacity", "Capacity", 120.0, Align::Right),
            STATUS,
            AGE,
        ],
        ResourceKind::Leases => cols![
            NAME,
            NAMESPACE,
            spec("version", "Holder", 200.0, Align::Left),
            spec("capacity", "Duration", 90.0, Align::Right),
            spec("status", "Status", 100.0, Align::Left),
            AGE,
        ],
        ResourceKind::Events => cols![
            NAME,
            NAMESPACE,
            spec("version", "Reason", 180.0, Align::Left),
            spec("capacity", "Count", 80.0, Align::Right),
            spec("status", "Type", 100.0, Align::Left),
            AGE,
        ],
        ResourceKind::Roles => cols![
            NAME,
            NAMESPACE,
            spec("capacity", "Rules", 90.0, Align::Right),
            spec("status", "Status", 100.0, Align::Left),
            AGE,
        ],
        ResourceKind::RoleBindings => cols![
            NAME,
            NAMESPACE,
            spec("version", "Role", 220.0, Align::Left),
            spec("capacity", "Subjects", 90.0, Align::Right),
            spec("status", "Status", 100.0, Align::Left),
            AGE,
        ],
        ResourceKind::ClusterRoles => cols![
            spec("name", "Name", 300.0, Align::Left),
            spec("capacity", "Rules", 90.0, Align::Right),
            spec("status", "Status", 100.0, Align::Left),
            AGE,
        ],
        ResourceKind::Custom(_) => cols![
            NAME,
            NAMESPACE,
            spec("status", "Status", 110.0, Align::Left),
            AGE,
        ],
    }
}

fn table_columns(kind: ResourceKind) -> Vec<Column> {
    columns(kind)
        .iter()
        .map(|spec| {
            let column = Column::new(spec.key, spec.label).width(px(spec.width));
            match spec.align {
                Align::Left => column,
                Align::Center => column.text_center(),
                Align::Right => column.text_right(),
            }
        })
        .collect()
}

fn column_key(kind: ResourceKind, col_ix: usize) -> gpui_kit::SharedString {
    columns(kind)
        .get(col_ix)
        .map(|spec| gpui_kit::SharedString::from(spec.key))
        .unwrap_or_default()
}

fn cell(item: &Item, key: &str) -> String {
    match key {
        "name" => item.name.clone(),
        "namespace" => item.namespace.clone(),
        "ready" => item.ready.clone(),
        "restarts" => item.restarts.to_string(),
        "capacity" => item.capacity.clone(),
        "version" => item.version.clone(),
        "status" => item.status.clone(),
        "age" => item.age(),
        _ => String::new(),
    }
}

/// Table delegate shared by every resource kind.
pub struct ResourceTable {
    model: Entity<ResourceModel>,
    kind: ResourceKind,
}

impl ResourceTable {
    pub fn new(model: Entity<ResourceModel>, kind: ResourceKind) -> Self {
        Self { model, kind }
    }
}

impl TableDelegate for ResourceTable {
    fn columns_count(&self, _cx: &App) -> usize {
        columns(self.kind).len()
    }

    fn rows_count(&self, cx: &App) -> usize {
        self.model.read(cx).snapshot().items().len()
    }

    fn column(&self, col_ix: usize, _cx: &App) -> Column {
        table_columns(self.kind)
            .get(col_ix)
            .cloned()
            .unwrap_or_else(|| Column::new("empty", ""))
    }

    fn render_td(
        &mut self,
        row: usize,
        col_ix: usize,
        _window: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        use gpui_kit::component::ActiveTheme as _;
        let model = self.model.read(cx);
        let Some(item) = model.snapshot().items().get(row) else {
            return div().into_any_element();
        };
        let key = column_key(self.kind, col_ix);
        let text = cell(item, &key);
        if col_ix == 0 {
            return div().child(text).into_any_element();
        }
        let color = if key == "status" && !item.is_ok() {
            cx.theme().danger
        } else {
            cx.theme().muted_foreground
        };
        div().text_color(color).child(text).into_any_element()
    }

    fn cell_text(&self, row: usize, col_ix: usize, cx: &App) -> String {
        let model = self.model.read(cx);
        let Some(item) = model.snapshot().items().get(row) else {
            return String::new();
        };
        cell(item, &column_key(self.kind, col_ix))
    }
}

impl ResourceTable {
    /// Build the table entity for a model.
    pub fn new_state<V: 'static>(
        model: Entity<ResourceModel>,
        kind: ResourceKind,
        window: &mut Window,
        cx: &mut Context<V>,
    ) -> Entity<TableState<Self>> {
        let table_model = model.clone();
        cx.new(|cx| {
            TableState::new(ResourceTable::new(table_model, kind), window, cx).row_selectable(true)
        })
    }
}
