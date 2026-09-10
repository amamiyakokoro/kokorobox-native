use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};

use crate::{file_to_data_url, get_app_name};

#[derive(Debug, Clone)]
pub struct ApplicationInfo {
    pub executable_path: String,
    pub executable_name: String,
    pub identifier: String,
    pub identifier_kind: String,
    pub icon_data_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ApplicationScanResult {
    pub applications: Vec<ApplicationInfo>,
    pub truncated: bool,
    pub unreadable_directory_count: u32,
}

fn canonicalize(path: impl AsRef<Path>) -> Result<PathBuf> {
    fs::canonicalize(path.as_ref()).with_context(|| {
        format!(
            "APPLICATION_NOT_FOUND: unable to resolve {}",
            path.as_ref().display()
        )
    })
}

fn display_name(path: &Path) -> String {
    get_app_name(path)
        .ok()
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| {
            path.file_stem()
                .or_else(|| path.file_name())
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.display().to_string())
        })
}

fn icon_data_url(path: &Path) -> Option<String> {
    file_to_data_url(path).ok().filter(|icon| !icon.is_empty())
}

#[cfg(target_os = "windows")]
fn normalize_windows_path(path: PathBuf) -> String {
    let path = path.to_string_lossy().into_owned().replace('/', "\\");
    if let Some(path) = path.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{path}")
    } else if let Some(path) = path.strip_prefix(r"\\?\") {
        path.to_string()
    } else {
        path
    }
}

#[cfg(target_os = "windows")]
fn inspect_windows_application(path: PathBuf) -> Result<ApplicationInfo> {
    let metadata = fs::metadata(&path)?;
    if !metadata.is_file()
        || !path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
    {
        bail!("INVALID_APPLICATION: Windows application routing requires an existing .exe file")
    }

    let executable_path = normalize_windows_path(path.clone());
    let executable_name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| executable_path.clone());
    Ok(ApplicationInfo {
        executable_path: executable_path.clone(),
        executable_name: display_name(&path),
        identifier: executable_path,
        identifier_kind: "windows-executable".to_string(),
        icon_data_url: icon_data_url(&path),
    })
}

#[cfg(target_os = "macos")]
fn macos_signing_identifier(path: &Path) -> Result<String> {
    use std::process::Command;

    let output = Command::new("/usr/bin/codesign")
        .args(["--display", "--verbose=2"])
        .arg(path)
        .output()
        .context("APPLICATION_IDENTIFIER_UNAVAILABLE: unable to inspect macOS code signature")?;
    let output = String::from_utf8_lossy(&output.stderr);
    output
        .lines()
        .find_map(|line| line.strip_prefix("Identifier="))
        .map(str::trim)
        .filter(|identifier| !identifier.is_empty() && !identifier.contains(['\r', '\n']))
        .map(str::to_string)
        .ok_or_else(|| anyhow::anyhow!("APPLICATION_IDENTIFIER_UNAVAILABLE: selected application has no signing identifier"))
}

#[cfg(target_os = "macos")]
fn inspect_macos_application(path: PathBuf) -> Result<ApplicationInfo> {
    let metadata = fs::metadata(&path)?;
    let is_bundle = metadata.is_dir()
        && path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("app"));
    let is_executable = metadata.is_file()
        && {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                metadata.permissions().mode() & 0o111 != 0
            }
            #[cfg(not(unix))]
            {
                false
            }
        };
    if !is_bundle && !is_executable {
        bail!("INVALID_APPLICATION: select a signed macOS application or executable")
    }

    let executable_path = path.to_string_lossy().into_owned();
    Ok(ApplicationInfo {
        executable_name: display_name(&path),
        identifier: macos_signing_identifier(&path)?,
        identifier_kind: "macos-signing-identifier".to_string(),
        icon_data_url: icon_data_url(&path),
        executable_path,
    })
}

