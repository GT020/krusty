//! Service detail view.

use gpui_kit::{AnyElement, Context};

use crate::models::resources::detail::ResourceDetail;
use crate::ui::resources::detail_common::{
    conditions_section, fields_section, json_section, map_section, path, text,
};

pub fn sections<T: 'static>(detail: &ResourceDetail, cx: &mut Context<T>) -> Vec<AnyElement> {
    let value = &detail.value;
    let spec = path(value, &["spec"]);
    let status = path(value, &["status"]);
    let sections = vec![
        fields_section(
            "Overview",
            vec![
                ("Type", text(spec.and_then(|spec| spec.get("type")))),
                (
                    "Cluster IP",
                    text(spec.and_then(|spec| spec.get("clusterIP"))),
                ),
                (
                    "Cluster IPs",
                    text(spec.and_then(|spec| spec.get("clusterIPs"))),
                ),
                (
                    "External IPs",
                    text(spec.and_then(|spec| spec.get("externalIPs"))),
                ),
                (
                    "External name",
                    text(spec.and_then(|spec| spec.get("externalName"))),
                ),
                (
                    "Session affinity",
                    text(spec.and_then(|spec| spec.get("sessionAffinity"))),
                ),
                (
                    "External traffic policy",
                    text(spec.and_then(|spec| spec.get("externalTrafficPolicy"))),
                ),
                (
                    "Load balancer IP",
                    text(spec.and_then(|spec| spec.get("loadBalancerIP"))),
                ),
                (
                    "Load balancer class",
                    text(spec.and_then(|spec| spec.get("loadBalancerClass"))),
                ),
            ],
            cx,
        ),
        json_section(
            "Ports",
            "ports",
            spec.and_then(|spec| spec.get("ports")),
            cx,
        ),
        map_section("Selector", spec.and_then(|spec| spec.get("selector")), cx),
        json_section(
            "Load balancer",
            "loadBalancer",
            status.and_then(|status| status.get("loadBalancer")),
            cx,
        ),
        conditions_section(status.and_then(|status| status.get("conditions")), cx),
    ];
    sections
}
