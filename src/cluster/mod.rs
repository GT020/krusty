pub mod kubeconfig;

use crate::models::NamespaceScope;
use futures::TryStreamExt as _;
use gpui_kit::{App, BorrowAppContext as _, Global};
use k8s_openapi::api::core::v1::Namespace;
use kube::api::{ListParams, WatchEvent, WatchParams};
use kube::{Api, Client, Config};
use tokio::sync::watch;

/// One selectable cluster context: a kubectl context or a cluster added to krusty.
#[derive(Clone)]
pub struct ContextInfo {
    id: String,
    label: String,
    source: ContextSource,
}

#[derive(Clone)]
enum ContextSource {
    Kubeconfig(String),
    App(String),
}

impl ContextInfo {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn label(&self) -> &str {
        &self.label
    }
}

/// App-wide cluster connection state. Globals are read from any render.
#[derive(Clone)]
pub struct ClusterState {
    _runtime: std::sync::Arc<tokio::runtime::Runtime>,
    tokio: tokio::runtime::Handle,
    client: Option<Client>,
    kubeconfig: Option<kube::config::Kubeconfig>,
    contexts: Vec<ContextInfo>,
    active: Option<String>,
    store: kubeconfig::ConfigStore,
    reachable: Option<bool>,
    namespaces: Vec<String>,
    scope: NamespaceScope,
    context_epoch: u64,
    namespace_cancel: Option<watch::Sender<bool>>,
}

impl Global for ClusterState {}

impl ClusterState {
    pub fn context(&self) -> Option<&str> {
        let active = self.active.as_deref()?;
        self.contexts
            .iter()
            .find(|context| context.id == active)
            .map(ContextInfo::label)
    }

    pub fn contexts(&self) -> &[ContextInfo] {
        &self.contexts
    }

    pub fn active_id(&self) -> Option<&str> {
        self.active.as_deref()
    }

    /// `None` until the first API call confirms the connection.
    pub fn reachable(&self) -> Option<bool> {
        self.reachable
    }

    /// Bumped whenever the active connection changes. View-models watch this
    /// instead of every global update so fetch results do not trigger refetches.
    pub fn context_epoch(&self) -> u64 {
        self.context_epoch
    }

    pub fn client(&self) -> Option<Client> {
        self.client.clone()
    }

    pub fn tokio(&self) -> tokio::runtime::Handle {
        self.tokio.clone()
    }

    pub fn namespaces(&self) -> &[String] {
        &self.namespaces
    }

    pub fn scope(&self) -> &NamespaceScope {
        &self.scope
    }
}

/// Wire the global before the first window opens.
pub fn init(cx: &mut App) {
    let started = std::time::Instant::now();
    tracing::info!("cluster: init start");

    let runtime = std::sync::Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("failed to start async runtime"),
    );
    let tokio = runtime.handle().clone();
    tracing::debug!("cluster: async runtime ready");

    // kubectl-compatible source: $KUBECONFIG or ~/.kube/config.
    let kubeconfig = match kube::config::Kubeconfig::read() {
        Ok(kubeconfig) => {
            tracing::debug!(
                contexts = kubeconfig.contexts.len(),
                current = ?kubeconfig.current_context,
                "cluster: kubeconfig loaded"
            );
            Some(kubeconfig)
        }
        Err(error) => {
            tracing::debug!(%error, "cluster: no usable kubeconfig");
            None
        }
    };
    let store_started = std::time::Instant::now();
    let store = kubeconfig::ConfigStore::load();
    let contexts = build_contexts(kubeconfig.as_ref(), &store);
    tracing::debug!(
        elapsed_ms = store_started.elapsed().as_millis() as u64,
        app_clusters = store.entries().len(),
        contexts = contexts.len(),
        "cluster: context list built"
    );

    let preferred = store
        .last_context()
        .map(str::to_string)
        .filter(|id| contexts.iter().any(|context| context.id == *id))
        .or_else(|| {
            store
                .default_id()
                .map(|id| format!("app:{id}"))
                .filter(|id| contexts.iter().any(|context| context.id == *id))
        })
        .or_else(|| {
            kubeconfig
                .as_ref()
                .and_then(|config| config.current_context.as_ref())
                .map(|name| format!("kube:{name}"))
                .filter(|id| contexts.iter().any(|context| context.id == *id))
        })
        .or_else(|| contexts.first().map(|context| context.id.clone()));

    tracing::info!(preferred = ?preferred, "cluster: selected startup context");
    let client = preferred.as_deref().and_then(|id| {
        let context = contexts.iter().find(|context| context.id == id)?;
        match client_for(&tokio, kubeconfig.as_ref(), &context.source, &store) {
            Ok(client) => Some(client),
            Err(error) => {
                tracing::warn!(%error, context = %context.id, "cluster: startup client failed");
                None
            }
        }
    });

    let connected = client.is_some();
    cx.set_global(ClusterState {
        _runtime: runtime,
        tokio,
        client,
        kubeconfig,
        contexts,
        active: preferred,
        store,
        reachable: None,
        namespaces: Vec::new(),
        scope: NamespaceScope::All,
        context_epoch: 0,
        namespace_cancel: None,
    });
    refresh_namespaces(cx);
    tracing::info!(
        elapsed_ms = started.elapsed().as_millis() as u64,
        connected,
        "cluster: init complete"
    );
}

