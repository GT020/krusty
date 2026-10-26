use crate::cluster::ClusterState;
use crate::models::ResourceKind;
use gpui_kit::base::h_flex;
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::sidebar::{
    Sidebar, SidebarCollapsible, SidebarFooter, SidebarGroup, SidebarMenuItem,
};
use gpui_kit::component::{
    ActiveTheme, IconName, Sizable as _,
    button::{Button, ButtonVariants as _},
};
use gpui_kit::{Context, Entity, ParentElement as _, Styled as _, div};
use std::rc::Rc;

pub fn render_sidebar(
    selected: ResourceKind,
    entity: Entity<crate::ui::Workspace>,
    cx: &mut Context<crate::ui::Workspace>,
) -> Sidebar<SidebarGroup<SidebarMenuItem>> {
    let groups = ResourceKind::nav_groups()
        .into_iter()
        .map(|(label, kinds)| {
            let entity = entity.clone();
            SidebarGroup::new(label).children(kinds.into_iter().map(move |kind| {
                let handle = entity.clone();
                kind.menu_item().active(selected == kind).on_click({
                    let _rc: Rc<()> = Rc::new(());
                    move |_, window, cx| {
                        handle.update(cx, |workspace, cx| workspace.select_kind(kind, window, cx));
                    }
                })
            }))
        })
        .collect::<Vec<_>>();

    let state = cx.global::<ClusterState>();
    let contexts = state.contexts().to_vec();
    let active = state.active_id().map(str::to_string);
    let label = state.context().unwrap_or("No context").to_string();
    let namespaces = state.namespaces().to_vec();
    let scope = state.scope().clone();
    let scope_label = if scope.is_all() {
        "All namespaces".to_string()
    } else {
        scope.label()
    };
    tracing::trace!(
        contexts = contexts.len(),
        namespaces = namespaces.len(),
        active = ?active,
        "ui: render sidebar"
    );

    Sidebar::new("nav")
        .collapsible(SidebarCollapsible::Icon)
        .header(
            h_flex()
                .gap_1()
                .p_2()
                .child(
                    Button::new("context")
                        .ghost()
                        .small()
                        .label(label)
                        .icon(IconName::ChevronsUpDown)
                        .dropdown_menu(move |mut menu, _, cx| {
                            let state = cx.global::<ClusterState>();
                            let contexts = state.contexts().to_vec();
                            let active = state.active_id().map(str::to_string);
                            if contexts.is_empty() {
                                return menu
                                    .item(PopupMenuItem::new("No contexts found").disabled(true));
                            }
                            for context in &contexts {
                                let id = context.id().to_string();
                                let selected = active.as_deref() == Some(context.id());
                                menu = menu.item(
                                    PopupMenuItem::new(context.label().to_string())
                                        .checked(selected)
                                        .on_click(move |_, _, cx| {
                                            if let Err(error) =
                                                crate::cluster::switch_context(cx, &id)
                                            {
                                                tracing::error!(
                                                    context = %id,
                                                    %error,
                                                    "ui: switch context failed"
                                                );
                                            }
                                        }),
                                );
                            }
                            menu
                        }),
                )
                .child(
                    Button::new("namespace")
                        .ghost()
                        .small()
                        .label(scope_label)
                        .icon(IconName::ChevronsUpDown)
                        .dropdown_menu(move |mut menu, _, cx| {
                            use crate::models::NamespaceScope;
                            let state = cx.global::<ClusterState>();
                            let namespaces = state.namespaces().to_vec();
                            let scope = state.scope().clone();
                            menu = menu.item(
                                PopupMenuItem::new("All namespaces")
                                    .checked(scope.is_all())
                                    .on_click(move |_, _, cx| {
                                        crate::cluster::set_namespace_scope(
                                            cx,
                                            NamespaceScope::All,
                                        );
                                    }),
                            );
                            for namespace in &namespaces {
                                let id = namespace.clone();
                                let selected = scope.namespace() == Some(namespace.as_str());
                                menu = menu.item(
                                    PopupMenuItem::new(namespace.clone())
                                        .checked(selected)
                                        .on_click(move |_, _, cx| {
                                            crate::cluster::set_namespace_scope(
                                                cx,
                                                NamespaceScope::One(id.clone()),
                                            );
                                        }),
                                );
                            }
                            menu
                        }),
                ),
        )
        .footer(
            SidebarFooter::new().child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("krusty v0.1"),
            ),
        )
        .children(groups)
}
