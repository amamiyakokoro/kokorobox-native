use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

#[derive(Debug, Clone)]
pub struct CorePrivilegeStatus {
    pub path: String,
    pub granted: bool,
}

const MAXIMUM_CORE_PATHS: usize = 8;

fn validate_core_paths(paths: &[String]) -> Result<Vec<PathBuf>> {
    if paths.is_empty() {
        bail!("CORE_PRIVILEGE_INVALID_PATH: at least one core path is required");
    }
    if paths.len() > MAXIMUM_CORE_PATHS {
        bail!("CORE_PRIVILEGE_INVALID_PATH: too many core paths");
    }

    paths
        .iter()
        .map(|path| {
            let requested = Path::new(path);
            if !requested.is_absolute() {
                bail!("CORE_PRIVILEGE_INVALID_PATH: core path must be absolute");
            }
            let canonical = std::fs::canonicalize(requested).with_context(|| {
                format!(
                    "CORE_PRIVILEGE_INVALID_PATH: unable to resolve {}",
                    requested.display()
                )
            })?;
            let metadata = std::fs::metadata(&canonical)?;
            if !metadata.is_file() {
                bail!("CORE_PRIVILEGE_INVALID_PATH: core path must be a regular file");
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;

                if metadata.permissions().mode() & 0o111 == 0 {
                    bail!("CORE_PRIVILEGE_INVALID_PATH: core path must be executable");
                }
            }
            let name = canonical
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default();
            if !matches!(name, "mihomo" | "mihomo-alpha") {
                bail!("CORE_PRIVILEGE_INVALID_PATH: only mihomo and mihomo-alpha are accepted");
            }
            Ok(canonical)
        })
        .collect()
}

#[cfg(unix)]
fn privilege_granted(path: &Path) -> Result<bool> {
    use std::os::unix::fs::PermissionsExt;

    const SETUID: u32 = 0o4000;
    Ok(std::fs::metadata(path)?.permissions().mode() & SETUID != 0)
}

#[cfg(not(unix))]
fn privilege_granted(_path: &Path) -> Result<bool> {
    Err(anyhow::anyhow!(
        "UNSUPPORTED_PLATFORM: core file privileges are only available on Unix"
    ))
}

pub fn get_core_privilege_status(paths: &[String]) -> Result<Vec<CorePrivilegeStatus>> {
    validate_core_paths(paths)?
        .into_iter()
        .map(|path| {
            Ok(CorePrivilegeStatus {
                path: path.to_string_lossy().into_owned(),
                granted: privilege_granted(&path)?,
            })
        })
        .collect()
}

#[cfg(target_os = "linux")]
fn run_pkexec(program: &str, operation: &str, paths: &[PathBuf]) -> Result<()> {
    let status = std::process::Command::new("pkexec")
        .arg(program)
        .arg(operation)
        .arg("--")
        .args(paths)
        .status()
        .with_context(|| format!("CORE_PRIVILEGE_ELEVATION_FAILED: unable to run {program}"))?;
    if !status.success() {
        bail!(
            "CORE_PRIVILEGE_ELEVATION_FAILED: {program} exited with {}",
            status.code().unwrap_or(-1)
        );
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn platform_set_core_privileges(paths: &[PathBuf], enabled: bool) -> Result<()> {
    let chmod = command_path("chmod")?;
    if enabled {
        let chown = command_path("chown")?;
        run_pkexec(chown, "root:root", paths)?;
        run_pkexec(chmod, "u+s", paths)
    } else {
        run_pkexec(chmod, "u-s", paths)
    }
}

#[cfg(target_os = "linux")]
fn command_path(name: &str) -> Result<&'static str> {
    let candidates: &[&str] = match name {
        "chown" => &["/usr/bin/chown", "/bin/chown"],
        "chmod" => &["/usr/bin/chmod", "/bin/chmod"],
        _ => &[],
    };
    candidates
        .iter()
        .copied()
        .find(|path| Path::new(path).is_file())
        .ok_or_else(|| anyhow::anyhow!("CORE_PRIVILEGE_ELEVATION_FAILED: {name} is unavailable"))
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
fn platform_set_core_privileges(paths: &[PathBuf], enabled: bool) -> Result<()> {
    let arguments = paths
        .iter()
        .map(|path| shell_quote(&path.to_string_lossy()))
        .collect::<Vec<_>>()
        .join(" ");
    let shell = if enabled {
        format!("/usr/sbin/chown root:admin -- {arguments} && /bin/chmod u+s -- {arguments}")
    } else {
        format!("/bin/chmod u-s -- {arguments}")
    };
    let script = format!(
        "do shell script \"{}\" with administrator privileges",
        applescript_escape(&shell)
    );
    let output = std::process::Command::new("/usr/bin/osascript")
        .args(["-e", &script])
        .output()
        .context("CORE_PRIVILEGE_ELEVATION_FAILED: unable to run osascript")?;
    if !output.status.success() {
        bail!(
            "CORE_PRIVILEGE_ELEVATION_FAILED: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn platform_set_core_privileges(_paths: &[PathBuf], _enabled: bool) -> Result<()> {
    Err(anyhow::anyhow!(
        "UNSUPPORTED_PLATFORM: core file privileges are only available on Linux and macOS"
    ))
}

pub fn set_core_privileges(paths: &[String], enabled: bool) -> Result<Vec<CorePrivilegeStatus>> {
    let validated = validate_core_paths(paths)?;
    platform_set_core_privileges(&validated, enabled)?;
    get_core_privilege_status(paths)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_core_path_list() {
        assert!(validate_core_paths(&[]).is_err());
    }

    #[test]
    fn rejects_relative_core_path() {
        assert!(validate_core_paths(&["mihomo".to_string()]).is_err());
    }
}
