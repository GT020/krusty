use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::process::{Command, Stdio};
use std::sync::OnceLock;
#[cfg(unix)]
use std::time::{Duration, Instant};

use kube::config::{ExecConfig, Kubeconfig};

#[cfg(unix)]
const SHELL_ENV_START: &str = "__KRUSTY_ENV_START__";
#[cfg(unix)]
const SHELL_ENV_END: &str = "__KRUSTY_ENV_END__";
#[cfg(unix)]
const SHELL_ENV_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Clone, Debug, Default)]
struct ShellEnvironment {
    vars: HashMap<String, String>,
}

impl ShellEnvironment {
    fn path_dirs(&self) -> impl Iterator<Item = PathBuf> + '_ {
        self.vars
            .get("PATH")
            .into_iter()
            .flat_map(|path| std::env::split_paths(path))
    }
}

pub async fn resolve_programs(kubeconfig: &mut Kubeconfig) {
    let shell = tokio::task::spawn_blocking(cached_shell_environment)
        .await
        .ok()
        .flatten();

    resolve_programs_with(kubeconfig, &search_dirs(), shell.as_ref());
}

fn cached_shell_environment() -> Option<ShellEnvironment> {
    static CACHED: OnceLock<Option<ShellEnvironment>> = OnceLock::new();

    CACHED.get_or_init(shell_environment).clone()
}

fn shell_environment() -> Option<ShellEnvironment> {
    #[cfg(not(unix))]
    {
        return None;
    }

    #[cfg(unix)]
    {
        let started = Instant::now();
        let shell = std::env::var("SHELL")
            .ok()
            .filter(|shell| !shell.is_empty())
            .or_else(default_shell)?;

        match read_shell_environment(&shell) {
            Ok(vars) => {
                tracing::debug!(
                    shell = %shell,
                    variables = vars.len(),
                    elapsed_ms = started.elapsed().as_millis() as u64,
                    "cluster: read login shell environment"
                );
                Some(ShellEnvironment { vars })
            }
            Err(error) => {
                tracing::warn!(
                    shell = %shell,
                    %error,
                    "cluster: could not read login shell environment; falling back to well-known directories"
                );
                None
            }
        }
    }
}

#[cfg(unix)]
fn default_shell() -> Option<String> {
    ["/bin/zsh", "/bin/bash", "/bin/sh"]
        .into_iter()
        .find(|candidate| Path::new(candidate).is_file())
        .map(str::to_string)
}

#[cfg(unix)]
struct ShellInvocation {
    args: Vec<String>,
    stdin: Option<String>,
}

#[cfg(unix)]
fn shell_invocation(shell: &str) -> ShellInvocation {
    let command =
        format!("printf '%s' {SHELL_ENV_START}; /usr/bin/env -0; printf '%s' {SHELL_ENV_END}");
    let name = Path::new(shell)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();

    if name.starts_with("fish") {
        ShellInvocation {
            args: vec!["-l".into(), "-c".into(), command],
            stdin: None,
        }
    } else if name == "csh" || name == "tcsh" {
        ShellInvocation {
            args: vec!["-l".into()],
            stdin: Some(command),
        }
    } else {
        ShellInvocation {
            args: vec!["-l".into(), "-i".into(), "-c".into(), command],
            stdin: None,
        }
    }
}

#[cfg(unix)]
fn read_shell_environment(shell: &str) -> Result<HashMap<String, String>, String> {
    let invocation = shell_invocation(shell);
    let mut command = Command::new(shell);
    command
        .args(&invocation.args)
        .stdin(if invocation.stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if std::env::var_os("TERM").is_none() {
        command.env("TERM", "xterm-256color");
    }

    let mut child = command
        .spawn()
        .map_err(|error| format!("failed to start: {error}"))?;

    if let Some(input) = invocation.stdin {
        use std::io::Write as _;
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(input.as_bytes())
                .map_err(|error| format!("failed to write command: {error}"))?;
        }
    }

    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| "no stdout pipe".to_string())?;
    let stdout_reader = std::thread::spawn(move || {
        use std::io::Read as _;
        let mut bytes = Vec::new();
        let _ = stdout.read_to_end(&mut bytes);
        bytes
    });

    let mut stderr = child
        .stderr
        .take()
        .ok_or_else(|| "no stderr pipe".to_string())?;
    let stderr_reader = std::thread::spawn(move || {
        use std::io::Read as _;
        let mut bytes = Vec::new();
        let _ = stderr.read_to_end(&mut bytes);
        bytes
    });

    let deadline = Instant::now() + SHELL_ENV_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let stderr = stderr_reader.join().unwrap_or_default();
                if !status.success() {
                    let _ = stdout_reader.join();
                    return Err(format!(
                        "shell exited with {status}: {}",
                        String::from_utf8_lossy(&stderr).trim()
                    ));
                }
                break;
            }
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = stdout_reader.join();
                let _ = stderr_reader.join();
                return Err(format!("timed out after {}s", SHELL_ENV_TIMEOUT.as_secs()));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(25)),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = stdout_reader.join();
                let _ = stderr_reader.join();
                return Err(format!("failed to wait: {error}"));
            }
        }
    }

    let output = stdout_reader
        .join()
        .map_err(|_| "environment reader panicked".to_string())?;
    parse_shell_environment(&output)
}

