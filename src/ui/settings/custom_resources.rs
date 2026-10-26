//! User-defined custom resource definitions and the dialog to add them.
//!
//! Definitions are persisted next to the other application preferences so a
//! resource added once shows up in the sidebar on every launch.

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;

use gpui_kit::base::v_flex;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{Sizable as _, WindowExt as _};
use gpui_kit::{
    App, AppContext as _, BorrowAppContext as _, ClickEvent, Context, Global, IntoElement,
    ParentElement as _, Styled as _, Window, px,
};
use serde::{Deserialize, Serialize};

use crate::models::ResourceKind;
use crate::models::resources::catalog::{self, CustomResourceDef};

const CUSTOM_RESOURCES_FILE: &str = "custom_resources.json";

/// Global revision counter so the shell can rebuild the sidebar when a
/// definition is added.
#[derive(Default)]
pub struct CustomResourcesState {
    revision: u64,
}

impl Global for CustomResourcesState {}

#[derive(Default, Serialize, Deserialize)]
struct StoredCustomResources {
    #[serde(default)]
    resources: Vec<CustomResourceDef>,
}

/// Load persisted definitions before the first window opens.
pub fn init(cx: &mut App) {
    let stored = StoredCustomResources::load();
    for def in stored.resources {
        catalog::add_custom_resource(def);
    }
    cx.set_global(CustomResourcesState::default());
    tracing::debug!(
        custom_resources = catalog::custom_resources().len(),
        "ui: custom resources loaded"
    );
}

pub fn add(cx: &mut App, def: CustomResourceDef) -> Result<ResourceKind, String> {
    let def = normalize(def)?;
    let kind = catalog::add_custom_resource(def);
    save();
    cx.update_global::<CustomResourcesState, _>(|state, _| {
        state.revision = state.revision.wrapping_add(1);
    });
    tracing::info!(kind = kind.label(), "ui: custom resource added");
    Ok(kind)
}

fn normalize(mut def: CustomResourceDef) -> Result<CustomResourceDef, String> {
    def.group = def.group.trim().to_string();
    def.version = def.version.trim().to_string();
    def.kind = def.kind.trim().to_string();
    def.plural = def.plural.trim().to_string();

    if def.version.is_empty() {
        return Err("Version is required.".to_string());
    }
    if def.kind.is_empty() {
        return Err("Kind is required.".to_string());
    }
    if def.plural.is_empty() {
        def.plural = format!("{}s", def.kind.to_lowercase());
    }
    if def.plural.chars().any(char::is_whitespace) {
        return Err("Plural must not contain spaces.".to_string());
    }
    Ok(def)
}

fn save() {
    let stored = StoredCustomResources {
        resources: catalog::custom_resources().into_iter().cloned().collect(),
    };
    let Some(path) = StoredCustomResources::path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match serde_json::to_vec_pretty(&stored) {
        Ok(bytes) => {
            if let Err(error) = std::fs::write(&path, bytes) {
                tracing::warn!(
                    path = %path.display(),
                    %error,
                    "ui: could not save custom resources"
                );
            }
        }
        Err(error) => {
            tracing::warn!(%error, "ui: could not serialize custom resources");
        }
    }
}

impl StoredCustomResources {
    fn path() -> Option<PathBuf> {
        dirs::config_dir().map(|dir| dir.join("krusty").join(CUSTOM_RESOURCES_FILE))
    }

    fn load() -> Self {
        let Some(path) = Self::path() else {
            return Self::default();
        };
        match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|error| {
                tracing::warn!(
                    path = %path.display(),
                    %error,
                    "ui: invalid custom resources file, ignoring it"
                );
                Self::default()
            }),
            Err(_) => Self::default(),
        }
    }
}

/// Settings section action. Opens a small form for a CRD-like definition.
pub fn render_add_button(cx: &Context<crate::ui::Workspace>) -> impl IntoElement {
    use gpui_kit::component::button::Button;

    let count = catalog::custom_resources().len();
    v_flex()
        .gap_2()
        .child(
            gpui_kit::div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(if count == 1 {
                    "1 custom resource in the sidebar.".to_string()
                } else {
                    format!("{count} custom resources in the sidebar.")
                }),
        )
        .child(
            Button::new("add-custom-resource")
                .outline()
                .small()
                .label("Add custom resource")
                .on_click(move |_, window, cx| open_add_dialog(window, cx)),
        )
}

fn open_add_dialog(window: &mut Window, cx: &mut App) {
    let group = cx.new(|cx| InputState::new(window, cx).placeholder("example.com"));
    let version = cx.new(|cx| {
        InputState::new(window, cx)
            .placeholder("v1")
            .default_value("v1")
    });
    let kind = cx.new(|cx| InputState::new(window, cx).placeholder("MyResource"));
    let plural = cx.new(|cx| InputState::new(window, cx).placeholder("myresources (optional)"));
    let namespaced = Rc::new(Cell::new(true));

    window.open_alert_dialog(cx, move |alert, _, _| {
        let namespaced_state = Rc::clone(&namespaced);
        let namespaced_click = Rc::clone(&namespaced);
        let namespaced_ok = Rc::clone(&namespaced);
        let group = group.clone();
        let version = version.clone();
        let kind = kind.clone();
        let plural = plural.clone();
        alert
            .title("Add custom resource")
            .description(
                "Enter the API group, version, and kind for a resource your cluster \
                 serves. It will appear under Custom in the sidebar.",
            )
            .child(
                v_flex()
                    .gap_2()
                    .w(px(360.))
                    .child(group_input(&group, "API group"))
                    .child(group_input(&version, "Version"))
                    .child(group_input(&kind, "Kind"))
                    .child(group_input(&plural, "Plural"))
                    .child(
                        Checkbox::new("custom-resource-namespaced")
                            .label("Namespaced")
                            .checked(namespaced_state.get())
                            .on_click(move |checked, _, _| {
                                namespaced_click.set(*checked);
                            }),
                    ),
            )
            .ok_text("Add")
            .show_cancel(true)
            .on_ok(move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                let def = CustomResourceDef {
                    group: group.read(cx).value().to_string(),
                    version: version.read(cx).value().to_string(),
                    kind: kind.read(cx).value().to_string(),
                    plural: plural.read(cx).value().to_string(),
                    namespaced: namespaced_ok.get(),
                };
                match add(cx, def) {
                    Ok(_) => true,
                    Err(error) => {
                        tracing::warn!(%error, "ui: invalid custom resource definition");
                        false
                    }
                }
            })
    });
}

fn group_input(input: &gpui_kit::Entity<InputState>, label: &'static str) -> impl IntoElement {
    v_flex()
        .gap_1()
        .child(gpui_kit::div().text_xs().child(label))
        .child(Input::new(input).small())
}
