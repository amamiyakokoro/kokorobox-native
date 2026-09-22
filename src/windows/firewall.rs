use anyhow::{Result, anyhow};
use std::path::Path;
use windows::Win32::Foundation::VARIANT_TRUE;
use windows::Win32::NetworkManagement::WindowsFirewall::{
    INetFwPolicy2, INetFwRule, INetFwRules, NET_FW_ACTION_ALLOW, NET_FW_PROFILE2_ALL,
    NET_FW_RULE_DIR_IN, NetFwPolicy2, NetFwRule,
};
use windows::Win32::System::Com::{
    CLSCTX_ALL, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoCreateInstance, CoInitializeEx,
    CoUninitialize,
};
use windows::core::BSTR;

struct ComApartment;

impl ComApartment {
    fn init() -> Result<Self> {
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE)
                .ok()
                .map_err(|error| anyhow!("CoInitializeEx failed: {error}"))?;
        }
        Ok(Self)
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}

fn firewall_rules() -> Result<INetFwRules> {
    unsafe {
        let policy: INetFwPolicy2 = CoCreateInstance(&NetFwPolicy2, None, CLSCTX_ALL)
            .map_err(|error| anyhow!("CoCreateInstance(NetFwPolicy2) failed: {error}"))?;
        policy
            .Rules()
            .map_err(|error| anyhow!("INetFwPolicy2.Rules failed: {error}"))
    }
}

fn remove_firewall_rule(fw_rules: &INetFwRules, name: &str) {
    unsafe {
        let _ = fw_rules.Remove(&BSTR::from(name));
    }
}

fn add_firewall_rule(fw_rules: &INetFwRules, name: &str, application_path: &str) -> Result<()> {
    unsafe {
        let fw_rule: INetFwRule = CoCreateInstance(&NetFwRule, None, CLSCTX_ALL)
            .map_err(|error| anyhow!("CoCreateInstance(NetFwRule) failed: {error}"))?;
        fw_rule
            .SetName(&BSTR::from(name))
            .map_err(|error| anyhow!("SetName({name}) failed: {error}"))?;
        fw_rule
            .SetApplicationName(&BSTR::from(application_path))
            .map_err(|error| anyhow!("SetApplicationName({name}) failed: {error}"))?;
        fw_rule
            .SetDirection(NET_FW_RULE_DIR_IN)
            .map_err(|error| anyhow!("SetDirection({name}) failed: {error}"))?;
        fw_rule
            .SetAction(NET_FW_ACTION_ALLOW)
            .map_err(|error| anyhow!("SetAction({name}) failed: {error}"))?;
        fw_rule
            .SetProfiles(NET_FW_PROFILE2_ALL.0)
            .map_err(|error| anyhow!("SetProfiles({name}) failed: {error}"))?;
        fw_rule
            .SetEnabled(VARIANT_TRUE)
            .map_err(|error| anyhow!("SetEnabled({name}) failed: {error}"))?;
        fw_rules
            .Add(&fw_rule)
            .map_err(|error| anyhow!("Add({name}) failed: {error}"))?;
    }
    Ok(())
}

fn validate_executable_path(path: &str, expected_names: &[&str]) -> Result<()> {
    let executable = Path::new(path);
    if path.contains('\0') || !executable.is_absolute() {
        return Err(anyhow!(
            "Firewall executable path must be absolute and NUL-free"
        ));
    }
    let file_name = executable
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow!("Firewall executable path has no filename"))?;
    if !expected_names
        .iter()
        .any(|expected| file_name.eq_ignore_ascii_case(expected))
    {
        return Err(anyhow!("Unexpected firewall executable: {file_name}"));
    }
    Ok(())
}

pub fn ensure_kokoro_box_core_firewall(
    mihomo_path: &str,
    mihomo_alpha_path: &str,
    application_path: &str,
) -> Result<()> {
    validate_executable_path(mihomo_path, &["mihomo.exe"])?;
    validate_executable_path(mihomo_alpha_path, &["mihomo-alpha.exe"])?;
    validate_executable_path(application_path, &["KokoroBox.exe", "electron.exe"])?;

    let _com = ComApartment::init()?;
    let fw_rules = firewall_rules()?;

    let rules = [
        ("mihomo", mihomo_path),
        ("mihomo-alpha", mihomo_alpha_path),
        ("KokoroBox", application_path),
    ];
    for (name, _) in &rules {
        remove_firewall_rule(&fw_rules, name);
    }

    for (name, path) in rules {
        add_firewall_rule(&fw_rules, name, path)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_executable_path;

    #[test]
    fn rejects_unrelated_firewall_executables() {
        assert!(validate_executable_path(r"C:\KokoroBox\mihomo.exe", &["mihomo.exe"]).is_ok());
        assert!(validate_executable_path(r"C:\KokoroBox\other.exe", &["mihomo.exe"]).is_err());
        assert!(validate_executable_path("mihomo.exe", &["mihomo.exe"]).is_err());
        assert!(validate_executable_path("C:\\mihomo.exe\0junk", &["mihomo.exe"]).is_err());
    }
}
