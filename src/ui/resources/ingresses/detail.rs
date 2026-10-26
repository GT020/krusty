//! Ingress detail view.

use gpui_kit::{AnyElement, Context, Entity};

use crate::models::ResourceKind;
use crate::models::resources::detail::ResourceDetailState;
use crate::ui::resources::detail_common::{
    fields_section, json_section, path, render_resource_detail, text,
};

pub fn render<T: 'static>(
    state: ResourceDetailState,
    view: Entity<T>,
    compact: bool,
    close: fn(&mut T, &mut Context<T>),
    cx: &mut Context<T>,
) -> AnyElement {
    render_resource_detail(
        ResourceKind::Ingresses,
        state,
        view,
        compact,
        close,
        cx,
        |detail, cx| {
            let value = &detail.value;
            let spec = path(value, &["spec"]);
            let status = path(value, &["status"]);
            vec![
                fields_section(
                    "Overview",
                    vec![
                        (
                            "Ingress class",
                            text(spec.and_then(|spec| spec.get("ingressClassName"))),
                        ),
                        (
                            "Load balancer",
                            text(
                                path(value, &["status", "loadBalancer", "ingress"])
                                    .and_then(|ingress| ingress.get(0))
                                    .and_then(|ingress| {
                                        ingress.get("ip").or_else(|| ingress.get("hostname"))
                                    }),
                            ),
                        ),
                    ],
                    cx,
                ),
                json_section(
                    "Default backend",
                    "defaultBackend",
                    spec.and_then(|spec| spec.get("defaultBackend")),
                    cx,
                ),
                json_section(
                    "Rules",
                    "rules",
                    spec.and_then(|spec| spec.get("rules")),
                    cx,
                ),
                json_section("TLS", "tls", spec.and_then(|spec| spec.get("tls")), cx),
                json_section(
                    "Load balancer status",
                    "loadBalancer",
                    status.and_then(|status| status.get("loadBalancer")),
                    cx,
                ),
            ]
        },
    )
}
