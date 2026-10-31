//! Deployment action bar and the reusable actions widget.

use gpui_kit::assets::IconName;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::{
    Disableable as _, Sizable as _,
    button::{Button, ButtonVariants as _},
};
use gpui_kit::{
    AnyElement, App, AppContext as _, Entity, IntoElement, ParentElement as _, SharedString,
    Styled as _,
};

use crate::models::dashboard::{DeploymentWidgetSource, WidgetSource};
use crate::models::resources::deployments::actions::ActionStatus;
use crate::ui::components::actions::{FeedbackTone, FeedbackView, feedback_banner};
use crate::ui::dashboard::pinnable;
use crate::view_models::resources::deployments::actions::DeploymentActionsViewModel;

pub fn render_actions(
    vm: Entity<DeploymentActionsViewModel>,
    id_prefix: &str,
    cx: &App,
) -> AnyElement {
    let replicas = vm.read(cx).replicas();
    let feedback = vm.read(cx).feedback().cloned();

    let scale_vm = vm.clone();
    let scale = Button::new(format!("{id_prefix}-scale"))
        .ghost()
        .small()
        .icon(IconName::Scaling)
        .tooltip("Scale deployment")
        .disabled(replicas.is_none())
        .on_click(move |_, window, cx| {
            scale_vm.update(cx, |vm, cx| vm.scale(window, cx));
        });

    let restart_vm = vm.clone();
    let restart = Button::new(format!("{id_prefix}-restart"))
        .ghost()
        .small()
        .icon(IconName::RotateCcw)
        .tooltip("Restart deployment")
        .on_click(move |_, _, cx| {
            restart_vm.update(cx, |vm, cx| vm.restart(cx));
        });

    let edit_vm = vm.clone();
    let edit = Button::new(format!("{id_prefix}-edit"))
        .ghost()
        .small()
        .icon(IconName::Pencil)
        .tooltip("Edit deployment")
        .on_click(move |_, window, cx| {
            edit_vm.update(cx, |vm, cx| vm.edit(window, cx));
        });

    let namespace = vm.read(cx).namespace().to_string();
    let name = vm.read(cx).name().to_string();
    let delete_vm = vm;
    let delete = Button::new(format!("{id_prefix}-delete"))
        .ghost()
        .small()
        .text_color(cx.theme().danger)
        .icon(IconName::Trash)
        .tooltip("Delete deployment")
        .on_click(move |_, window, cx| {
            delete_vm.update(cx, |vm, cx| vm.delete(window, cx));
        });

    let mut body = v_flex().gap_2();
    if let Some(feedback) = feedback {
        let tone = match feedback.status() {
            ActionStatus::Pending => FeedbackTone::Pending,
            ActionStatus::Success => FeedbackTone::Success,
            ActionStatus::Error => FeedbackTone::Error,
        };
        body = body.child(feedback_banner(
            &format!("{id_prefix}-feedback"),
            FeedbackView {
                tone,
                message: feedback.message(),
            },
            cx,
        ));
    }
    let content = body
        .child(
            h_flex()
                .gap_1()
                .items_center()
                .child(scale)
                .child(restart)
                .child(edit)
                .child(delete),
        )
        .into_any_element();
    pinnable(
        format!("deployment-actions-menu-{id_prefix}"),
        content,
        WidgetSource::Deployment(DeploymentWidgetSource::Actions { namespace, name }),
    )
}

pub struct DeploymentActionsArgs {
    pub namespace: String,
    pub name: String,
    pub id_prefix: SharedString,
}

/// Reusable deployment actions component. Owns its summary and action state.
pub struct DeploymentActionsWidget {
    vm: Entity<DeploymentActionsViewModel>,
    id_prefix: SharedString,
}

impl DeploymentActionsWidget {
    pub fn mount(args: DeploymentActionsArgs, cx: &mut App) -> Self {
        let vm = cx.new(|cx| {
            DeploymentActionsViewModel::new(args.namespace.clone(), args.name.clone(), cx)
        });
        Self {
            vm,
            id_prefix: args.id_prefix,
        }
    }

    pub fn element(&self, cx: &App) -> AnyElement {
        render_actions(self.vm.clone(), &self.id_prefix, cx)
    }

    pub fn vm(&self) -> &Entity<DeploymentActionsViewModel> {
        &self.vm
    }

    pub fn shutdown(&self, cx: &mut App) {
        self.vm.update(cx, |vm, _| vm.shutdown());
    }
}
