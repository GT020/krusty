use crate::models::ResourceKind;
use crate::ui::shell::nav::{self, NavTarget};
use gpui_kit::component::ActiveTheme;
use gpui_kit::component::sidebar::{
    Sidebar, SidebarCollapsible, SidebarFooter, SidebarGroup, SidebarMenuItem,
};
use gpui_kit::{Context, Entity, ParentElement as _, Styled as _, div};

fn nav_item(
    target: NavTarget,
    selected: NavTarget,
    entity: &Entity<crate::ui::Workspace>,
) -> SidebarMenuItem {
    let handle = entity.clone();
    nav::menu_item(target)
        .active(selected == target)
        .on_click(move |_, window, cx| {
            handle.update(cx, |workspace, cx| workspace.select(target, window, cx));
        })
}

pub fn render_sidebar(
    selected: NavTarget,
    entity: Entity<crate::ui::Workspace>,
    cx: &mut Context<crate::ui::Workspace>,
) -> Sidebar<SidebarGroup<SidebarMenuItem>> {
    let groups = ResourceKind::nav_groups()
        .into_iter()
        .map(|(label, kinds)| {
            SidebarGroup::new(label).children(
                kinds
                    .into_iter()
                    .map(|kind| nav_item(NavTarget::Resource(kind), selected, &entity)),
            )
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
                    .child("krusty v0.1"),
            ),
        )
        .children(groups)
}
