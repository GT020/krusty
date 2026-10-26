//! User-defined custom resource definitions: discovery, selection, editing,
//! and persistence.
//!
//! Definitions are persisted next to the other application preferences so a
//! resource added once shows up in the sidebar on every launch.

use std::cell::Cell;
use std::collections::HashSet;
use std::path::PathBuf;
use std::rc::Rc;

use gpui_kit::assets::IconName;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{
    Disableable as _, Sizable as _, WindowExt as _,
    button::{Button, ButtonVariant, ButtonVariants as _},
};
use gpui_kit::{
    App, AppContext as _, BorrowAppContext as _, ClickEvent, Context, Entity, Global, IntoElement,
    ParentElement as _, Render, SharedString, Styled as _, Window, div, px,
};
use serde::{Deserialize, Serialize};

use crate::models::ResourceKind;
use crate::models::resources::catalog::{self, CustomResourceDef};
use crate::repository;

const CUSTOM_RESOURCES_FILE: &str = "custom_resources.json";

/// Global revision counter so the shell can rebuild the sidebar and drop stale
/// custom resource panels when definitions change.
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

fn bump(cx: &mut App) {
    cx.update_global::<CustomResourcesState, _>(|state, _| {
        state.revision = state.revision.wrapping_add(1);
    });
}

pub fn add(cx: &mut App, def: CustomResourceDef) -> Result<ResourceKind, String> {
    let def = normalize(def)?;
    let kind = catalog::add_custom_resource(def);
    save();
    bump(cx);
    tracing::info!(kind = kind.label(), "ui: custom resource added");
    Ok(kind)
}

pub fn update_definition(cx: &mut App, index: u16, def: CustomResourceDef) -> Result<(), String> {
    let def = normalize(def)?;
    catalog::update_custom_resource(index, def)
        .ok_or_else(|| "This custom resource no longer exists.".to_string())?;
    save();
    bump(cx);
    tracing::info!(index, "ui: custom resource updated");
    Ok(())
}

