//! ServiceAccount detail view.

use gpui_kit::{AnyElement, Context, Entity};

use crate::models::ResourceKind;
use crate::models::resources::detail::ResourceDetailState;
use crate::ui::resources::detail_common::{
    count, fields_section, json_section, render_resource_detail, text,
};

pub fn render<T: 'static>(
    state: ResourceDetailState,
    view: Entity<T>,
    compact: bool,
    close: fn(&mut T, &mut Context<T>),
    cx: &mut Context<T>,
) -> AnyElement {
    render_resource_detail(
        ResourceKind::ServiceAccounts,
        state,
        view,
        compact,
        close,
        cx,
        |detail, cx| {
            let value = &detail.value;
            vec![
                fields_section(
                    "Overview",
                    vec![
                        ("Secrets", count(value.get("secrets")).to_string()),
                        (
                            "Image pull secrets",
                            count(value.get("imagePullSecrets")).to_string(),
                        ),
                        (
                            "Automount token",
                            text(value.get("automountServiceAccountToken")),
                        ),
                    ],
                    cx,
                ),
                json_section("Secrets", "secrets", value.get("secrets"), cx),
                json_section(
                    "Image pull secrets",
                    "imagePullSecrets",
                    value.get("imagePullSecrets"),
                    cx,
                ),
            ]
        },
    )
}
