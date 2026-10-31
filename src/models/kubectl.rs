//! Parsed `kubectl` commands used by the workspace command bar.
//!
//! Only navigation commands are supported: `get`, `describe`, and `logs`.
//! Parsing is kept free of GPUI so it can be tested directly.

use crate::models::NamespaceScope;
use crate::models::resources::catalog::{self, ResourceKind};

/// A command that the workspace can turn into a tab, detail pane, or log pane.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KubectlCommand {
    pub action: KubectlAction,
    pub namespace: NamespaceScope,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KubectlAction {
    /// Open the resource list for `kind`.
    Get {
        kind: ResourceKind,
        name: Option<String>,
    },
    /// Open the detail pane for one resource.
    Describe { kind: ResourceKind, name: String },
    /// Open the log panel for a pod.
    Logs {
        pod: String,
        container: Option<String>,
        follow: bool,
    },
}

/// Parse a pasted kubectl command. The leading `kubectl` (or `k`) is optional.
pub fn parse(input: &str) -> Result<KubectlCommand, String> {
    let tokens = tokenize(input)?;
    let start = match tokens.first() {
        Some(token) if token.eq_ignore_ascii_case("kubectl") || token.eq_ignore_ascii_case("k") => {
            1
        }
        Some(token)
            if matches!(
                token.to_ascii_lowercase().as_str(),
                "get" | "describe" | "logs"
            ) =>
        {
            0
        }
        Some(_) => {
            return Err("Start the command with kubectl, e.g. kubectl get pods -n default.".into());
        }
        None => return Err("Paste a kubectl command to navigate.".into()),
    };
    let command = tokens
        .get(start)
        .map(|token| token.to_ascii_lowercase())
        .ok_or_else(|| "Missing a kubectl command, e.g. get, describe, or logs.".to_string())?;
    let rest = &tokens[start + 1..];

    match command.as_str() {
        "get" => parse_get(rest),
        "describe" => parse_describe(rest),
        "logs" => parse_logs(rest),
        other => Err(format!(
            "Unsupported kubectl command “{other}”. Try get, describe, or logs."
        )),
    }
}

fn parse_get(tokens: &[String]) -> Result<KubectlCommand, String> {
    let args = parse_flags(tokens)?;
    let (kind, name) = parse_resource_positionals(&args.positionals)?;
    Ok(KubectlCommand {
        action: KubectlAction::Get { kind, name },
        namespace: args.namespace_scope(),
    })
}

fn parse_describe(tokens: &[String]) -> Result<KubectlCommand, String> {
    let args = parse_flags(tokens)?;
    // Accept the shorthand `kubectl describe my-pod -n cas` by treating a
    // lone, non-resource token as a pod name.
    let (kind, name) = if args.positionals.len() == 1
        && !args.positionals[0].contains('/')
        && resource_kind(&args.positionals[0]).is_none()
    {
        (ResourceKind::Pods, Some(args.positionals[0].clone()))
    } else {
        parse_resource_positionals(&args.positionals)?
    };
    let name = name.ok_or_else(|| {
        "describe needs a resource name, e.g. kubectl describe pod my-pod -n default.".to_string()
    })?;
    Ok(KubectlCommand {
        action: KubectlAction::Describe { kind, name },
        namespace: args.namespace_scope(),
    })
}

fn parse_logs(tokens: &[String]) -> Result<KubectlCommand, String> {
    let args = parse_flags(tokens)?;
    let first = args
        .positionals
        .first()
        .ok_or_else(|| "logs needs a pod name, e.g. kubectl logs my-pod -n default.".to_string())?;
    let pod = if let Some((kind, name)) = first.split_once('/') {
        if !is_pod_kind(kind) {
            return Err(format!(
                "logs works with pods, not “{kind}”. Try a pod name or pod/{first}."
            ));
        }
        name.to_string()
    } else if is_pod_kind(first) && args.positionals.len() > 1 {
        args.positionals[1].clone()
    } else {
        first.clone()
    };
    if pod.is_empty() {
        return Err("logs needs a pod name.".to_string());
    }
    let namespace = args.namespace_scope();
    Ok(KubectlCommand {
        action: KubectlAction::Logs {
            pod,
            container: args.container,
            follow: args.follow,
        },
        namespace,
    })
}

fn parse_resource_positionals(
    positionals: &[String],
) -> Result<(ResourceKind, Option<String>), String> {
    let first = positionals
        .first()
        .ok_or_else(|| "Expected a resource type, e.g. kubectl get pods.".to_string())?;
    let (kind_token, inline_name) = match first.split_once('/') {
        Some((kind, name)) => (kind, Some(name.to_string())),
        None => (first.as_str(), None),
    };
    let kind = resource_kind(kind_token).ok_or_else(|| {
        format!(
            "Unknown resource type “{kind_token}”. Try pods, deployments, services, or another resource."
        )
    })?;
    let name = inline_name.or_else(|| positionals.get(1).cloned());
    Ok((kind, name))
}

