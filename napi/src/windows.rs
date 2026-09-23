use napi::bindgen_prelude::*;
use napi_derive::napi;

use crate::error::map_err;

#[napi(object)]
pub struct JsUwpLoopbackApp {
    pub id: String,
    pub package_family_name: String,
    pub package_full_name: Option<String>,
    pub display_name: String,
    pub description: Option<String>,
    pub enabled: bool,
    pub category: String,
    pub package_type: Option<String>,
    pub framework: bool,
    pub resource_package: bool,
}

#[napi]
pub fn list_uwp_loopback_apps() -> Result<Vec<JsUwpLoopbackApp>> {
    kokorobox_native::list_uwp_loopback_apps()
        .map(|apps| {
            apps.into_iter()
                .map(|app| JsUwpLoopbackApp {
                    id: app.id,
                    package_family_name: app.package_family_name,
                    package_full_name: app.package_full_name,
                    display_name: app.display_name,
                    description: app.description,
                    enabled: app.enabled,
                    category: app.category.as_str().to_string(),
                    package_type: app
                        .package_type
                        .map(|package_type| package_type.as_str().to_string()),
                    framework: app.framework,
                    resource_package: app.resource_package,
                })
                .collect()
        })
        .map_err(map_err)
}

#[napi]
pub fn set_uwp_loopback_exemption(id: String, enabled: bool) -> Result<()> {
    kokorobox_native::set_uwp_loopback_exemption(&id, enabled).map_err(map_err)
}

#[napi]
pub fn get_current_user_sid() -> Result<String> {
    kokorobox_native::current_user_sid().map_err(map_err)
}

#[napi]
pub fn is_running_as_admin() -> Result<bool> {
    kokorobox_native::is_running_as_admin().map_err(map_err)
}

#[napi]
pub fn get_windows_service_status() -> Result<String> {
    kokorobox_native::get_windows_service_status()
        .map(|status| status.as_str().to_string())
        .map_err(map_err)
}

#[napi]
pub fn ensure_kokoro_box_core_firewall(
    mihomo_path: String,
    mihomo_alpha_path: String,
    application_path: String,
) -> Result<()> {
    kokorobox_native::ensure_kokoro_box_core_firewall(
        &mihomo_path,
        &mihomo_alpha_path,
        &application_path,
    )
    .map_err(map_err)
}
