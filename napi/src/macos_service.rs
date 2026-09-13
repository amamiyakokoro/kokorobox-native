use napi::bindgen_prelude::*;
use napi_derive::napi;

use crate::error::map_err;

fn status_string(status: kokorobox_native::MacOSManagedServiceStatus) -> String {
    status.as_str().to_string()
}

#[napi]
pub fn get_macos_managed_service_status(plist_name: String) -> Result<String> {
    kokorobox_native::get_macos_managed_service_status(&plist_name)
        .map(status_string)
        .map_err(map_err)
}

#[napi]
pub fn register_macos_managed_service(plist_name: String) -> Result<String> {
    kokorobox_native::register_macos_managed_service(&plist_name)
        .map(status_string)
        .map_err(map_err)
}

#[napi]
pub fn unregister_macos_managed_service(plist_name: String) -> Result<String> {
    kokorobox_native::unregister_macos_managed_service(&plist_name)
        .map(status_string)
        .map_err(map_err)
}

#[napi]
pub fn reload_macos_managed_service(plist_name: String) -> Result<String> {
    kokorobox_native::reload_macos_managed_service(&plist_name)
        .map(status_string)
        .map_err(map_err)
}

#[napi]
pub fn open_macos_login_items_settings() -> Result<()> {
    kokorobox_native::open_macos_login_items_settings().map_err(map_err)
}
