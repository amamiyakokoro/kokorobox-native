#![deny(clippy::all)]

mod icons;
mod rules;
mod application;

#[cfg(not(target_os = "windows"))]
mod non_windows;
#[cfg(target_os = "windows")]
mod windows;

pub use application::{
    ApplicationInfo, ApplicationScanResult, inspect_application, scan_windows_applications,
};
pub use icons::{file_to_data_url, get_app_name};
#[cfg(not(target_os = "windows"))]
pub use non_windows::{current_user_sid, is_running_as_admin, run_elevated, setup_firewall_rules};
pub use rules::{
    RuleConvertOptions, RuleOutputInfo, RuleSkippedItem, RuleStringResult, rule_file_to_string,
};
#[cfg(target_os = "windows")]
pub use windows::{current_user_sid, is_running_as_admin, run_elevated, setup_firewall_rules};

#[derive(Debug, Clone)]
pub struct FirewallRule {
    pub name: String,
    pub application_path: String,
}

#[derive(Debug, Clone, Copy)]
pub struct NativeCapabilities {
    pub application_inspection: bool,
    pub windows_application_scan: bool,
    pub windows_account: bool,
    pub windows_elevation: bool,
    pub windows_firewall: bool,
}

pub fn native_capabilities() -> NativeCapabilities {
    NativeCapabilities {
        application_inspection: true,
        windows_application_scan: cfg!(target_os = "windows"),
        windows_account: cfg!(target_os = "windows"),
        windows_elevation: cfg!(target_os = "windows"),
        windows_firewall: cfg!(target_os = "windows"),
    }
}