pub fn delete_definition(cx: &mut App, index: u16) -> Result<(), String> {
    if !catalog::delete_custom_resource(index) {
        return Err("This custom resource no longer exists.".to_string());
    }
    save();
    bump(cx);
    tracing::info!(index, "ui: custom resource deleted");
    Ok(())
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
    if def.kind.chars().any(char::is_whitespace) {
        return Err("Kind must not contain spaces.".to_string());
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

struct DiscoveredItem {
    def: CustomResourceDef,
    selected: bool,
    existing: bool,
}

/// Settings section for custom resources. Holds scan results and selections.
pub struct CustomResourcesView {
    scanning: bool,
    discovered: Vec<DiscoveredItem>,
    error: Option<String>,
    notice: Option<String>,
}

impl CustomResourcesView {
    pub fn new() -> Self {
        Self {
            scanning: false,
            discovered: Vec::new(),
            error: None,
            notice: None,
        }
    }

    fn scan(&mut self, cx: &mut Context<Self>) {
        if self.scanning {
            return;
        }
        let receiver = match repository::custom_resources::discover(cx) {
            Ok(receiver) => receiver,
            Err(error) => {
                self.error = Some(error);
                cx.notify();
                return;
            }
        };
        self.scanning = true;
        self.error = None;
        self.notice = None;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let result = receiver
                .await
                .unwrap_or_else(|_| Err("The discovery task ended unexpectedly.".to_string()));
            let _ = this.update(cx, |this, cx| {
                this.scanning = false;
                match result {
                    Ok(defs) => this.apply_discovery(defs),
                    Err(error) => this.error = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn apply_discovery(&mut self, defs: Vec<CustomResourceDef>) {
        let existing: HashSet<(String, String, String)> = catalog::custom_resources()
            .iter()
            .map(|def| (def.group.clone(), def.version.clone(), def.plural.clone()))
            .collect();
        self.discovered = defs
            .into_iter()
            .map(|def| {
                let existing = existing.contains(&(
                    def.group.clone(),
                    def.version.clone(),
                    def.plural.clone(),
                ));
                DiscoveredItem {
                    def,
                    selected: !existing,
                    existing,
                }
            })
            .collect();
        if self.discovered.is_empty() {
            self.notice = Some("No custom resource definitions were found.".to_string());
        } else {
            self.notice = Some(format!(
                "Found {} custom resource definition{}.",
                self.discovered.len(),
                if self.discovered.len() == 1 { "" } else { "s" }
            ));
        }
    }

    fn toggle(&mut self, index: usize, selected: bool, cx: &mut Context<Self>) {
        if let Some(item) = self.discovered.get_mut(index) {
            item.selected = selected;
            cx.notify();
        }
    }

    fn add_selected(&mut self, cx: &mut Context<Self>) {
        let mut added = 0;
        for item in &mut self.discovered {
            if item.selected && !item.existing {
                catalog::add_custom_resource(item.def.clone());
                item.existing = true;
                item.selected = false;
                added += 1;
            }
        }
        if added > 0 {
            save();
            bump(cx);
            self.notice = Some(format!(
                "Added {added} custom resource definition{} to the sidebar.",
                if added == 1 { "" } else { "s" }
            ));
        } else {
            self.notice = Some("Select at least one new resource to add.".to_string());
        }
        self.error = None;
        cx.notify();
    }

    fn clear_discovery(&mut self, cx: &mut Context<Self>) {
        self.discovered.clear();
        self.notice = None;
        cx.notify();
    }
}

impl Render for CustomResourcesView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity().clone();
        let resources = catalog::custom_resource_entries();
        let selected_new = self
            .discovered
            .iter()
            .filter(|item| item.selected && !item.existing)
            .count();

        let scan_view = view.clone();
        let scan = Button::new("scan-custom-resources")
            .outline()
            .small()
            .icon(IconName::RefreshCw)
            .label(if self.scanning {
                "Scanning…"
            } else {
                "Scan cluster"
            })
            .on_click(move |_, _, cx| {
                scan_view.update(cx, |this, cx| this.scan(cx));
            });

        let add_view = view.clone();
        let add = Button::new("add-custom-resource")
            .outline()
            .small()
            .icon(IconName::Plus)
            .label("Add manually")
            .on_click(move |_, window, cx| {
                open_definition_dialog(window, cx, add_view.clone(), None);
            });

        let mut root = v_flex().gap_3().child(
            h_flex()
                .gap_2()
                .items_center()
                .child(scan)
                .child(add)
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(if resources.len() == 1 {
                            "1 resource configured".to_string()
                        } else {
                            format!("{} resources configured", resources.len())
                        }),
                ),
        );

        if let Some(error) = &self.error {
            root = root.child(
                div()
                    .text_xs()
                    .whitespace_normal()
                    .text_color(cx.theme().danger)
                    .child(SharedString::from(error.clone())),
            );
        } else if let Some(notice) = &self.notice {
            root = root.child(
                div()
                    .text_xs()
                    .whitespace_normal()
                    .text_color(cx.theme().muted_foreground)
                    .child(SharedString::from(notice.clone())),
            );
        }

        if !self.discovered.is_empty() {
            let mut list = v_flex().gap_1();
            for (index, item) in self.discovered.iter().enumerate() {
                let checkbox_view = view.clone();
                let label = format!(
                    "{} · {} · {}",
                    item.def.kind,
                    api_version(&item.def),
                    if item.def.namespaced {
                        "Namespaced"
                    } else {
                        "Cluster"
                    }
                );
                list = list.child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .py_1()
                        .child(
                            Checkbox::new(format!("discovered-{index}"))
                                .label(label)
                                .checked(item.selected)
                                .disabled(item.existing)
                                .on_click(move |checked, _, cx| {
                                    let checked = *checked;
                                    checkbox_view.update(cx, |this, cx| {
                                        this.toggle(index, checked, cx);
                                    });
                                }),
                        )
                        .child(if item.existing {
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child("already configured")
                                .into_any_element()
                        } else {
                            div().into_any_element()
                        }),
                );
            }

            let add_selected_view = view.clone();
            let add_selected = Button::new("add-selected-custom-resources")
                .primary()
                .small()
                .label(format!("Add selected ({selected_new})"))
                .disabled(selected_new == 0)
                .on_click(move |_, _, cx| {
                    add_selected_view.update(cx, |this, cx| this.add_selected(cx));
                });
            let clear_view = view.clone();
            let clear = Button::new("clear-custom-resource-scan")
                .ghost()
                .small()
                .label("Clear")
                .on_click(move |_, _, cx| {
                    clear_view.update(cx, |this, cx| this.clear_discovery(cx));
                });

            root = root.child(
                v_flex()
                    .gap_2()
                    .rounded(cx.theme().radius)
                    .border_1()
                    .border_color(cx.theme().border)
                    .p_3()
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child("Select the discovered resources to keep"),
                    )
                    .child(list)
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(add_selected)
                            .child(clear),
                    ),
            );
        }

        let mut configured = v_flex().gap_0();
        if resources.is_empty() {
            configured = configured.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("No custom resources configured yet."),
            );
        }
        for (index, def) in resources {
            let def = def.clone();
            let edit_view = view.clone();
            let edit_def = def.clone();
            let delete_view = view.clone();
            let delete_def = def.clone();
            configured = configured.child(
                h_flex()
                    .justify_between()
                    .gap_3()
                    .items_center()
                    .py_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        v_flex()
                            .gap_1()
                            .min_w_0()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(gpui_kit::FontWeight::MEDIUM)
                                    .child(SharedString::from(def.kind.clone())),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(SharedString::from(format!(
                                        "{} · {}",
                                        api_version(&def),
                                        if def.namespaced {
                                            "Namespaced"
                                        } else {
                                            "Cluster"
                                        }
                                    ))),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_1()
                            .items_center()
                            .child(
                                Button::new(format!("edit-custom-resource-{index}"))
                                    .ghost()
                                    .small()
                                    .icon(IconName::Pencil)
                                    .label("Edit")
                                    .on_click(move |_, window, cx| {
                                        open_definition_dialog(
                                            window,
                                            cx,
                                            edit_view.clone(),
                                            Some((index, edit_def.clone())),
                                        );
                                    }),
                            )
                            .child(
                                Button::new(format!("delete-custom-resource-{index}"))
                                    .ghost()
                                    .small()
                                    .danger()
                                    .icon(IconName::Trash)
                                    .label("Delete")
                                    .on_click(move |_, window, cx| {
                                        open_delete_dialog(
                                            window,
                                            cx,
                                            delete_view.clone(),
                                            index,
                                            delete_def.clone(),
                                        );
                                    }),
                            ),
                    ),
            );
        }

        root.child(
            v_flex()
                .gap_1()
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child("Configured resources"),
                )
                .child(configured),
        )
    }
}