struct ParsedFlags {
    namespace: Option<String>,
    all_namespaces: bool,
    container: Option<String>,
    follow: bool,
    positionals: Vec<String>,
}

impl ParsedFlags {
    fn namespace_scope(&self) -> NamespaceScope {
        if self.all_namespaces {
            return NamespaceScope::All;
        }
        match &self.namespace {
            Some(namespace) => NamespaceScope::One(namespace.clone()),
            None => NamespaceScope::All,
        }
    }
}

fn parse_flags(tokens: &[String]) -> Result<ParsedFlags, String> {
    let mut parsed = ParsedFlags {
        namespace: None,
        all_namespaces: false,
        container: None,
        follow: false,
        positionals: Vec::new(),
    };
    let mut index = 0;
    while index < tokens.len() {
        let token = &tokens[index];
        match token.as_str() {
            "-n" | "--namespace" => {
                index += 1;
                parsed.namespace = Some(flag_value(tokens, index, "namespace")?);
            }
            "-A" | "--all-namespaces" => parsed.all_namespaces = true,
            "-c" | "--container" => {
                index += 1;
                parsed.container = Some(flag_value(tokens, index, "container")?);
            }
            "-f" | "--follow" => parsed.follow = true,
            _ if token.starts_with("--namespace=") => {
                parsed.namespace = Some(non_empty(&token["--namespace=".len()..], "namespace")?);
            }
            _ if token.starts_with("-n=") => {
                parsed.namespace = Some(non_empty(&token[3..], "namespace")?);
            }
            _ if token.starts_with("--container=") => {
                parsed.container = Some(non_empty(&token["--container=".len()..], "container")?);
            }
            _ if token.starts_with("-c=") => {
                parsed.container = Some(non_empty(&token[3..], "container")?);
            }
            _ if token.starts_with("-n") && token.len() > 2 => {
                parsed.namespace = Some(non_empty(&token[2..], "namespace")?);
            }
            _ if token.starts_with("-c") && token.len() > 2 => {
                parsed.container = Some(non_empty(&token[2..], "container")?);
            }
            _ if token.starts_with('-') => {
                if flag_takes_value(token) {
                    index += 1;
                }
            }
            _ => parsed.positionals.push(token.clone()),
        }
        index += 1;
    }
    Ok(parsed)
}

fn flag_value(tokens: &[String], index: usize, name: &str) -> Result<String, String> {
    tokens
        .get(index)
        .map(|value| non_empty(value, name))
        .transpose()?
        .ok_or_else(|| format!("Missing a value for --{name}."))
}

fn non_empty(value: &str, name: &str) -> Result<String, String> {
    if value.is_empty() {
        Err(format!("Missing a value for --{name}."))
    } else {
        Ok(value.to_string())
    }
}

/// Flags that take a following value and are irrelevant to navigation.
fn flag_takes_value(flag: &str) -> bool {
    matches!(
        flag,
        "-o" | "--output"
            | "-l"
            | "--selector"
            | "--context"
            | "--kubeconfig"
            | "--cluster"
            | "--user"
            | "--server"
            | "--tail"
            | "--since"
            | "--since-time"
            | "--limit-bytes"
            | "--pod-running-timeout"
            | "--field-selector"
            | "--sort-by"
            | "--chunk-size"
            | "--request-timeout"
            | "--as"
            | "--as-group"
    )
}

/// Split a command line into arguments, honoring single and double quotes.
fn tokenize(input: &str) -> Result<Vec<String>, String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut escaped = false;
    let mut started = false;

    for character in input.chars() {
        if escaped {
            current.push(character);
            escaped = false;
            started = true;
            continue;
        }
        match quote {
            Some(closing) if character == closing => quote = None,
            Some(_) => {
                if character == '\\' {
                    escaped = true;
                } else {
                    current.push(character);
                }
                started = true;
            }
            None => match character {
                '\\' => {
                    escaped = true;
                    started = true;
                }
                '\'' | '"' => {
                    quote = Some(character);
                    started = true;
                }
                character if character.is_whitespace() => {
                    if started {
                        tokens.push(std::mem::take(&mut current));
                        started = false;
                    }
                }
                character => {
                    current.push(character);
                    started = true;
                }
            },
        }
    }

    if escaped {
        current.push('\\');
    }
    if quote.is_some() {
        return Err("The command has an unclosed quote.".into());
    }
    if started {
        tokens.push(current);
    }
    Ok(tokens)
}

fn is_pod_kind(token: &str) -> bool {
    resource_kind(token) == Some(ResourceKind::Pods)
}

