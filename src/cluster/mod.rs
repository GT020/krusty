pub mod kubeconfig;

use std::time::Duration;

use gpui_kit::{App, BorrowAppContext as _, Global};
use kube::{Client, Config};
use tokio::sync::watch;

use crate::models::{NamespaceScope, ResourceKind, SnapshotPhase};

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
    context_epoch: u64,
    namespace_cancel: Option<watch::Sender<bool>>,
    /// Kept alive for the app lifetime: the selector's handle on the shared
    /// namespaces watcher that the Namespaces page also reads.
    namespace_subscription: Option<crate::repository::Subscription>,
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
    let startup = preferred
        .as_deref()
        .and_then(|id| contexts.iter().find(|context| context.id == id).cloned());

    cx.set_global(ClusterState {
        _runtime: runtime,
        tokio,
        client: None,
        kubeconfig,
        contexts,
        active: preferred,
        store,
        reachable: None,
        namespaces: Vec::new(),
        context_epoch: 0,
        namespace_cancel: None,
        namespace_subscription: None,
    });
    // Building the client can run an auth plugin (kubelogin, gcloud, …):
    // keep it off the UI thread so startup stays instant.
    if let Some(context) = startup {
        spawn_client_build(cx, context);
    }
    tracing::info!(
        elapsed_ms = started.elapsed().as_millis() as u64,
        "cluster: init complete"
    );
}

/// Start or restart the shared namespace subscription. The selector and the
/// Namespaces resource page read from the same repository watcher, so
/// namespaces are fetched once per context and follow the saved poll rate.
pub fn refresh_namespaces(cx: &mut App) {
    if cx.global::<ClusterState>().client.is_none() {
        return;
    }
    // Stop the previous pump and release the previous handle; dropping the
    // last handle cancels the shared watcher task.
    if let Some(cancel) = &cx.global::<ClusterState>().namespace_cancel {
        let _ = cancel.send(true);
    }

    let subscription =
        match crate::repository::subscribe(cx, ResourceKind::Namespaces, NamespaceScope::All) {
            Ok(subscription) => subscription,
            Err(error) => {
                tracing::debug!(%error, "cluster: namespace subscription unavailable");
                return;
            }
        };

    let (cancel_tx, mut cancel_rx) = watch::channel(false);
    cx.update_global::<ClusterState, _>(|state, _| {
        state.namespace_cancel = Some(cancel_tx);
        state.namespace_subscription = Some(subscription.clone());
    });

    let mut subscription = subscription;
    cx.spawn(async move |cx: &mut gpui_kit::AsyncApp| {
        loop {
            let snapshot = tokio::select! {
                _ = cancel_rx.changed() => break,
                snapshot = subscription.next() => snapshot,
            };
            let Some(snapshot) = snapshot else {
                break;
            };
            let names: Vec<String> = snapshot
                .items()
                .iter()
                .map(|item| item.name.clone())
                .collect();
            cx.update_global::<ClusterState, _>(|state, _| {
                // An empty list means the watcher has not loaded yet (or the
                // context is switching); keep the previous selector contents.
                if !names.is_empty() {
                    state.namespaces = names;
                }
                match snapshot.phase() {
                    SnapshotPhase::Ready => state.reachable = Some(true),
                    SnapshotPhase::Error(_) => state.reachable = Some(false),
                    SnapshotPhase::Loading => {}
                }
            });
        }
    })
    .detach();
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

/// Build a kube client without blocking the caller. Auth plugins (kubelogin,
/// gcloud, aws, …) can take seconds, so this always runs on the cluster
/// runtime, never the UI thread.
async fn build_client(
    kubeconfig: Option<kube::config::Kubeconfig>,
    source: ContextSource,
    store: kubeconfig::ConfigStore,
) -> Result<Client, &'static str> {
    let started = std::time::Instant::now();
    let source_label = match &source {
        ContextSource::Kubeconfig(name) => format!("kube:{name}"),
        ContextSource::App(id) => format!("app:{id}"),
    };
    tracing::debug!(context = %source_label, "cluster: building kube client");
    let mut config = match &source {
        ContextSource::Kubeconfig(name) => {
            let kubeconfig = kubeconfig.ok_or("kubeconfig unavailable")?;
            let opts = kube::config::KubeConfigOptions {
                context: Some(name.clone()),
                cluster: None,
                user: None,
            };
            Config::from_custom_kubeconfig(kubeconfig, &opts)
                .await
                .map_err(|_| "invalid kubeconfig context")?
        }
        ContextSource::App(id) => {
            let entry = store
                .entries()
                .iter()
                .find(|entry| entry.id == *id)
                .ok_or("unknown cluster")?;
            load_inline(&entry.kubeconfig_yaml).await?
        }
    };
    // kube defaults to 30s; fail dead endpoints faster and let the retry
    // policy back off instead of stalling the UI.
    config.connect_timeout = Some(Duration::from_secs(10));
    let client = Client::try_from(config).map_err(|_| "connection failed")?;
    tracing::debug!(
        context = %source_label,
        elapsed_ms = started.elapsed().as_millis() as u64,
        "cluster: kube client ready"
    );
    Ok(client)
}