async fn run_namespace_watcher(
    client: Client,
    tx: watch::Sender<Vec<String>>,
    mut cancel: watch::Receiver<bool>,
) {
    let api: Api<Namespace> = Api::all(client);
    let list_params = ListParams::default();
    let watch_params = WatchParams::default().timeout(280);

    loop {
        if cancel.has_changed().is_err() || *cancel.borrow() {
            return;
        }

        let listed = tokio::select! {
            _ = cancel.changed() => return,
            result = api.list(&list_params) => result,
        };

        let list = match listed {
            Ok(list) => list,
            Err(error) => {
                tracing::warn!(%error, "cluster: namespace list failed, retrying");
                tokio::select! {
                    _ = cancel.changed() => return,
                    _ = tokio::time::sleep(std::time::Duration::from_millis(1500)) => continue,
                };
            }
        };

        let mut names = crate::repository::namespace_names(list.items);
        let mut resource_version = list.metadata.resource_version.unwrap_or_default();
        let _ = tx.send(names.clone());

        let watched = tokio::select! {
            _ = cancel.changed() => return,
            result = api.watch(&watch_params, &resource_version) => result,
        };

        let mut stream = match watched {
            Ok(stream) => Box::pin(stream),
            Err(error) => {
                tracing::warn!(%error, "cluster: namespace watch setup failed, retrying");
                tokio::select! {
                    _ = cancel.changed() => return,
                    _ = tokio::time::sleep(std::time::Duration::from_millis(1500)) => continue,
                };
            }
        };

        loop {
            let event = tokio::select! {
                _ = cancel.changed() => return,
                item = stream.try_next() => item,
            };

            match event {
                Ok(Some(WatchEvent::Added(ns))) => {
                    if let Some(name) = ns.metadata.name
                        && !names.contains(&name)
                    {
                        names.push(name);
                        names.sort_unstable();
                        let _ = tx.send(names.clone());
                    }
                    if let Some(version) = ns.metadata.resource_version {
                        resource_version = version;
                    }
                }
                Ok(Some(WatchEvent::Deleted(ns))) => {
                    if let Some(name) = ns.metadata.name
                        && let Some(idx) = names.iter().position(|n| n == &name)
                    {
                        names.remove(idx);
                        let _ = tx.send(names.clone());
                    }
                    if let Some(version) = ns.metadata.resource_version {
                        resource_version = version;
                    }
                }
                Ok(Some(WatchEvent::Bookmark(bm))) => {
                    resource_version = bm.metadata.resource_version;
                }
                Ok(Some(WatchEvent::Modified(_))) => {}
                Ok(Some(WatchEvent::Error(err))) => {
                    tracing::warn!(%err, "cluster: namespace watch error, relisting");
                    break;
                }
                Ok(None) => {
                    break;
                }
                Err(error) => {
                    tracing::warn!(%error, "cluster: namespace watch stream error, relisting");
                    break;
                }
            }
        }
    }
}

/// Start or restart the continuous namespace list/watch loop.
pub fn refresh_namespaces(cx: &mut App) {
    let state = cx.global::<ClusterState>();
    let Some(client) = state.client.clone() else {
        return;
    };
    let tokio = state.tokio.clone();

    if let Some(cancel) = &state.namespace_cancel {
        let _ = cancel.send(true);
    }

    let (cancel_tx, cancel_rx) = watch::channel(false);
    let (names_tx, mut names_rx) = watch::channel(state.namespaces.clone());

    cx.update_global::<ClusterState, _>(|state, _| {
        state.namespace_cancel = Some(cancel_tx);
    });

    tokio.spawn(run_namespace_watcher(client, names_tx, cancel_rx));

    cx.spawn(async move |cx: &mut gpui_kit::AsyncApp| {
        while names_rx.changed().await.is_ok() {
            let names = names_rx.borrow_and_update().clone();
            cx.update_global::<ClusterState, _>(|state, _| {
                state.namespaces = names.clone();
                state.reachable = Some(true);
                if let NamespaceScope::One(ref selected) = state.scope
                    && !names.contains(selected)
                {
                    state.scope = NamespaceScope::All;
                }
            });
        }
    })
    .detach();
}