#[cfg(target_os = "linux")]
fn inspect_linux_application(path: PathBuf) -> Result<ApplicationInfo> {
    use std::os::unix::fs::PermissionsExt;

    let metadata = fs::metadata(&path)?;
    if !metadata.is_file() || metadata.permissions().mode() & 0o111 == 0 {
        bail!("INVALID_APPLICATION: application routing requires an executable Linux file")
    }
    let executable_path = path.to_string_lossy().into_owned();
    Ok(ApplicationInfo {
        executable_name: display_name(&path),
        identifier: executable_path.clone(),
        identifier_kind: "linux-executable".to_string(),
        icon_data_url: icon_data_url(&path),
        executable_path,
    })
}

pub fn inspect_application(path: impl AsRef<Path>) -> Result<ApplicationInfo> {
    let path = canonicalize(path)?;
    #[cfg(target_os = "windows")]
    return inspect_windows_application(path);
    #[cfg(target_os = "macos")]
    return inspect_macos_application(path);
    #[cfg(target_os = "linux")]
    return inspect_linux_application(path);
    #[allow(unreachable_code)]
    bail!("UNSUPPORTED_PLATFORM: application inspection is not supported on this platform")
}

#[cfg(target_os = "windows")]
pub fn scan_windows_applications(
    root_directory: impl AsRef<Path>,
    maximum_results: u32,
    excluded_executable_names: &[String],
) -> Result<ApplicationScanResult> {
    let root_directory = canonicalize(root_directory)?;
    if !fs::metadata(&root_directory)?.is_dir() {
        bail!("INVALID_DIRECTORY: application scan requires an existing directory")
    }

    let maximum_results = maximum_results.clamp(1, 4096) as usize;
    let excluded_executable_names = excluded_executable_names
        .iter()
        .map(|name| name.to_lowercase())
        .collect::<std::collections::HashSet<_>>();
    let mut pending = vec![root_directory];
    let mut applications = Vec::new();
    let mut unreadable_directory_count = 0;
    let mut truncated = false;

    while let Some(directory) = pending.pop() {
        if applications.len() >= maximum_results {
            truncated = true;
            break;
        }
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) => {
                if applications.is_empty() && pending.is_empty() {
                    return Err(error).context("APPLICATION_SCAN_FAILED: unable to read selected directory");
                }
                unreadable_directory_count += 1;
                continue;
            }
        };
        let mut entries = entries.filter_map(Result::ok).collect::<Vec<_>>();
        entries.sort_by(|left, right| left.file_name().cmp(&right.file_name()));
        let mut subdirectories = Vec::new();
        for entry in entries {
            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(_) => continue,
            };
            if file_type.is_symlink() {
                continue;
            }
            if file_type.is_dir() {
                subdirectories.push(entry.path());
                continue;
            }
            if !file_type.is_file() {
                continue;
            }
            let path = entry.path();
            if !path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
            {
                continue;
            }
            let file_name = entry.file_name().to_string_lossy().to_lowercase();
            if excluded_executable_names.contains(&file_name) {
                continue;
            }
            if applications.len() >= maximum_results {
                truncated = true;
                break;
            }
            applications.push(inspect_windows_application(path)?);
        }
        pending.extend(subdirectories.into_iter().rev());
    }

    Ok(ApplicationScanResult {
        applications,
        truncated,
        unreadable_directory_count,
    })
}

#[cfg(not(target_os = "windows"))]
pub fn scan_windows_applications(
    _root_directory: impl AsRef<Path>,
    _maximum_results: u32,
    _excluded_executable_names: &[String],
) -> Result<ApplicationScanResult> {
    bail!("UNSUPPORTED_PLATFORM: Windows application scanning is only available on Windows")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_application_has_stable_error_prefix() {
        let error = inspect_application("/definitely/not/a/kokorobox/application").unwrap_err();
        assert!(error.to_string().starts_with("APPLICATION_NOT_FOUND:"));
    }
}