/// Build the client for `context` in the background and publish it when ready.
/// The epoch bump resubscribes every resource view; namespace polling starts
/// here too. Stale builds (superseded by a newer switch) are discarded.
fn spawn_client_build(cx: &mut App, context: ContextInfo) {
    let (kubeconfig, store, tokio) = {
        let state = cx.global::<ClusterState>();
        (
            state.kubeconfig.clone(),
            state.store.clone(),
            state.tokio.clone(),
        )
    };
    let context_id = context.id.clone();
    let started = std::time::Instant::now();
    let building = tokio.spawn(build_client(kubeconfig, context.source, store));
    cx.spawn(async move |cx: &mut gpui_kit::AsyncApp| {
        let built = building.await;
        cx.update(|cx| {
            let state = cx.global::<ClusterState>();
            if state.active_id() != Some(context_id.as_str()) {
                tracing::debug!(context = %context_id, "cluster: discarding stale client build");
                return;
            }
            match built {
                Ok(Ok(client)) => {
                    tracing::info!(
                        context = %context_id,
                        elapsed_ms = started.elapsed().as_millis() as u64,
                        "cluster: client ready"
                    );
                    cx.update_global::<ClusterState, _>(|state, _| {
                        state.client = Some(client);
                        state.context_epoch = state.context_epoch.wrapping_add(1);
                    });
                    refresh_namespaces(cx);
                }
                Ok(Err(error)) => {
                    tracing::warn!(%error, context = %context_id, "cluster: client build failed");
                    cx.update_global::<ClusterState, _>(|state, _| {
                        state.reachable = Some(false);
                    });
                }
                Err(error) => {
                    tracing::error!(%error, context = %context_id, "cluster: client build task failed");
                    cx.update_global::<ClusterState, _>(|state, _| {
                        state.reachable = Some(false);
                    });
                }
            }
        });
    })
    .detach();
}

/// Switch the active cluster context. Selection is remembered across launches.
/// The new client is built in the background; views resubscribe when it lands.
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
        state.client = None;
        state.active = Some(context.id.clone());
        state.reachable = None;
        state.namespaces.clear();
        state.context_epoch = state.context_epoch.wrapping_add(1);
        if let Some(cancel) = state.namespace_cancel.take() {
            let _ = cancel.send(true);
        }
        state.namespace_subscription = None;
        state.store.set_last_context(Some(id));
        let _ = state.store.save();
        Ok(context)
    });
    match result {
        Ok(context) => {
            spawn_client_build(cx, context);
            tracing::info!(
                elapsed_ms = started.elapsed().as_millis() as u64,
                "cluster: context switched, client building"
            );
            Ok(())
        }
        Err(error) => {
            tracing::error!(
                %error,
                elapsed_ms = started.elapsed().as_millis() as u64,
                "cluster: context switch failed"
            );
            Err(error)
        }
    }
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

async fn load_inline(yaml: &str) -> Result<Config, &'static str> {
    let kubeconfig: kube::config::Kubeconfig =
        serde_saphyr::from_str(yaml).map_err(|_| "invalid kubeconfig")?;
    Config::from_custom_kubeconfig(kubeconfig, &kube::config::KubeConfigOptions::default())
        .await
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
        let client = runtime.block_on(build_client(None, contexts[0].source.clone(), store));
        assert!(client.is_ok());
    }

    #[test]
    fn parses_inline_kubeconfig() {
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        let yaml = kube::config::Kubeconfig::read()
            .ok()
            .and_then(|kubeconfig| serde_saphyr::to_string(&kubeconfig).ok());
        let Some(yaml) = yaml else {
            return;
        };
        assert!(runtime.block_on(load_inline(&yaml)).is_ok());
    }
}