fn resource_kind(token: &str) -> Option<ResourceKind> {
    let token = token.to_ascii_lowercase();
    // Accept fully qualified forms such as `deployments.apps`.
    if let Some(kind) = resource_kind_name(&token) {
        return Some(kind);
    }
    let base = token.split('.').next().unwrap_or(&token);
    resource_kind_name(base)
}

/// Resolve an unqualified resource token: aliases first, then every built-in
/// label, then user-defined resources.
fn resource_kind_name(token: &str) -> Option<ResourceKind> {
    let kind = match token {
        "pod" | "pods" | "po" => ResourceKind::Pods,
        "deployment" | "deployments" | "deploy" => ResourceKind::Deployments,
        "statefulset" | "statefulsets" | "sts" => ResourceKind::StatefulSets,
        "replicaset" | "replicasets" | "rs" => ResourceKind::ReplicaSets,
        "replicationcontroller" | "replicationcontrollers" | "rc" => {
            ResourceKind::ReplicationControllers
        }
        "daemonset" | "daemonsets" | "ds" => ResourceKind::DaemonSets,
        "job" | "jobs" => ResourceKind::Jobs,
        "cronjob" | "cronjobs" | "cj" => ResourceKind::CronJobs,
        "horizontalpodautoscaler" | "horizontalpodautoscalers" | "hpa" => {
            ResourceKind::HorizontalPodAutoscalers
        }
        "poddisruptionbudget" | "poddisruptionbudgets" | "pdb" => {
            ResourceKind::PodDisruptionBudgets
        }
        "service" | "services" | "svc" => ResourceKind::Services,
        "ingress" | "ingresses" | "ing" => ResourceKind::Ingresses,
        "endpoint" | "endpoints" | "ep" => ResourceKind::Endpoints,
        "endpointslice" | "endpointslices" => ResourceKind::EndpointSlices,
        "networkpolicy" | "networkpolicies" | "netpol" => ResourceKind::NetworkPolicies,
        "persistentvolume" | "persistentvolumes" | "pv" => ResourceKind::PersistentVolumes,
        "persistentvolumeclaim" | "persistentvolumeclaims" | "pvc" => {
            ResourceKind::PersistentVolumeClaims
        }
        "storageclass" | "storageclasses" | "sc" => ResourceKind::StorageClasses,
        "configmap" | "configmaps" | "cm" => ResourceKind::ConfigMaps,
        "secret" | "secrets" => ResourceKind::Secrets,
        "serviceaccount" | "serviceaccounts" | "sa" => ResourceKind::ServiceAccounts,
        "namespace" | "namespaces" | "ns" => ResourceKind::Namespaces,
        "node" | "nodes" | "no" => ResourceKind::Nodes,
        "lease" | "leases" => ResourceKind::Leases,
        "event" | "events" | "ev" => ResourceKind::Events,
        "clusterrole" | "clusterroles" => ResourceKind::ClusterRoles,
        "role" | "roles" => ResourceKind::Roles,
        "rolebinding" | "rolebindings" => ResourceKind::RoleBindings,
        _ => {
            return catalog::builtin_kinds()
                .iter()
                .copied()
                .find(|kind| {
                    let label = kind.label().to_ascii_lowercase();
                    label == token || label.strip_suffix('s') == Some(token)
                })
                .or_else(|| custom_resource_kind(token));
        }
    };
    Some(kind)
}