#[cfg(unix)]
fn parse_shell_environment(output: &[u8]) -> Result<HashMap<String, String>, String> {
    let text = String::from_utf8_lossy(output);
    let start = text
        .find(SHELL_ENV_START)
        .ok_or_else(|| "start marker missing".to_string())?;
    let rest = &text[start + SHELL_ENV_START.len()..];
    let end = rest
        .find(SHELL_ENV_END)
        .ok_or_else(|| "end marker missing".to_string())?;

    let mut vars = HashMap::new();
    for record in rest[..end].split('\0') {
        if let Some((name, value)) = record.split_once('=')
            && !name.is_empty()
        {
            vars.insert(name.to_string(), value.to_string());
        }
    }

    if vars.is_empty() {
        Err("empty environment".to_string())
    } else {
        Ok(vars)
    }
}

fn resolve_programs_with(
    kubeconfig: &mut Kubeconfig,
    dirs: &[PathBuf],
    shell: Option<&ShellEnvironment>,
) {
    let process_dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).collect())
        .unwrap_or_default();
    let shell_dirs: Vec<PathBuf> = shell
        .map(|shell| shell.path_dirs().collect())
        .unwrap_or_default();

    let mut seen = HashSet::new();
    let search: Vec<PathBuf> = shell_dirs
        .into_iter()
        .chain(process_dirs)
        .chain(dirs.iter().cloned())
        .filter(|dir| !dir.as_os_str().is_empty() && seen.insert(dir.clone()))
        .collect();

    for named in &mut kubeconfig.auth_infos {
        let Some(auth) = &mut named.auth_info else {
            continue;
        };

        if let Some(exec) = &mut auth.exec {
            if let Some(command) = &exec.command
                && let Some(resolved) = resolve(command, &search)
            {
                tracing::debug!(
                    command = %command,
                    resolved = %resolved.display(),
                    "cluster: resolved credential plugin"
                );
                exec.command = Some(resolved.to_string_lossy().into_owned());
            }

            if let Some(shell) = shell {
                add_exec_environment(exec, &shell.vars);
            }
        }

        if let Some(provider) = &mut auth.auth_provider
            && let Some(command) = provider.config.get("cmd-path").cloned()
            && let Some(resolved) = resolve(&command, &search)
        {
            tracing::debug!(
                command = %command,
                resolved = %resolved.display(),
                "cluster: resolved auth-provider command"
            );
            provider
                .config
                .insert("cmd-path".into(), resolved.to_string_lossy().into_owned());
        }
    }
}

fn add_exec_environment(exec: &mut ExecConfig, vars: &HashMap<String, String>) {
    let existing: HashSet<&str> = exec
        .env
        .iter()
        .flatten()
        .filter_map(|entry| entry.get("name").map(String::as_str))
        .collect();

    let mut additions: Vec<(&String, &String)> = vars
        .iter()
        .filter(|(name, _)| {
            name.as_str() != "KUBERNETES_EXEC_INFO" && !existing.contains(name.as_str())
        })
        .collect();
    additions.sort_by(|left, right| left.0.cmp(right.0));

    if additions.is_empty() {
        return;
    }

    let env = exec.env.get_or_insert_with(Vec::new);
    env.extend(additions.into_iter().map(|(name, value)| {
        HashMap::from([
            ("name".to_string(), name.clone()),
            ("value".to_string(), value.clone()),
        ])
    }));
}

pub fn missing_plugin_hint(kubeconfig: &Kubeconfig, context_name: &str) -> Option<String> {
    let context = kubeconfig
        .contexts
        .iter()
        .find(|context| context.name == context_name)?
        .context
        .as_ref()?;
    let user_name = context.user.as_ref()?;
    let auth = kubeconfig
        .auth_infos
        .iter()
        .find(|user| &user.name == user_name)?
        .auth_info
        .as_ref()?;
    let exec = auth.exec.as_ref()?;
    let command = exec.command.as_deref()?;

    if Path::new(command).is_absolute() || command.contains(std::path::MAIN_SEPARATOR) {
        return None;
    }

    let mut message = format!("credential plugin `{command}` was not found on PATH");
    if let Some(hint) = &exec.install_hint {
        message.push_str(&format!("; {hint}"));
    }
    Some(message)
}

