use crate::FirewallRule;
use crate::UwpLoopbackApp;

pub fn list_uwp_loopback_apps() -> anyhow::Result<Vec<UwpLoopbackApp>> {
    Err(windows_only("list_uwp_loopback_apps"))
}

pub fn set_uwp_loopback_exemption(_sid: &str, _enabled: bool) -> anyhow::Result<()> {
    Err(windows_only("set_uwp_loopback_exemption"))
}

fn windows_only(operation: &str) -> anyhow::Error {
    anyhow::anyhow!("UNSUPPORTED_PLATFORM: {operation} is only available on Windows")
}

pub fn current_user_sid() -> anyhow::Result<String> {
    Err(windows_only("current_user_sid"))
}

pub fn is_running_as_admin() -> anyhow::Result<bool> {
    Err(windows_only("is_running_as_admin"))
}

pub fn setup_firewall_rules(_rules: Vec<FirewallRule>) -> anyhow::Result<()> {
    Err(windows_only("setup_firewall_rules"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_only_apis_do_not_report_success() {
        let error = current_user_sid().unwrap_err();
        assert!(error.to_string().starts_with("UNSUPPORTED_PLATFORM:"));
        assert!(is_running_as_admin().is_err());
        assert!(setup_firewall_rules(Vec::new()).is_err());
    }
}
