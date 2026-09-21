#![deny(clippy::all)]

mod application;
mod executables;
mod icons;
mod macos_app_routing;
mod macos_service;
mod platform;
mod privileges;
mod rules;
mod service_identity;

#[cfg(not(target_os = "windows"))]
mod non_windows;
#[cfg(target_os = "windows")]
mod windows;

pub use application::{
    ApplicationInfo, ApplicationScanResult, inspect_application, scan_windows_applications,
};
pub use executables::{ExecutableCandidate, ExecutableSearchOptions, find_executables};
pub use icons::{file_to_data_url, get_app_name};
pub use macos_app_routing::invoke_macos_application_routing;
pub use macos_service::{
    MacOSManagedServiceStatus, get_macos_managed_service_status, open_macos_login_items_settings,
    register_macos_managed_service, reload_macos_managed_service, unregister_macos_managed_service,
};
#[cfg(not(target_os = "windows"))]
pub use non_windows::{
    current_user_sid, is_running_as_admin, launch_elevated, launch_unelevated, run_elevated,
    setup_firewall_rules,
};
pub use platform::{
    LaunchAtLoginOptions, LaunchAtLoginStatus, NetworkContext, get_launch_at_login,
    get_network_context, set_launch_at_login, wait_for_network_context_change,
};
pub use privileges::{CorePrivilegeStatus, get_core_privilege_status, set_core_privileges};
pub use rules::{
    RuleConvertOptions, RuleOutputInfo, RuleSkippedItem, RuleStringResult, rule_file_to_string,
};
pub use service_identity::{
    LegacyServiceIdentity, ServiceIdentity, ServiceIdentityInfo, ServiceIdentityOptions,
    delete_service_identity, open_service_identity,
};
#[cfg(target_os = "windows")]
pub use windows::{
    current_user_sid, is_running_as_admin, launch_elevated, launch_unelevated, run_elevated,
    setup_firewall_rules,
};

#[derive(Debug, Clone)]
pub struct FirewallRule {
    pub name: String,
    pub application_path: String,
}

#[derive(Debug, Clone, Copy)]
pub struct NativeCapabilities {
    pub application_inspection: bool,
    pub executable_discovery: bool,
    pub windows_application_scan: bool,
    pub windows_account: bool,
    pub windows_elevation: bool,
    pub windows_firewall: bool,
    pub launch_at_login: bool,
    pub network_context: bool,
    pub network_monitor: bool,
    pub macos_service_management: bool,
    pub macos_application_routing: bool,
    pub core_file_privileges: bool,
    pub service_identity: bool,
}

pub fn native_capabilities() -> NativeCapabilities {
    NativeCapabilities {
        application_inspection: true,
        executable_discovery: true,
        windows_application_scan: cfg!(target_os = "windows"),
        windows_account: cfg!(target_os = "windows"),
        windows_elevation: cfg!(target_os = "windows"),
        windows_firewall: cfg!(target_os = "windows"),
        launch_at_login: cfg!(any(
            target_os = "windows",
            target_os = "macos",
            target_os = "linux"
        )),
        network_context: cfg!(any(
            target_os = "windows",
            target_os = "macos",
            target_os = "linux"
        )),
        network_monitor: cfg!(any(
            target_os = "windows",
            target_os = "macos",
            target_os = "linux"
        )),
        macos_service_management: cfg!(target_os = "macos"),
        macos_application_routing: cfg!(target_os = "macos"),
        core_file_privileges: cfg!(any(target_os = "macos", target_os = "linux")),
        service_identity: true,
    }
}
