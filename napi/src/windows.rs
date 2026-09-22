use napi::bindgen_prelude::*;
use napi_derive::napi;

use crate::error::map_err;

#[napi(object)]
pub struct JsUwpLoopbackApp {
    pub sid: String,
    pub package_name: String,
    pub display_name: String,
    pub enabled: bool,
}

#[napi]
pub fn list_uwp_loopback_apps() -> Result<Vec<JsUwpLoopbackApp>> {
    kokorobox_native::list_uwp_loopback_apps()
        .map(|apps| {
            apps.into_iter()
                .map(|app| JsUwpLoopbackApp {
                    sid: app.sid,
                    package_name: app.package_name,
                    display_name: app.display_name,
                    enabled: app.enabled,
                })
                .collect()
        })
        .map_err(map_err)
}

#[napi]
pub fn set_uwp_loopback_exemption(sid: String, enabled: bool) -> Result<()> {
    kokorobox_native::set_uwp_loopback_exemption(&sid, enabled).map_err(map_err)
}

#[napi(object)]
pub struct JsFirewallRule {
    pub name: String,
    pub application_path: String,
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
pub fn setup_firewall_rules(rules: Vec<JsFirewallRule>) -> Result<()> {
    let rules = rules
        .into_iter()
        .map(|rule| kokorobox_native::FirewallRule {
            name: rule.name,
            application_path: rule.application_path,
        })
        .collect();
    kokorobox_native::setup_firewall_rules(rules).map_err(map_err)
}