fn api_version(def: &CustomResourceDef) -> String {
    if def.group.is_empty() {
        def.version.clone()
    } else {
        format!("{}/{}", def.group, def.version)
    }
}

fn open_definition_dialog(
    window: &mut Window,
    cx: &mut App,
    view: Entity<CustomResourcesView>,
    editing: Option<(u16, CustomResourceDef)>,
) {
    let edit_index = editing.as_ref().map(|(index, _)| *index);
    let def = editing.as_ref().map(|(_, def)| def.clone());
    let group = cx.new(|cx| {
        InputState::new(window, cx)
            .placeholder("example.com")
            .default_value(
                def.as_ref()
                    .map(|def| def.group.clone())
                    .unwrap_or_default(),
            )
    });
    let version = cx.new(|cx| {
        InputState::new(window, cx).placeholder("v1").default_value(
            def.as_ref()
                .map(|def| def.version.clone())
                .unwrap_or_else(|| "v1".to_string()),
        )
    });
    let kind = cx.new(|cx| {
        InputState::new(window, cx)
            .placeholder("MyResource")
            .default_value(def.as_ref().map(|def| def.kind.clone()).unwrap_or_default())
    });
    let plural = cx.new(|cx| {
        InputState::new(window, cx)
            .placeholder("myresources (optional)")
            .default_value(
                def.as_ref()
                    .map(|def| def.plural.clone())
                    .unwrap_or_default(),
            )
    });
    let namespaced = Rc::new(Cell::new(
        def.as_ref().map(|def| def.namespaced).unwrap_or(true),
    ));

    let title = if edit_index.is_some() {
        "Edit custom resource"
    } else {
        "Add custom resource"
    };
    let ok_text = if edit_index.is_some() { "Save" } else { "Add" };

    window.open_alert_dialog(cx, move |alert, _, _| {
        let namespaced_state = Rc::clone(&namespaced);
        let namespaced_click = Rc::clone(&namespaced);
        let namespaced_ok = Rc::clone(&namespaced);
        let group = group.clone();
        let version = version.clone();
        let kind = kind.clone();
        let plural = plural.clone();
        let result_view = view.clone();
        alert
            .title(title)
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
            .ok_text(ok_text)
            .show_cancel(true)
            .on_ok(move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                let definition = CustomResourceDef {
                    group: group.read(cx).value().to_string(),
                    version: version.read(cx).value().to_string(),
                    kind: kind.read(cx).value().to_string(),
                    plural: plural.read(cx).value().to_string(),
                    namespaced: namespaced_ok.get(),
                };
                let result = match edit_index {
                    Some(index) => update_definition(cx, index, definition),
                    None => add(cx, definition).map(|_| ()),
                };
                match result {
                    Ok(()) => {
                        let message = if edit_index.is_some() {
                            "Custom resource updated."
                        } else {
                            "Custom resource added."
                        };
                        result_view.update(cx, |this, cx| {
                            this.error = None;
                            this.notice = Some(message.to_string());
                            cx.notify();
                        });
                        true
                    }
                    Err(error) => {
                        result_view.update(cx, |this, cx| {
                            this.error = Some(error);
                            cx.notify();
                        });
                        false
                    }
                }
            })
    });
}

