use anyhow::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacosServiceProcessStatus {
    Running,
    Stopped,
    NotInstalled,
    Unknown,
}

impl MacosServiceProcessStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Stopped => "stopped",
            Self::NotInstalled => "not-installed",
            Self::Unknown => "unknown",
        }
    }
}

fn parse_launchctl_status(output: &str, success: bool) -> Option<MacosServiceProcessStatus> {
    if success {
        return Some(
            if output.contains("state = running") || output.contains("\n\tpid = ") {
                MacosServiceProcessStatus::Running
            } else {
                MacosServiceProcessStatus::Stopped
            },
        );
    }
    let lower = output.to_ascii_lowercase();
    if lower.contains("could not find service") || lower.contains("service not found") {
        Some(MacosServiceProcessStatus::NotInstalled)
    } else {
        None
    }
}

#[cfg(target_os = "macos")]
pub fn get_macos_service_process_status() -> Result<MacosServiceProcessStatus> {
    use anyhow::{Context, anyhow};
    use std::process::Command;

    // This fixed, read-only launchd query mirrors the Service's existing
    // status probe without launching the privileged Service executable.
    let output = Command::new("/bin/launchctl")
        .args(["print", "system/KokoroBoxService"])
        .output()
        .context("Query KokoroBox Service in launchd")?;
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    parse_launchctl_status(&combined, output.status.success())
        .ok_or_else(|| anyhow!("launchctl could not determine KokoroBox Service state"))
}

#[cfg(not(target_os = "macos"))]
pub fn get_macos_service_process_status() -> Result<MacosServiceProcessStatus> {
    anyhow::bail!("macOS Service status is unavailable on this platform")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interprets_launchd_output_like_the_service() {
        assert_eq!(
            parse_launchctl_status("state = running", true),
            Some(MacosServiceProcessStatus::Running)
        );
        assert_eq!(
            parse_launchctl_status("service = {\n\tpid = 42\n}", true),
            Some(MacosServiceProcessStatus::Running)
        );
        assert_eq!(
            parse_launchctl_status("state = exited", true),
            Some(MacosServiceProcessStatus::Stopped)
        );
        assert_eq!(
            parse_launchctl_status("Could not find service", false),
            Some(MacosServiceProcessStatus::NotInstalled)
        );
        assert_eq!(parse_launchctl_status("permission denied", false), None);
    }
}
