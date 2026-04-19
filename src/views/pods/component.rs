use crate::components::{ResourceDetail, ResourceTable};
use crate::k8s::client::{self, K8sResource};
use crate::k8s::context::{use_is_connected, set_connected, CURRENT_NAMESPACE};
use crate::views::resource_list::ResourcePageHeader;
use dioxus::prelude::*;
use futures::StreamExt;
use std::collections::HashMap;
use kube::api::WatchEvent;

#[component]
pub fn PodsPage() -> Element {
    let is_connected = use_is_connected();
    let mut resources = use_signal(|| HashMap::new());
    let mut selected_resource = use_signal(|| Option::<K8sResource>::None);
    let resources_watch = resources.clone();
    
    use_effect(move || {
        let mut resources_watch = resources_watch.clone();
        let ns = CURRENT_NAMESPACE.signal().read().clone();
        
        spawn(async move {
            let namespace = if ns.is_empty() { None } else { Some(ns.as_str()) };
            let mut stream = client::watch_pods(namespace).await;
            resources_watch.write().clear();
            
            while let Some(event) = stream.next().await {
                match event {
                    WatchEvent::Added(obj) | WatchEvent::Modified(obj) => {
                        let resource: K8sResource = obj.into();
                        resources_watch.write().insert(resource.uid.clone().unwrap_or(resource.name.clone()), resource);
                    }
                    WatchEvent::Deleted(obj) => {
                        let resource: K8sResource = obj.into();
                        resources_watch.write().remove(&resource.uid.clone().unwrap_or(resource.name.clone()));
                    }
                    WatchEvent::Error(e) => {
                        tracing::error!("Watch error: {:?}", e);
                        set_connected(false);
                    }
                    WatchEvent::Bookmark(_) => {}
                }
            }
        });
    });
    
    let current_resources: Vec<K8sResource> = resources.read().values().cloned().collect();

    rsx! {
        div { class: "flex-1 flex flex-col",
            ResourcePageHeader { display_name: "Pods".to_string(), on_clear: move |_| resources.write().clear() }
            div { class: "flex-1 px-6 pb-6 overflow-auto",
                if !is_connected {
                    div { class: "text-red-500 text-center py-8", "Not connected to cluster" }
                } else if resources.read().is_empty() {
                    div { class: "text-gray-500 text-center py-8", "Watching for changes..." }
                } else {
                    ResourceTable { 
                        resources: current_resources, 
                        on_select: move |r| *selected_resource.write() = Some(r)
                    }
                }
            }
            if let Some(resource) = selected_resource.read().as_ref() {
                ResourceDetail { 
                    resource: resource.clone(),
                    on_close: move |_| *selected_resource.write() = None 
                }
            }
        }
    }
}