fn open_delete_dialog(
    window: &mut Window,
    cx: &mut App,
    view: Entity<CustomResourcesView>,
    index: u16,
    def: CustomResourceDef,
) {
    let kind = def.kind.clone();
    window.open_alert_dialog(cx, move |alert, _, _| {
        let result_view = view.clone();
        let deleted_def = def.clone();
        alert
            .title(format!("Delete custom resource “{kind}”?"))
            .description(
                "The resource will be removed from the sidebar and from the saved \
                 custom resource list. The CRD itself is not deleted from the cluster.",
            )
            .ok_text("Delete")
            .ok_variant(ButtonVariant::Danger)
            .show_cancel(true)
            .on_ok(move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                match delete_definition(cx, index) {
                    Ok(()) => {
                        result_view.update(cx, |this, cx| {
                            for item in &mut this.discovered {
                                if item.def == deleted_def {
                                    item.existing = false;
                                    item.selected = false;
                                }
                            }
                            this.error = None;
                            this.notice = Some("Custom resource deleted.".to_string());
                            cx.notify();
                        });
                        true
                    }
                    Err(error) => {
                        result_view.update(cx, |this, cx| {
                            this.error = Some(error);
                            cx.notify();
                        });
                        false
                    }
                }
            })
    });
}

fn group_input(input: &Entity<InputState>, label: &'static str) -> impl IntoElement {
    v_flex()
        .gap_1()
        .child(div().text_xs().child(label))
        .child(Input::new(input).small())
}
