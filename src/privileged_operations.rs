use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceLifecycleAction {
    Init,
    Install,
    Uninstall,
    Start,
    Stop,
    Restart,
}

impl ServiceLifecycleAction {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "init" => Ok(Self::Init),
            "install" => Ok(Self::Install),
            "uninstall" => Ok(Self::Uninstall),
            "start" => Ok(Self::Start),
            "stop" => Ok(Self::Stop),
            "restart" => Ok(Self::Restart),
            _ => bail!("PRIVILEGED_SERVICE_INVALID_ACTION: unsupported service action"),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Init => "init",
            Self::Install => "install",
            Self::Uninstall => "uninstall",
            Self::Start => "start",
            Self::Stop => "stop",
            Self::Restart => "restart",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ServiceLifecycleOptions {
    pub executable_path: String,
    pub action: ServiceLifecycleAction,
    pub public_key: Option<String>,
    pub authorized_sid: Option<String>,
    pub authorized_uid: Option<u32>,
}

fn validate_service_executable(value: &str) -> Result<PathBuf> {
    let requested = Path::new(value);
    if !requested.is_absolute() {
        bail!("PRIVILEGED_SERVICE_INVALID_PATH: service path must be absolute");
    }
    let canonical = std::fs::canonicalize(requested)
        .context("PRIVILEGED_SERVICE_INVALID_PATH: unable to resolve service executable")?;
    if !canonical.is_file() {
        bail!("PRIVILEGED_SERVICE_INVALID_PATH: service path must be a regular file");
    }
    let name = canonical
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    if !matches!(name, "kokorobox-service" | "kokorobox-service.exe") {
        bail!("PRIVILEGED_SERVICE_INVALID_PATH: unexpected service executable name");
    }
    Ok(canonical)
}

fn validate_public_key(value: &str) -> Result<()> {
    if !(32..=512).contains(&value.len())
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/' | b'='))
    {
        bail!("PRIVILEGED_SERVICE_INVALID_KEY: invalid service public key");
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn validate_sid(value: &str) -> Result<()> {
    if value.len() > 184
        || !value.starts_with("S-")
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || byte == b'S' || byte == b'-')
    {
        bail!("PRIVILEGED_SERVICE_INVALID_PRINCIPAL: invalid Windows SID");
    }
    Ok(())
}

fn service_arguments(options: &ServiceLifecycleOptions) -> Result<Vec<String>> {
    let mut arguments = vec!["service".to_string(), options.action.as_str().to_string()];
    if options.action == ServiceLifecycleAction::Init {
        let public_key = options
            .public_key
            .as_deref()
            .ok_or_else(|| anyhow!("PRIVILEGED_SERVICE_INVALID_KEY: public key is required"))?;
        validate_public_key(public_key)?;
        arguments.extend(["--public-key".to_string(), public_key.to_string()]);

        #[cfg(target_os = "windows")]
        {
            let sid = options.authorized_sid.as_deref().ok_or_else(|| {
                anyhow!("PRIVILEGED_SERVICE_INVALID_PRINCIPAL: Windows SID is required")
            })?;
            validate_sid(sid)?;
            if options.authorized_uid.is_some() {
                bail!("PRIVILEGED_SERVICE_INVALID_PRINCIPAL: UID is not valid on Windows");
            }
            arguments.extend(["--authorized-sid".to_string(), sid.to_string()]);
        }
        #[cfg(not(target_os = "windows"))]
        {
            let uid = options.authorized_uid.ok_or_else(|| {
                anyhow!("PRIVILEGED_SERVICE_INVALID_PRINCIPAL: Unix UID is required")
            })?;
            if options.authorized_sid.is_some() {
                bail!("PRIVILEGED_SERVICE_INVALID_PRINCIPAL: SID is not valid on Unix");
            }
            arguments.extend(["--authorized-uid".to_string(), uid.to_string()]);
        }
    } else if options.public_key.is_some()
        || options.authorized_sid.is_some()
        || options.authorized_uid.is_some()
    {
        bail!("PRIVILEGED_SERVICE_INVALID_ARGUMENT: init fields require the init action");
    }
    Ok(arguments)
}

#[cfg(target_os = "windows")]
fn run_service(executable: &Path, arguments: &[String]) -> Result<()> {
    let executable = executable.to_string_lossy().into_owned();
    let exit_code = if crate::windows::is_running_as_admin()? {
        std::process::Command::new(&executable)
            .args(arguments)
            .status()
            .context("PRIVILEGED_SERVICE_FAILED: unable to start service command")?
            .code()
            .unwrap_or(-1) as u32
    } else {
        crate::windows::run_elevated(&executable, arguments)?
    };
    if exit_code != 0 {
        bail!("PRIVILEGED_SERVICE_FAILED: service command exited with {exit_code}");
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn run_service(executable: &Path, arguments: &[String]) -> Result<()> {
    let status = std::process::Command::new("pkexec")
        .arg(executable)
        .args(arguments)
        .status()
        .context("PRIVILEGED_SERVICE_FAILED: unable to start pkexec")?;
    if !status.success() {
        bail!(
            "PRIVILEGED_SERVICE_FAILED: service command exited with {}",
            status.code().unwrap_or(-1)
        );
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(target_os = "macos")]
fn applescript_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(target_os = "macos")]
fn run_macos_privileged_shell(shell: &str, error_code: &str) -> Result<()> {
    let script = format!(
        "do shell script \"{}\" with administrator privileges",
        applescript_escape(shell)
    );
    let output = std::process::Command::new("/usr/bin/osascript")
        .args(["-e", &script])
        .output()
        .with_context(|| format!("{error_code}: unable to run osascript"))?;
    if !output.status.success() {
        bail!(
            "{error_code}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn run_service(executable: &Path, arguments: &[String]) -> Result<()> {
    let command = std::iter::once(shell_quote(&executable.to_string_lossy()))
        .chain(arguments.iter().map(|argument| shell_quote(argument)))
        .collect::<Vec<_>>()
        .join(" ");
    run_macos_privileged_shell(&command, "PRIVILEGED_SERVICE_FAILED")
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
fn run_service(_executable: &Path, _arguments: &[String]) -> Result<()> {
    Err(anyhow!(
        "UNSUPPORTED_PLATFORM: privileged service commands require Windows, Linux, or macOS"
    ))
}

pub fn run_service_lifecycle(options: &ServiceLifecycleOptions) -> Result<()> {
    let executable = validate_service_executable(&options.executable_path)?;
    let arguments = service_arguments(options)?;
    run_service(&executable, &arguments)
}

#[cfg(target_os = "macos")]
pub fn cleanup_legacy_macos_service() -> Result<()> {
    run_macos_privileged_shell(
        "/bin/launchctl bootout system/KokoroBoxService >/dev/null 2>&1 || true; /bin/rm -f '/Library/LaunchDaemons/KokoroBoxService.plist' '/Library/PrivilegedHelperTools/com.amamiyakokoro.kokorobox-service'",
        "PRIVILEGED_MACOS_SERVICE_CLEANUP_FAILED",
    )
}

#[cfg(not(target_os = "macos"))]
pub fn cleanup_legacy_macos_service() -> Result<()> {
    Err(anyhow!(
        "UNSUPPORTED_PLATFORM: legacy macOS service cleanup requires macOS"
    ))
}

#[cfg(target_os = "macos")]
pub fn stop_macos_managed_service() -> Result<()> {
    run_macos_privileged_shell(
        "/bin/launchctl kill SIGTERM system/KokoroBoxService",
        "PRIVILEGED_MACOS_SERVICE_STOP_FAILED",
    )
}

#[cfg(not(target_os = "macos"))]
pub fn stop_macos_managed_service() -> Result<()> {
    Err(anyhow!(
        "UNSUPPORTED_PLATFORM: stopping the managed macOS service requires macOS"
    ))
}

fn normalized_absolute(path: &Path) -> Result<PathBuf> {
    if !path.is_absolute() {
        bail!("PRIVILEGED_FILE_INVALID_PATH: path must be absolute");
    }
    if path
        .components()
        .any(|component| component == Component::ParentDir)
    {
        bail!("PRIVILEGED_FILE_INVALID_PATH: parent traversal is not allowed");
    }
    Ok(path.to_path_buf())
}

fn validate_managed_file(target: &str, managed_root: &str) -> Result<(PathBuf, PathBuf)> {
    let target = normalized_absolute(Path::new(target))?;
    let root = std::fs::canonicalize(normalized_absolute(Path::new(managed_root))?)
        .context("PRIVILEGED_FILE_INVALID_PATH: unable to resolve managed root")?;
    if !root.is_dir() {
        bail!("PRIVILEGED_FILE_INVALID_PATH: managed root must be a directory");
    }
    let parent = target
        .parent()
        .ok_or_else(|| anyhow!("PRIVILEGED_FILE_INVALID_PATH: target has no parent"))?
        .to_path_buf();
    let mut ancestor = parent.as_path();
    while !ancestor.exists() {
        ancestor = ancestor.parent().ok_or_else(|| {
            anyhow!("PRIVILEGED_FILE_INVALID_PATH: unable to resolve target parent")
        })?;
    }
    let ancestor = std::fs::canonicalize(ancestor)
        .context("PRIVILEGED_FILE_INVALID_PATH: unable to resolve target parent")?;
    if !ancestor.starts_with(&root) || !target.starts_with(Path::new(managed_root)) {
        bail!("PRIVILEGED_FILE_INVALID_PATH: target is outside the managed root");
    }
    if target.exists()
        && !std::fs::canonicalize(&target)
            .context("PRIVILEGED_FILE_INVALID_PATH: unable to resolve target")?
            .starts_with(&root)
    {
        bail!("PRIVILEGED_FILE_INVALID_PATH: target resolves outside the managed root");
    }
    Ok((target, parent))
}

#[cfg(unix)]
fn validate_requested_owner(uid: u32, gid: u32) -> Result<()> {
    // SAFETY: These libc accessors have no preconditions and read the real IDs
    // of the current process without retaining pointers.
    let (current_uid, current_gid) = unsafe { (libc::getuid(), libc::getgid()) };
    if uid == 0 || uid != current_uid || gid != current_gid {
        bail!("PRIVILEGED_FILE_INVALID_OWNER: owner must be the current non-root user");
    }
    Ok(())
}

#[cfg(not(unix))]
fn validate_requested_owner(_uid: u32, _gid: u32) -> Result<()> {
    Err(anyhow!(
        "UNSUPPORTED_PLATFORM: managed-file permission repair requires Unix"
    ))
}

#[cfg(target_os = "linux")]
fn repair_file_permissions(target: &Path, parent: &Path, uid: u32, gid: u32) -> Result<()> {
    let script = "mkdir -p -- \"$1\" && chown \"$2:$3\" -- \"$1\" && chmod u+rwx -- \"$1\" && { [ ! -e \"$4\" ] || { chown \"$2:$3\" -- \"$4\" && chmod u+rw -- \"$4\"; }; }";
    let status = std::process::Command::new("pkexec")
        .args(["/bin/sh", "-c", script, "kokorobox-permission-repair"])
        .arg(parent)
        .arg(uid.to_string())
        .arg(gid.to_string())
        .arg(target)
        .status()
        .context("PRIVILEGED_FILE_REPAIR_FAILED: unable to start pkexec")?;
    if !status.success() {
        bail!(
            "PRIVILEGED_FILE_REPAIR_FAILED: permission repair exited with {}",
            status.code().unwrap_or(-1)
        );
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn repair_file_permissions(target: &Path, parent: &Path, uid: u32, gid: u32) -> Result<()> {
    let owner = format!("{uid}:{gid}");
    let shell = format!(
        "/bin/mkdir -p {parent} && /usr/sbin/chown {owner} {parent} && /bin/chmod u+rwx {parent} && {{ [ ! -e {target} ] || {{ /usr/sbin/chown {owner} {target} && /bin/chmod u+rw {target}; }}; }}",
        parent = shell_quote(&parent.to_string_lossy()),
        target = shell_quote(&target.to_string_lossy()),
        owner = shell_quote(&owner),
    );
    run_macos_privileged_shell(&shell, "PRIVILEGED_FILE_REPAIR_FAILED")
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn repair_file_permissions(_target: &Path, _parent: &Path, _uid: u32, _gid: u32) -> Result<()> {
    Err(anyhow!(
        "UNSUPPORTED_PLATFORM: managed-file permission repair requires Linux or macOS"
    ))
}

pub fn repair_managed_file_permissions(
    target: &str,
    managed_root: &str,
    uid: u32,
    gid: u32,
) -> Result<()> {
    validate_requested_owner(uid, gid)?;
    let (target, parent) = validate_managed_file(target, managed_root)?;
    repair_file_permissions(&target, &parent, uid, gid)
}

#[cfg(target_os = "windows")]
pub fn relaunch_current_application_with_privilege(
    arguments: &[String],
    elevated: bool,
) -> Result<()> {
    let executable = std::env::current_exe()
        .context("PRIVILEGED_RELAUNCH_FAILED: unable to resolve current executable")?;
    let executable = executable.to_string_lossy();
    if elevated {
        crate::windows::launch_elevated(executable.as_ref(), arguments)
    } else {
        crate::windows::launch_unelevated(executable.as_ref(), arguments)
    }
}

#[cfg(not(target_os = "windows"))]
pub fn relaunch_current_application_with_privilege(
    _arguments: &[String],
    _elevated: bool,
) -> Result<()> {
    Err(anyhow!(
        "UNSUPPORTED_PLATFORM: privilege relaunch requires Windows"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_named_service_actions() {
        for action in ["init", "install", "uninstall", "start", "stop", "restart"] {
            assert!(ServiceLifecycleAction::parse(action).is_ok());
        }
        assert!(ServiceLifecycleAction::parse("exec").is_err());
        assert!(ServiceLifecycleAction::parse("--help").is_err());
    }

    #[test]
    fn validates_service_init_fields() {
        let options = ServiceLifecycleOptions {
            executable_path: "/missing/kokorobox-service".to_string(),
            action: ServiceLifecycleAction::Init,
            public_key: Some("A".repeat(44)),
            authorized_sid: None,
            authorized_uid: Some(1000),
        };
        assert!(service_arguments(&options).is_ok());
        let mut invalid = options.clone();
        invalid.public_key = Some("not a key!".to_string());
        assert!(service_arguments(&invalid).is_err());
    }

    #[test]
    fn rejects_parent_traversal_in_managed_paths() {
        assert!(normalized_absolute(Path::new("relative/file")).is_err());
        assert!(normalized_absolute(Path::new("/tmp/root/../outside")).is_err());
    }
}
