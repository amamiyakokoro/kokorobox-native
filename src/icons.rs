use std::path::Path;

pub fn file_to_data_url(path: impl AsRef<Path>) -> anyhow::Result<String> {
    #[cfg(target_os = "macos")]
    if let Some(icon) = macos_bundle_icon_data_url(path.as_ref()) {
        return Ok(icon);
    }

    file_icon::file_to_data_url(path).map_err(|error| anyhow::anyhow!(error.to_string()))
}

pub fn get_app_name(path: impl AsRef<Path>) -> anyhow::Result<String> {
    file_icon::get_app_name_with_options(path, file_icon::FileIconOptions::default())
        .map_err(|error| anyhow::anyhow!(error.to_string()))
}

/// Resolve a bundle's declared `.icns` resource before falling back to
/// `file-icon`. `file-icon` can obtain an application icon on many systems,
/// but handing it a macOS `.app` directory often returns the generic folder
/// icon instead of the bundle icon.
#[cfg(target_os = "macos")]
fn macos_bundle_icon_data_url(bundle_path: &Path) -> Option<String> {
    use std::{
        fs,
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };

    if !bundle_path.is_dir()
        || !bundle_path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("app"))
    {
        return None;
    }

    let info_plist = bundle_path.join("Contents").join("Info.plist");
    let output = Command::new("/usr/bin/plutil")
        .args(["-extract", "CFBundleIconFile", "raw", "-o", "-"])
        .arg(&info_plist)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let icon_name = String::from_utf8(output.stdout).ok()?.trim().to_owned();
    // `CFBundleIconFile` must name a resource directly. Reject separators and
    // traversal so a malformed bundle cannot escape Contents/Resources.
    if icon_name.is_empty()
        || icon_name == "."
        || icon_name == ".."
        || Path::new(&icon_name)
            .file_name()
            .is_none_or(|name| name.to_string_lossy().as_ref() != icon_name.as_str())
    {
        return None;
    }
    let resource_name = if Path::new(&icon_name).extension().is_some() {
        icon_name
    } else {
        format!("{icon_name}.icns")
    };
    let resource = bundle_path.join("Contents").join("Resources").join(resource_name);
    if !resource.is_file() {
        return None;
    }

    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_nanos();
    let temporary_directory = std::env::temp_dir().join(format!(
        "kokorobox-native-icon-{}-{nonce}",
        std::process::id()
    ));
    // `create_dir` is atomic, so the converter never writes through a
    // predictable file or symlink in the shared temporary directory.
    if fs::create_dir(&temporary_directory).is_err() {
        return None;
    }
    let output_path = temporary_directory.join("icon.png");
    let converted = Command::new("/usr/bin/sips")
        .args(["-s", "format", "png"])
        .arg(&resource)
        .args(["--out"])
        .arg(&output_path)
        .output()
        .ok()
        .is_some_and(|output| output.status.success());
    if !converted {
        let _ = fs::remove_dir_all(temporary_directory);
        return None;
    }

    let icon = file_icon::file_to_data_url(&output_path)
        .ok()
        .filter(|icon| !icon.is_empty());
    let _ = fs::remove_dir_all(temporary_directory);
    icon
}
