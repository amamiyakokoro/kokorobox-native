use napi::bindgen_prelude::*;
use napi_derive::napi;

use crate::error::map_err;

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
pub fn run_elevated(command: String, args: Option<Vec<String>>) -> Result<u32> {
    kokorobox_native::run_elevated(&command, args.as_deref().unwrap_or(&[])).map_err(map_err)
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
