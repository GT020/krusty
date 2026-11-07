//! Per-tab resource filter: text query, case sensitivity, and regex mode.

use crate::models::ResourceFilter;
use gpui_kit::base::h_flex;
use gpui_kit::component::{
    ActiveTheme as _, Selectable as _, Sizable as _,
    button::{Button, ButtonVariants as _},
    input::{Input, InputEvent, InputState},
};
use gpui_kit::{
    App, AppContext as _, Context, Entity, EventEmitter, IntoElement, ParentElement as _,
    Styled as _, Subscription, Window, px,
};

/// Emitted whenever the query or one of its matching options changes.
#[derive(Clone, Debug)]
pub enum SearchFilterEvent {
    Changed(ResourceFilter),
}

pub struct SearchFilter {
    input: Entity<InputState>,
    filter: ResourceFilter,
    #[allow(dead_code)] // held for its Drop side-effect: unsubscribes the input event
    input_sub: Subscription,
}

impl EventEmitter<SearchFilterEvent> for SearchFilter {}

impl SearchFilter {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Filter resources"));
        let input_sub = cx.subscribe(&input, |this, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                this.refresh(cx);
            }
        });
        Self {
            input,
            filter: ResourceFilter::default(),
            input_sub,
        }
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        let query = self.input.read(cx).value().to_string();
        self.apply(
            ResourceFilter::new(query, self.filter.case_sensitive(), self.filter.regex()),
            cx,
        );
    }

    fn set_case_sensitive(&mut self, case_sensitive: bool, cx: &mut Context<Self>) {
        let query = self.input.read(cx).value().to_string();
        self.apply(
            ResourceFilter::new(query, case_sensitive, self.filter.regex()),
            cx,
        );
    }

    fn set_regex(&mut self, regex: bool, cx: &mut Context<Self>) {
        let query = self.input.read(cx).value().to_string();
        self.apply(
            ResourceFilter::new(query, self.filter.case_sensitive(), regex),
            cx,
        );
    }

    fn apply(&mut self, filter: ResourceFilter, cx: &mut Context<Self>) {
        if self.filter == filter {
            return;
        }
        self.filter = filter;
        cx.emit(SearchFilterEvent::Changed(self.filter.clone()));
        cx.notify();
    }
}

/// Render the tab-local resource filter.
pub fn render(filter: &Entity<SearchFilter>, cx: &App) -> impl IntoElement + use<> {
    let state = filter.read(cx);
    let case_sensitive = state.filter.case_sensitive();
    let regex = state.filter.regex();
    let invalid_regex = state.filter.error().is_some();

    let case_entity = filter.clone();
    let regex_entity = filter.clone();

    h_flex()
        .gap_1()
        .items_center()
        .child(Input::new(&state.input).small().w(px(180.)).cleanable(true))
        .child(
            Button::new("resource-filter-case")
                .ghost()
                .small()
                .label("Aa")
                .selected(case_sensitive)
                .toggled(case_sensitive)
                .on_click(move |_, _, cx| {
                    case_entity.update(cx, |this, cx| {
                        this.set_case_sensitive(!case_sensitive, cx);
                    });
                }),
        )
        .child(
            Button::new("resource-filter-regex")
                .ghost()
                .small()
                .label(".*")
                .selected(regex)
                .toggled(regex)
                .text_color(if invalid_regex {
                    cx.theme().danger
                } else {
                    cx.theme().foreground
                })
                .on_click(move |_, _, cx| {
                    regex_entity.update(cx, |this, cx| {
                        this.set_regex(!regex, cx);
                    });
                }),
        )
}