/// Change the namespace filter used by every repository subscription.
pub fn set_namespace_scope(cx: &mut App, scope: NamespaceScope) {
    cx.update_global::<ClusterState, _>(|state, _| {
        if state.scope == scope {
            return;
        }
        state.scope = scope;
        state.context_epoch = state.context_epoch.wrapping_add(1);
    });
}

fn build_contexts(
    kubeconfig: Option<&kube::config::Kubeconfig>,
    store: &kubeconfig::ConfigStore,
) -> Vec<ContextInfo> {
    let mut contexts = Vec::new();
    let mut used_labels = std::collections::HashSet::new();

    if let Some(kubeconfig) = kubeconfig {
        for context in &kubeconfig.contexts {
            used_labels.insert(context.name.clone());
            contexts.push(ContextInfo {
                id: format!("kube:{}", context.name),
                label: context.name.clone(),
                source: ContextSource::Kubeconfig(context.name.clone()),
            });
        }
    }

    for entry in store.entries() {
        let label = if used_labels.contains(&entry.name) {
            format!("{} (krusty)", entry.name)
        } else {
            entry.name.clone()
        };
        used_labels.insert(label.clone());
        contexts.push(ContextInfo {
            id: format!("app:{}", entry.id),
            label,
            source: ContextSource::App(entry.id.clone()),
        });
    }

    contexts
}

fn client_for(
    tokio: &tokio::runtime::Handle,
    kubeconfig: Option<&kube::config::Kubeconfig>,
    source: &ContextSource,
    store: &kubeconfig::ConfigStore,
) -> Result<Client, &'static str> {
    let started = std::time::Instant::now();
    let source_label = match source {
        ContextSource::Kubeconfig(name) => format!("kube:{name}"),
        ContextSource::App(id) => format!("app:{id}"),
    };
    tracing::debug!(context = %source_label, "cluster: building kube client");
    let config = match source {
        ContextSource::Kubeconfig(name) => {
            let kubeconfig = kubeconfig.ok_or("kubeconfig unavailable")?;
            let opts = kube::config::KubeConfigOptions {
                context: Some(name.clone()),
                cluster: None,
                user: None,
            };
            tokio
                .block_on(Config::from_custom_kubeconfig(kubeconfig.clone(), &opts))
                .map_err(|_| "invalid kubeconfig context")?
        }
        ContextSource::App(id) => {
            let entry = store
                .entries()
                .iter()
                .find(|entry| entry.id == *id)
                .ok_or("unknown cluster")?;
            load_inline(tokio, &entry.kubeconfig_yaml)?
        }
    };
    let client = tokio
        .block_on(async { Client::try_from(config) })
        .map_err(|_| "connection failed")?;
    tracing::debug!(
        context = %source_label,
        elapsed_ms = started.elapsed().as_millis() as u64,
        "cluster: kube client ready"
    );
    Ok(client)
}

/// Switch the active cluster context. Selection is remembered across launches.
pub fn switch_context(cx: &mut App, id: &str) -> Result<(), &'static str> {
    let started = std::time::Instant::now();
    tracing::info!(context = %id, "cluster: switching context");
    let result = cx.update_global::<ClusterState, _>(|state, _| {
        let context = state
            .contexts
            .iter()
            .find(|context| context.id == id)
            .cloned()
            .ok_or("unknown context")?;
        let client = client_for(
            &state.tokio,
            state.kubeconfig.as_ref(),
            &context.source,
            &state.store,
        )?;
        state.client = Some(client);
        state.active = Some(context.id);
        state.reachable = None;
        state.namespaces.clear();
        state.scope = NamespaceScope::All;
        state.context_epoch = state.context_epoch.wrapping_add(1);
        if let Some(cancel) = state.namespace_cancel.take() {
            let _ = cancel.send(true);
        }
        state.store.set_last_context(Some(id));
        let _ = state.store.save();
        Ok(())
    });
    if result.is_ok() {
        refresh_namespaces(cx);
    }
    match &result {
        Ok(()) => tracing::info!(
            elapsed_ms = started.elapsed().as_millis() as u64,
            "cluster: context switched"
        ),
        Err(error) => tracing::error!(
            %error,
            elapsed_ms = started.elapsed().as_millis() as u64,
            "cluster: context switch failed"
        ),
    }
    result
}

