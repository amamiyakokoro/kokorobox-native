//! OS status first, bounded fixed-argument Service CLI compatibility second.
use anyhow::{Context, Result, bail};
use std::{path::Path, process::Command, time::Duration};

pub fn get_service_process_status(executable: &str) -> Result<String> {
    #[cfg(target_os = "windows")]
    let state = crate::get_windows_service_status().map(|s| s.as_str());
    #[cfg(target_os = "macos")]
    let state = crate::get_macos_service_process_status().map(|s| s.as_str());
    #[cfg(target_os = "linux")]
    let state = crate::get_linux_service_status().map(|s| s.as_str());
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    let state: Result<&str> = Ok("unknown");
    if let Ok(state) = state
        && state != "unknown"
    {
        return Ok(state.into());
    }
    service_cli_status(executable)
}
fn service_cli_status(executable: &str) -> Result<String> {
    if !Path::new(executable).is_absolute() {
        bail!("Service executable path must be absolute");
    }
    let executable = std::fs::canonicalize(executable).context("resolve Service executable")?;
    let name = executable
        .file_name()
        .and_then(|v| v.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if !matches!(
        name.as_str(),
        "kokorobox-service" | "kokorobox-service.exe" | "sparkle-service" | "sparkle-service.exe"
    ) {
        bail!("invalid Service executable");
    }
    let mut command = Command::new(executable);
    command.args(["service", "status"]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let result = crate::core_validation::run(command, Duration::from_millis(2500))?;
    if matches!(result.outcome.as_str(), "timeout" | "output-limit") {
        return Ok("unknown".into());
    }
    Ok(parse_service_status(&result.output)
        .unwrap_or("unknown")
        .into())
}

fn parse_service_status(output: &str) -> Option<&'static str> {
    let mut offset = 0;
    let mut last = None;
    while let Some(start) = output[offset..].find('{') {
        offset += start;
        let mut values =
            serde_json::Deserializer::from_str(&output[offset..]).into_iter::<serde_json::Value>();
        if let Some(Ok(value)) = values.next() {
            last = match value.pointer("/status/state").and_then(|v| v.as_str()) {
                Some("running") => Some("running"),
                Some("stopped") => Some("stopped"),
                Some("paused") => Some("paused"),
                Some("not-installed") => Some("not-installed"),
                Some("unknown") => Some("unknown"),
                _ => last,
            };
            offset += values.byte_offset();
        } else {
            offset += 1;
        }
    }
    last
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn handles_nested_pretty_json_multiple_entries_and_trailing_text() {
        let text = "prefix {\n \"status\": {\"state\": \"stopped\"}, \"msg\": \"{quoted}\"\n}\n{\"status\":{\"state\":\"running\"}} trailing";
        assert_eq!(parse_service_status(text), Some("running"));
        assert_eq!(
            parse_service_status("{\"status\":{\"state\":\"invented\"}}"),
            None
        );
        assert_eq!(parse_service_status("{bad json}"), None);
    }
    #[test]
    fn rejects_arbitrary_cli_programs() {
        assert!(service_cli_status("/bin/sh").is_err());
    }
    #[cfg(unix)]
    #[test]
    fn cli_uses_only_fixed_arguments_and_reads_status_after_nonzero_exit() {
        use std::os::unix::fs::PermissionsExt;
        let root = std::env::temp_dir().join(format!("kokoro-status-test-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("kokorobox-service");
        std::fs::write(&path, "#!/bin/sh\n[ \"$1\" = service ] && [ \"$2\" = status ] && [ \"$#\" = 2 ] || exit 2\nprintf '%s' '{\"status\":{\"state\":\"stopped\"}}'\nexit 1\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        let result = service_cli_status(path.to_str().unwrap());
        let _ = std::fs::remove_dir_all(root);
        assert_eq!(result.unwrap(), "stopped");
    }
}
