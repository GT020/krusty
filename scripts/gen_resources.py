#!/usr/bin/env python3
"""One Model + ViewModel + View per resource. Plain duplicated code, no shared abstractions."""
import pathlib, textwrap

RESOURCES = {
    "pods": ("Pods", [("name","Name",280,None), ("ready","Ready",80,"center"), ("restarts","Restarts",90,"center"), ("status","Status",110,None), ("age","Age",90,"right")]),
    "deployments": ("Deployments", [("name","Name",280,None), ("namespace","Namespace",140,None), ("middle","Details",120,None), ("status","Status",110,None), ("age","Age",90,"right")]),
    "daemon_sets": ("DaemonSets", [("name","Name",280,None), ("namespace","Namespace",140,None), ("middle","Details",120,None), ("status","Status",110,None), ("age","Age",90,"right")]),
    "jobs": ("Jobs", [("name","Name",280,None), ("namespace","Namespace",140,None), ("middle","Details",120,None), ("status","Status",110,None), ("age","Age",90,"right")]),
    "services": ("Services", [("name","Name",280,None), ("namespace","Namespace",140,None), ("middle","Port",120,"center"), ("status","Type",110,None), ("age","Age",90,"right")]),
    "ingresses": ("Ingresses", [("name","Name",280,None), ("namespace","Namespace",140,None), ("middle","Host",220,None), ("status","Kind",110,None), ("age","Age",90,"right")]),
    "persistent_volumes": ("PersistentVolumes", [("name","Name",240,None), ("capacity","Capacity",110,"right"), ("status","Status",110,None), ("age","Age",90,"right")]),
    "persistent_volume_claims": ("PersistentVolumeClaims", [("name","Name",240,None), ("capacity","Capacity",110,"right"), ("status","Status",110,None), ("age","Age",90,"right")]),
    "config_maps": ("ConfigMaps", [("name","Name",280,None), ("namespace","Namespace",140,None), ("middle","Keys",140,"center"), ("status","Content",110,None), ("age","Age",90,"right")]),
    "secrets": ("Secrets", [("name","Name",280,None), ("namespace","Namespace",140,None), ("middle","Keys",140,"center"), ("status","Content",110,None), ("age","Age",90,"right")]),
    "nodes": ("Nodes", [("name","Name",220,None), ("status","Status",110,None), ("version","Version",120,None), ("age","Age",90,"right")]),
}

COLUMNS = '''"name" => item.name.into(),
            "namespace" => item.namespace.into(),
            "ready" => item.ready.into(),
            "restarts" => item.restarts.to_string(),
            "capacity" => item.capacity.into(),
            "version" => item.version.into(),
            "age" => item.age.into(),
            "status" => item.status.into(),
            "middle" => String::new(),
'''

def columns_code(cols, indent=8):
    out=[]
    for key,label,w,align in cols:
        c=f'Column::new("{key}", "{label}").width(px({w}.))'
        if align=="center": c+=".text_center()"
        if align=="right": c+=".text_right()"
        out.append(" "*indent+c+",")
    return "\n".join(out)

def render(template, snake, camel, cols):
    body = template.replace("@CAMEL@", camel).replace("@SNAKE@", snake)
    body = body.replace("@COLUMNS_CODE@", columns_code(cols))
    return body

MODEL_T = textwrap.dedent('''\
    //! @CAMEL@ model. Domain state only; no presentation or gpui types.

    use crate::models::{Item, LoadState, ResourceKind, cluster_data::items};

    #[derive(Clone)]
    pub struct @CAMEL@Model {
        items: Vec<Item>,
        load: LoadState,
        revision: u64,
    }

    impl @CAMEL@Model {
        pub fn new() -> Self {
            Self {
                items: items(ResourceKind::@CAMEL@),
                load: LoadState::Loaded,
                revision: 0,
            }
        }

        pub fn items(&self) -> &[Item] {
            &self.items
        }

        pub fn load(&self) -> LoadState {
            self.load
        }

        pub fn revision(&self) -> u64 {
            self.revision
        }

        /// Refresh from the cluster source. With kube-rs the response must carry
        /// the revision so stale data is dropped.
        pub fn refresh(&mut self) {
            self.revision += 1;
            self.items = items(ResourceKindKinds::@CAMEL@);
            self.load = LoadState::Loaded;
        }
    }
    ''')