fn rebuild_contexts(state: &mut ClusterState) {
    state.contexts = build_contexts(state.kubeconfig.as_ref(), &state.store);
    if let Some(active) = &state.active
        && !state.contexts.iter().any(|context| context.id == *active)
    {
        state.active = None;
        state.client = None;
        state.context_epoch = state.context_epoch.wrapping_add(1);
    }
}

fn load_inline(tokio: &tokio::runtime::Handle, yaml: &str) -> Result<Config, &'static str> {
    let kubeconfig: kube::config::Kubeconfig =
        serde_saphyr::from_str(yaml).map_err(|_| "invalid kubeconfig")?;
    tokio
        .block_on(Config::from_custom_kubeconfig(
            kubeconfig,
            &kube::config::KubeConfigOptions::default(),
        ))
        .map_err(|_| "invalid kubeconfig")
}

/// Cluster configs added directly to the app. Credentials are stored in the
/// app config folder (`~/.config/krusty/clusters.json`).
#[allow(dead_code)]
pub mod store {
    use super::*;

    pub fn add(
        cx: &mut App,
        id: &str,
        name: &str,
        kubeconfig_yaml: &str,
    ) -> Result<(), &'static str> {
        cx.update_global::<ClusterState, _>(|state, _| {
            state.store.upsert(id, name, kubeconfig_yaml);
            state.store.save()?;
            rebuild_contexts(state);
            Ok(())
        })
    }

    pub fn select(cx: &mut App, id: &str) -> Result<(), &'static str> {
        switch_context(cx, &format!("app:{id}"))?;
        cx.update_global::<ClusterState, _>(|state, _| {
            state.store.set_default(Some(id));
            state.store.save()
        })
    }

    pub fn remove(cx: &mut App, id: &str) -> Result<(), &'static str> {
        cx.update_global::<ClusterState, _>(|state, _| {
            state.store.remove(id);
            state.store.save()?;
            rebuild_contexts(state);
            Ok(())
        })
    }
}

/// Fetch real objects for a resource kind. Runs on the cluster runtime so the
#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL_KUBECONFIG: &str = r#"apiVersion: v1
kind: Config
clusters:
- name: test
  cluster:
    server: https://127.0.0.1:1
contexts:
- name: test
  context:
    cluster: test
    user: test
current-context: test
users:
- name: test
  user:
    token: test
"#;

    #[test]
    fn loads_kubectl_style_kubeconfig() {
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        let config = runtime
            .block_on(Config::from_kubeconfig(
                &kube::config::KubeConfigOptions::default(),
            ))
            .expect("kubeconfig resolves like kubectl");
        assert!(!config.cluster_url.to_string().is_empty());
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        assert!(runtime.block_on(async { Client::try_from(config) }).is_ok());
    }

    #[test]
    fn enumerates_kubeconfig_contexts() {
        let Ok(kubeconfig) = kube::config::Kubeconfig::read() else {
            return;
        };
        let contexts = build_contexts(Some(&kubeconfig), &kubeconfig::ConfigStore::default());
        assert_eq!(contexts.len(), kubeconfig.contexts.len());
        for context in &contexts {
            assert!(context.id().starts_with("kube:"));
            assert!(!context.label().is_empty());
        }
    }

    #[test]
    fn enumerates_app_store_contexts() {
        let mut store = kubeconfig::ConfigStore::default();
        store.upsert("dev", "kind-dev", MINIMAL_KUBECONFIG);
        let contexts = build_contexts(None, &store);
        assert_eq!(contexts.len(), 1);
        assert_eq!(contexts[0].id(), "app:dev");
        assert_eq!(contexts[0].label(), "kind-dev");

        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        let client = client_for(runtime.handle(), None, &contexts[0].source, &store);
        assert!(client.is_ok());
    }

    #[test]
    fn parses_inline_kubeconfig() {
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        let handle = runtime.handle().clone();
        let yaml = kube::config::Kubeconfig::read()
            .ok()
            .and_then(|kubeconfig| serde_saphyr::to_string(&kubeconfig).ok());
        let Some(yaml) = yaml else {
            return;
        };
        assert!(load_inline(&handle, &yaml).is_ok());
    }
}
