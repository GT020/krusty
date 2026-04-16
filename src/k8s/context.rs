use dioxus::prelude::*;

pub static CURRENT_NAMESPACE: GlobalSignal<String> = GlobalSignal::new(String::new);
pub static CURRENT_CONTEXT: GlobalSignal<String> = GlobalSignal::new(String::new);
pub static IS_CONNECTED: GlobalSignal<bool> = GlobalSignal::new(|| false);

pub fn set_current_namespace(ns: String) {
    CURRENT_NAMESPACE.with_mut(|n| *n = ns);
}

pub fn get_current_namespace() -> String {
    CURRENT_NAMESPACE.read().clone()
}

pub fn set_current_context(ctx: String) {
    CURRENT_CONTEXT.with_mut(|c| *c = ctx);
}

pub fn get_current_context() -> String {
    CURRENT_CONTEXT.read().clone()
}

pub fn use_is_connected() -> bool {
    *IS_CONNECTED.read()
}

pub fn set_connected(connected: bool) {
    IS_CONNECTED.with_mut(|c| *c = connected);
}