VM_T = textwrap.dedent('''\
    //! @CAMEL@ view-model. Owns presentation state, binds the @CAMEL@Model.

    use crate::models::@SNAKE@::@CAMEL@Model;
    use gpui_kit::component::table::{Column, TableDelegate, TableState};
    use gpui_kit::{App, AppContext as _, Context, Entity, Styled as _, Window, div};

    pub struct @CAMEL@ViewModel {
        model: Entity<@CAMEL@Model>,
        table: Entity<TableState<@CAMEL@Table>>,
        _model_subscription: gpui_kit::Subscription,
    }

    impl @CAMEL@ViewModel {
        pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
            let model = cx.new(|_| @CAMEL@Model::new());
            let table_model = model.clone();
            let table = cx.new(|cx| {
                TableState::new(@CAMEL@Table::new(table_model), window, cx)
            });
            let subscription = cx.observe(&model, |_, _, cx| cx.notify());
            Self {
                model,
                table,
                _model_subscription: subscription,
            }
        }

        pub fn table(&self) -> &Entity<TableState<@CAMEL@Table>> {
            &self.table
        }

        /// User intent: refresh from the cluster.
        pub fn refresh(&mut self, cx: &mut Context<Self>) {
            self.model.update(cx, |m, _| m.refresh());
            self.table.update(cx, |state, cx| state.refresh(cx));
        }
    }

    fn columns() -> Vec<Column> {
        vec![
@COLUMNS_CODE@
        ]
    }

    fn column_key(col_ix: usize) -> &'static str {
        match columns().get(col_ix) {
            Some(c) => c.key.as_ref(),
            None => "",
        }
    }

    /// Delegate = the binding seam for this resource only.
    pub struct @CAMEL@Table {
        model: Entity<@CAMEL@Model>,
    }

    impl @CAMEL@Table {
        pub fn new(model: Entity<@CAMEL@Model>) -> Self {
            Self { model }
        }

        fn cell(&self, item: &crate::models::Item, key: &str) -> String {
            match key {
                @CELL_ARMS@
                _ => String::new(),
            }
        }
    }

    impl TableDelegate for @CAMEL@Table {
        fn columns_count(&self, _cx: &App) -> usize {
            columns().len()
        }

        fn rows_count(&self, cx: &App) -> usize {
            self.model.read(cx).items().len()
        }

        fn column(&self, col_ix: usize, _cx: &App) -> Column {
            columns()
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
        ) -> impl gpui_kit::IntoElement {
            use gpui_kit::component::ActiveTheme as _;
            let Some(item) = self.model.read(cx).items().get(row).copied() else {
                return div().into_any_element();
            };
            let key = column_key(col_ix);
            let text = self.cell(&item, key);
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
            let Some(item) = self.model.read(cx).items().get(row).copied() else {
                return String::new();
            };
            self.cell(&item, column_key(col_ix))
        }
    }
    ''')

VIEW_T = textwrap.dedent('''\
    //! @CAMEL@ list view. Declarative composition over the view-model.

    use crate::view_models::@SNAKE@::@CAMEL@ViewModel;
    use gpui_kit::component::{
        ActiveTheme as _, IconName, Sizable as _,
        button::{Button, ButtonVariants as _},
        table::DataTable,
    };
    use gpui_kit::base::{h_flex, v_flex};
    use gpui_kit::{
        AppContext as _, Context, Entity, FontWeight, IntoElement, ParentElement as _, Render,
        Styled as _, Window, div, px,
    };

    pub struct @CAMEL@View {
        title: gpui_kit::SharedString,
        vm: Entity<@CAMEL@ViewModel>,
    }

    impl @CAMEL@View {
        pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
            Self {
                title: crate::models::ResourceKind::@CAMEL@.label().into(),
                vm: cx.new(|cx| @CAMEL@ViewModel::new(window, cx)),
            }
        }
    }

    impl Render for @CAMEL@View {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let table = self.vm.read(cx).table().clone();
            let refresh_vm = self.vm.clone();

            v_flex()
                .flex_1()
                .min_w_0()
                .h_full()
                .child(
                    h_flex()
                        .justify_between()
                        .py_2()
                        .px_4()
                        .border_b_1()
                        .border_color(cx.theme().border)
                        .child(
                            div()
                                .text_size(px(15.))
                                .font_weight(FontWeight::MEDIUM)
                                .child(self.title.clone()),
                        )
                        .child(
                            Button::new("refresh")
                                .ghost()
                                .small()
                                .icon(IconName::RefreshCw)
                                .on_click(move |_, _, cx| {
                                    refresh_vm.update(cx, @CAMEL@ViewModel::refresh);
                                }),
                        ),
                )
                .child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .px_4()
                        .py_3()
                        .child(DataTable::new(&table)),
                )
        }
    }
    ''')

CELL_ARMS = '''"name" => item.name.into(),
            "namespace" => item.namespace.into(),
            "ready" => item.ready.into(),
            "restarts" => item.restarts.to_string(),
            "capacity" => item.capacity.into(),
            "version" => item.version.into(),
            "age" => item.age.into(),
            "status" => item.status.into(),
            "middle" => String::new(),
'''

for snake,(camel,cols) in RESOURCES.items():
    pathlib.Path(f"src/models/{snake}.rs").write_text(
        render(MODEL_T, snake, camel, cols)
        .replace("ResourceKindKinds::", "ResourceKind::")
        .replace("crate::@SNAKE@", "crate::")
        .replace("@CELL_ARMS@", "")
    )
    pathlib.Path(f"src/view_models/{snake}.rs").write_text(
        render(VM_T, snake, camel, cols)
        .replace("crate::@SNAKE@@CAMEL@Model", f"crate::models::{snake}::{camel}Model")
        .replace("@CELL_ARMS@", CELL_ARMS)
    )
    pathlib.Path(f"src/ui/{snake}.rs").write_text(render(VIEW_T, snake, camel, cols))
print("generated", len(RESOURCES), "resources")