fn custom_resource_kind(token: &str) -> Option<ResourceKind> {
    catalog::custom_resource_entries()
        .into_iter()
        .find(|(_, def)| {
            def.plural.eq_ignore_ascii_case(token) || def.kind.eq_ignore_ascii_case(token)
        })
        .map(|(index, _)| ResourceKind::Custom(index))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn namespaced(scope: &str) -> NamespaceScope {
        NamespaceScope::One(scope.to_string())
    }

    #[test]
    fn get_pods_with_namespace_opens_the_pod_list() {
        let command = parse("kubectl get pods -n cas").expect("parse");

        assert_eq!(
            command.action,
            KubectlAction::Get {
                kind: ResourceKind::Pods,
                name: None
            }
        );
        assert_eq!(command.namespace, namespaced("cas"));
    }

    #[test]
    fn short_resource_names_and_flags_in_any_order_are_supported() {
        let command = parse("kubectl get -n cas po").expect("parse");

        assert_eq!(
            command.action,
            KubectlAction::Get {
                kind: ResourceKind::Pods,
                name: None
            }
        );
        assert_eq!(command.namespace, namespaced("cas"));
    }

    #[test]
    fn get_with_a_resource_name_keeps_the_name() {
        let command = parse("kubectl get deploy/api -n cas").expect("parse");

        assert_eq!(
            command.action,
            KubectlAction::Get {
                kind: ResourceKind::Deployments,
                name: Some("api".to_string())
            }
        );
        assert_eq!(command.namespace, namespaced("cas"));
    }

    #[test]
    fn describe_pod_targets_the_detail_view() {
        let command = parse("kubectl describe pod cas-pod -n cas").expect("parse");

        assert_eq!(
            command.action,
            KubectlAction::Describe {
                kind: ResourceKind::Pods,
                name: "cas-pod".to_string()
            }
        );
        assert_eq!(command.namespace, namespaced("cas"));
    }

    #[test]
    fn describe_supports_type_slash_name() {
        let command = parse("kubectl describe deploy/api -n cas").expect("parse");

        assert_eq!(
            command.action,
            KubectlAction::Describe {
                kind: ResourceKind::Deployments,
                name: "api".to_string()
            }
        );
    }

    #[test]
    fn describe_with_only_a_name_assumes_a_pod() {
        let command = parse("kubectl describe -n cas cas-pod").expect("parse");

        assert_eq!(
            command.action,
            KubectlAction::Describe {
                kind: ResourceKind::Pods,
                name: "cas-pod".to_string()
            }
        );
        assert_eq!(command.namespace, namespaced("cas"));
    }

    #[test]
    fn logs_opens_the_pod_with_an_optional_container() {
        let command = parse("kubectl logs -n cas cas-pod -c app --follow").expect("parse");

        assert_eq!(
            command.action,
            KubectlAction::Logs {
                pod: "cas-pod".to_string(),
                container: Some("app".to_string()),
                follow: true,
            }
        );
        assert_eq!(command.namespace, namespaced("cas"));
    }

    #[test]
    fn the_kubectl_prefix_is_optional() {
        let command = parse("get pods -n cas").expect("parse");

        assert_eq!(
            command.action,
            KubectlAction::Get {
                kind: ResourceKind::Pods,
                name: None
            }
        );
    }

    #[test]
    fn namespace_equals_syntax_and_all_namespaces_work() {
        let command = parse("kubectl get services --namespace=cas").expect("parse");
        assert_eq!(command.namespace, namespaced("cas"));

        let command = parse("kubectl get pods -A").expect("parse");
        assert_eq!(command.namespace, NamespaceScope::All);
    }

    #[test]
    fn quoted_names_are_one_argument() {
        let command = parse("kubectl describe pod \"my pod\" -n cas").expect("parse");

        assert_eq!(
            command.action,
            KubectlAction::Describe {
                kind: ResourceKind::Pods,
                name: "my pod".to_string()
            }
        );
    }

    #[test]
    fn every_builtin_resource_kind_can_be_navigated_to() {
        for kind in catalog::builtin_kinds().iter().copied() {
            let label = kind.label();
            let get = parse(&format!("kubectl get {label} -n cas"))
                .unwrap_or_else(|error| panic!("could not parse get for {label}: {error}"));
            assert_eq!(
                get.action,
                KubectlAction::Get { kind, name: None },
                "get did not resolve {label}"
            );

            let describe = parse(&format!("kubectl describe {label} sample -n cas"))
                .unwrap_or_else(|error| panic!("could not parse describe for {label}: {error}"));
            assert_eq!(
                describe.action,
                KubectlAction::Describe {
                    kind,
                    name: "sample".to_string()
                },
                "describe did not resolve {label}"
            );
        }
    }

    #[test]
    fn every_navigable_resource_kind_can_be_navigated_to() {
        let mut checked = 0;
        for (_, kinds) in ResourceKind::nav_groups() {
            for kind in kinds {
                let label = kind.label();
                let command = parse(&format!("kubectl get {label} -n cas"))
                    .unwrap_or_else(|error| panic!("could not parse {label}: {error}"));
                assert!(
                    matches!(command.action, KubectlAction::Get { kind: resolved, .. } if resolved == kind),
                    "get did not resolve navigable kind {label}"
                );
                checked += 1;
            }
        }
        assert!(checked >= catalog::builtin_kinds().len());
    }

    #[test]
    fn fully_qualified_resource_names_are_resolved() {
        let command = parse("kubectl get deployments.apps -n cas").expect("parse");

        assert_eq!(
            command.action,
            KubectlAction::Get {
                kind: ResourceKind::Deployments,
                name: None
            }
        );
    }

    #[test]
    fn unsupported_commands_and_missing_types_are_reported() {
        assert!(parse("kubectl apply -f pods.yaml").is_err());
        assert!(parse("kubectl get").is_err());
        assert!(parse("kubectl describe pod").is_err());
        assert!(parse("kubectl logs").is_err());
        assert!(parse("not-kubectl get pods").is_err());
    }
}
