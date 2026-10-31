use crate::models::ResourceKind;
use crate::ui::shell::nav::{self, NavTarget};
use gpui_kit::component::ActiveTheme;
use gpui_kit::component::menu::PopupMenuItem;
use gpui_kit::component::sidebar::{
    Sidebar, SidebarCollapsible, SidebarFooter, SidebarGroup, SidebarMenuItem,
};
use gpui_kit::{Context, Entity, ParentElement as _, Styled as _, div};

fn nav_item(
    target: NavTarget,
    selected: Option<NavTarget>,
    entity: &Entity<crate::ui::Workspace>,
) -> SidebarMenuItem {
    let click_handle = entity.clone();
    let menu_handle = entity.clone();
    nav::menu_item(target)
        .active(selected == Some(target))
        .on_click(move |event, window, cx| {
            let new_tab = event.modifiers().secondary();
            click_handle.update(cx, |workspace, cx| {
                workspace.select(target, new_tab, window, cx);
            });
        })
        .context_menu(move |menu, _, _| {
            let open_handle = menu_handle.clone();
            let new_tab_handle = menu_handle.clone();
            let menu = menu.item(PopupMenuItem::new("Open").on_click(move |_, window, cx| {
                open_handle.update(cx, |workspace, cx| {
                    workspace.select(target, false, window, cx);
                });
            }));
            if target.is_singleton() {
                return menu;
            }
            menu.item(
                PopupMenuItem::new("Open in new tab").on_click(move |_, window, cx| {
                    new_tab_handle.update(cx, |workspace, cx| {
                        workspace.select(target, true, window, cx);
                    });
                }),
            )
        })
}

pub fn render_sidebar(
    selected: Option<NavTarget>,
    entity: Entity<crate::ui::Workspace>,
    cx: &mut Context<crate::ui::Workspace>,
) -> Sidebar<SidebarGroup<SidebarMenuItem>> {
    let groups = ResourceKind::nav_groups()
        .into_iter()
        .enumerate()
        .map(|(index, (label, kinds))| {
            let mut items = kinds
                .into_iter()
                .map(|kind| nav_item(NavTarget::Resource(kind), selected, &entity))
                .collect::<Vec<_>>();
            if index == 0 {
                items.insert(0, nav_item(NavTarget::Overview, selected, &entity));
            }
            SidebarGroup::new(label).children(items)
        })
        .chain(std::iter::once(
            SidebarGroup::new("Application").child(nav_item(
                NavTarget::Settings,
                selected,
                &entity,
            )),
        ))
        .collect::<Vec<_>>();

    Sidebar::new("nav")
        .collapsible(SidebarCollapsible::Icon)
        .footer(
            SidebarFooter::new().child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(concat!("krusty v", env!("CARGO_PKG_VERSION"))),
            ),
        )
        .children(groups)
}
