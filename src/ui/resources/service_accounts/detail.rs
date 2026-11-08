//! ServiceAccount detail view.

use gpui_kit::{AnyElement, Context};

use crate::models::resources::detail::ResourceDetail;
use crate::ui::resources::detail_common::{count, fields_section, json_section, text};

pub fn sections<T: 'static>(detail: &ResourceDetail, cx: &mut Context<T>) -> Vec<AnyElement> {
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
}
