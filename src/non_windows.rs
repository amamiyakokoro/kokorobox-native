use crate::FirewallRule;

fn windows_only(operation: &str) -> anyhow::Error {
    anyhow::anyhow!("UNSUPPORTED_PLATFORM: {operation} is only available on Windows")
}

pub fn current_user_sid() -> anyhow::Result<String> {
    Err(windows_only("current_user_sid"))
}

pub fn is_running_as_admin() -> anyhow::Result<bool> {
    Err(windows_only("is_running_as_admin"))
}

pub fn run_elevated(_command: &str, _args: &[String]) -> anyhow::Result<u32> {
    Err(windows_only("run_elevated"))
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
        assert!(run_elevated("ignored", &[]).is_err());
        assert!(setup_firewall_rules(Vec::new()).is_err());
    }
}