fn resolve(program: &str, dirs: &[PathBuf]) -> Option<PathBuf> {
    if program.trim().is_empty() {
        return None;
    }

    let path = Path::new(program);
    if path.is_absolute() {
        return is_executable(path).then(|| path.to_path_buf());
    }

    if program.contains(std::path::MAIN_SEPARATOR) {
        return None;
    }

    dirs.iter()
        .map(|dir| dir.join(program))
        .find(|candidate| is_executable(candidate))
}

fn is_executable(path: &Path) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn search_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    if let Some(home) = dirs::home_dir() {
        dirs.push(home.join("google-cloud-sdk").join("bin"));
        dirs.push(home.join(".local").join("bin"));
        dirs.push(home.join("bin"));
    }

    #[cfg(target_os = "macos")]
    {
        dirs.push(PathBuf::from("/opt/homebrew/bin"));
        dirs.push(PathBuf::from("/opt/homebrew/sbin"));
        dirs.push(PathBuf::from("/usr/local/bin"));
        dirs.push(PathBuf::from("/usr/local/sbin"));
        dirs.push(PathBuf::from("/opt/local/bin"));
        dirs.push(PathBuf::from("/opt/local/sbin"));
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        dirs.push(PathBuf::from("/usr/local/bin"));
        dirs.push(PathBuf::from("/home/linuxbrew/.linuxbrew/bin"));
        dirs.push(PathBuf::from("/snap/bin"));
    }

    dirs.push(PathBuf::from("/usr/bin"));
    dirs.push(PathBuf::from("/bin"));
    dirs
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use kube::config::{AuthInfo, AuthProviderConfig, ExecConfig, NamedAuthInfo};

    use super::*;

    fn fake_plugin_dir(name: &str) -> PathBuf {
        fake_plugin_dir_with(name, "#!/bin/sh\nexit 0\n")
    }

    fn fake_plugin_dir_with(name: &str, body: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("krusty-exec-test-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let plugin = dir.join(name);
        std::fs::write(&plugin, body).expect("write fake plugin");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(&plugin, std::fs::Permissions::from_mode(0o755))
                .expect("chmod fake plugin");
        }
        dir
    }

    #[test]
    fn resolves_bare_exec_commands_from_extra_dirs() {
        let name = "krusty-fake-auth-plugin";
        let dir = fake_plugin_dir(name);
        let mut kubeconfig = Kubeconfig::default();
        kubeconfig.auth_infos.push(NamedAuthInfo {
            name: "test".into(),
            auth_info: Some(AuthInfo {
                exec: Some(ExecConfig {
                    command: Some(name.into()),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        });

        resolve_programs_with(&mut kubeconfig, std::slice::from_ref(&dir), None);

        let command = kubeconfig.auth_infos[0]
            .auth_info
            .as_ref()
            .and_then(|auth| auth.exec.as_ref())
            .and_then(|exec| exec.command.as_deref());
        assert_eq!(command, dir.join(name).to_str());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn resolves_bare_exec_commands_from_the_login_shell_path() {
        let name = "krusty-fake-shell-plugin";
        let dir = fake_plugin_dir(name);
        let shell = ShellEnvironment {
            vars: HashMap::from([("PATH".into(), dir.to_string_lossy().into_owned())]),
        };
        let mut kubeconfig = Kubeconfig::default();
        kubeconfig.auth_infos.push(NamedAuthInfo {
            name: "test".into(),
            auth_info: Some(AuthInfo {
                exec: Some(ExecConfig {
                    command: Some(name.into()),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        });

        resolve_programs_with(&mut kubeconfig, &[], Some(&shell));

        let command = kubeconfig.auth_infos[0]
            .auth_info
            .as_ref()
            .and_then(|auth| auth.exec.as_ref())
            .and_then(|exec| exec.command.as_deref());
        assert_eq!(command, dir.join(name).to_str());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn exec_plugins_receive_the_login_shell_environment() {
        let mut exec = ExecConfig {
            env: Some(vec![HashMap::from([
                ("name".to_string(), "KEEP".to_string()),
                ("value".to_string(), "user-value".to_string()),
            ])]),
            ..Default::default()
        };
        let vars = HashMap::from([
            ("PATH".to_string(), "/custom/bin".to_string()),
            ("KEEP".to_string(), "shell-value".to_string()),
            ("KUBERNETES_EXEC_INFO".to_string(), "{}".to_string()),
        ]);

        add_exec_environment(&mut exec, &vars);

        let entries = exec.env.expect("environment");
        let value = |name: &str| {
            entries
                .iter()
                .find(|entry| entry.get("name").map(String::as_str) == Some(name))
                .and_then(|entry| entry.get("value"))
                .map(String::as_str)
        };
        assert_eq!(value("PATH"), Some("/custom/bin"));
        assert_eq!(value("KEEP"), Some("user-value"));
        assert_eq!(value("KUBERNETES_EXEC_INFO"), None);
    }

    #[test]
    fn resolves_legacy_auth_provider_cmd_paths() {
        let name = "krusty-fake-gcp-helper";
        let dir = fake_plugin_dir(name);
        let mut kubeconfig = Kubeconfig::default();
        kubeconfig.auth_infos.push(NamedAuthInfo {
            name: "test".into(),
            auth_info: Some(AuthInfo {
                auth_provider: Some(AuthProviderConfig {
                    name: "gcp".into(),
                    config: HashMap::from([("cmd-path".into(), name.to_string())]),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        });

        resolve_programs_with(&mut kubeconfig, std::slice::from_ref(&dir), None);

        let command = kubeconfig.auth_infos[0]
            .auth_info
            .as_ref()
            .and_then(|auth| auth.auth_provider.as_ref())
            .and_then(|provider| provider.config.get("cmd-path"));
        assert_eq!(command.map(String::as_str), dir.join(name).to_str());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn explains_a_missing_exec_plugin() {
        let mut kubeconfig = Kubeconfig::default();
        kubeconfig.contexts.push(kube::config::NamedContext {
            name: "gke".into(),
            context: Some(kube::config::Context {
                cluster: "cluster".into(),
                user: Some("user".into()),
                ..Default::default()
            }),
            ..Default::default()
        });
        kubeconfig.auth_infos.push(NamedAuthInfo {
            name: "user".into(),
            auth_info: Some(AuthInfo {
                exec: Some(ExecConfig {
                    command: Some("krusty-definitely-missing-plugin".into()),
                    install_hint: Some("install the plugin".into()),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        });

        let hint = missing_plugin_hint(&kubeconfig, "gke").expect("missing plugin hint");
        assert!(hint.contains("krusty-definitely-missing-plugin"));
        assert!(hint.contains("install the plugin"));
    }

    #[cfg(unix)]
    #[test]
    fn parses_the_login_shell_environment_between_markers() {
        let output = format!(
            "shell banner\n{SHELL_ENV_START}PATH=/a:/b\0HOME=/home/test\0EMPTY=\0{SHELL_ENV_END}trailing\n"
        );

        let vars = parse_shell_environment(output.as_bytes()).expect("parsed environment");
        assert_eq!(vars.get("PATH").map(String::as_str), Some("/a:/b"));
        assert_eq!(vars.get("HOME").map(String::as_str), Some("/home/test"));
        assert_eq!(vars.get("EMPTY").map(String::as_str), Some(""));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_resolved_plugin_makes_the_kube_client_build() {
        let name = "krusty-fake-token-plugin";
        let dir = fake_plugin_dir_with(
            name,
            "#!/bin/sh\nprintf '%s' '{\"apiVersion\":\"client.authentication.k8s.io/v1beta1\",\"kind\":\"ExecCredential\",\"status\":{\"token\":\"test-token\"}}'\n",
        );
        let mut kubeconfig = Kubeconfig {
            current_context: Some("test".into()),
            ..Default::default()
        };
        kubeconfig.clusters.push(kube::config::NamedCluster {
            name: "test".into(),
            cluster: Some(kube::config::Cluster {
                server: Some("https://127.0.0.1:1".into()),
                ..Default::default()
            }),
            ..Default::default()
        });
        kubeconfig.contexts.push(kube::config::NamedContext {
            name: "test".into(),
            context: Some(kube::config::Context {
                cluster: "test".into(),
                user: Some("test".into()),
                ..Default::default()
            }),
            ..Default::default()
        });
        kubeconfig.auth_infos.push(NamedAuthInfo {
            name: "test".into(),
            auth_info: Some(AuthInfo {
                exec: Some(ExecConfig {
                    command: Some(name.into()),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        });

        resolve_programs_with(&mut kubeconfig, std::slice::from_ref(&dir), None);
        assert!(missing_plugin_hint(&kubeconfig, "test").is_none());
        let config = kube::Config::from_custom_kubeconfig(
            kubeconfig,
            &kube::config::KubeConfigOptions::default(),
        )
        .await
        .expect("valid kubeconfig");
        assert!(
            kube::Client::try_from(config).is_ok(),
            "the resolved plugin should run and provide credentials"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn leaves_explicit_paths_untouched() {
        assert!(resolve("/definitely/missing/krusty-plugin", &[]).is_none());
        assert!(resolve("./krusty-plugin", &[]).is_none());
        assert!(resolve("nested/krusty-plugin", &[]).is_none());
        assert!(resolve("", &[]).is_none());
    }
}
