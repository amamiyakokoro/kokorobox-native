#![deny(clippy::all)]

mod application;
mod executables;
mod icons;
mod macos_app_routing;
mod macos_service;
mod platform;
mod privileged_operations;
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
pub use macos_app_routing::{
    MacosApplicationRoutingAction, MacosApplicationRoutingConfiguration,
    MacosApplicationRoutingIdentifierKind, MacosApplicationRoutingProtocol,
    MacosApplicationRoutingRule, MacosApplicationRoutingState, MacosApplicationRoutingStatus,
    apply_macos_application_routing, get_macos_application_routing_status,
    open_macos_application_routing_settings, stop_macos_application_routing,
};
pub use macos_service::{
    MacOSManagedServiceStatus, get_macos_managed_service_status, open_macos_login_items_settings,
    register_macos_managed_service, reload_macos_managed_service, unregister_macos_managed_service,
};
#[cfg(not(target_os = "windows"))]
pub use non_windows::{
    current_user_sid, ensure_kokoro_box_core_firewall, is_running_as_admin, list_uwp_loopback_apps,
    set_uwp_loopback_exemption,
};
pub use platform::{
    LaunchAtLoginOptions, LaunchAtLoginStatus, NetworkContext, get_launch_at_login,
    get_network_context, set_launch_at_login, wait_for_network_context_change,
};
pub use privileged_operations::{
    ServiceLifecycleAction, ServiceLifecycleOptions, cleanup_legacy_macos_service,
    relaunch_current_application_with_privilege, repair_managed_file_permissions,
    run_service_lifecycle, stop_macos_managed_service,
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
    current_user_sid, ensure_kokoro_box_core_firewall, is_running_as_admin, list_uwp_loopback_apps,
    set_uwp_loopback_exemption,
};

#[cfg(not(target_os = "windows"))]
#[derive(Debug, Clone)]
pub struct UwpLoopbackApp {
    pub sid: String,
    pub package_name: String,
    pub display_name: String,
    pub enabled: bool,
}

#[cfg(target_os = "windows")]
pub use windows::UwpLoopbackApp;

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
    pub service_lifecycle: bool,
    pub managed_file_permissions: bool,
    pub macos_service_management: bool,
    pub macos_application_routing: bool,
    pub windows_privilege_relaunch: bool,
    pub windows_uwp_loopback: bool,
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
        service_lifecycle: cfg!(any(
            target_os = "windows",
            target_os = "macos",
            target_os = "linux"
        )),
        managed_file_permissions: cfg!(any(target_os = "macos", target_os = "linux")),
        macos_service_management: cfg!(target_os = "macos"),
        macos_application_routing: cfg!(target_os = "macos"),
        windows_privilege_relaunch: cfg!(target_os = "windows"),
        windows_uwp_loopback: cfg!(target_os = "windows"),
        core_file_privileges: cfg!(any(target_os = "macos", target_os = "linux")),
        service_identity: true,
    }
}

#[cfg(test)]
mod capability_tests {
    use super::*;

    #[test]
    fn reports_platform_specific_operation_boundaries() {
        let capabilities = native_capabilities();

        assert_eq!(
            capabilities.service_lifecycle,
            cfg!(any(
                target_os = "windows",
                target_os = "macos",
                target_os = "linux"
            ))
        );
        assert_eq!(
            capabilities.managed_file_permissions,
            cfg!(any(target_os = "macos", target_os = "linux"))
        );
        assert_eq!(
            capabilities.windows_privilege_relaunch,
            cfg!(target_os = "windows")
        );
        assert_eq!(
            capabilities.windows_uwp_loopback,
            cfg!(target_os = "windows")
        );
    }

    #[test]
    fn exposes_typed_macos_application_routing_contract() {
        let declarations = include_str!("../napi/index.d.ts");
        let javascript = include_str!("../napi/index.js");

        for export in [
            "applyMacosApplicationRouting",
            "getMacosApplicationRoutingStatus",
            "stopMacosApplicationRouting",
            "openMacosApplicationRoutingSettings",
        ] {
            assert!(declarations.contains(export));
            assert!(javascript.contains(export));
        }
        assert!(!declarations.contains("invokeMacosApplicationRouting"));
        assert!(!javascript.contains("invokeMacosApplicationRouting"));
    }
}
