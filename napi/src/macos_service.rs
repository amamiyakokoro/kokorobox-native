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

pub struct MacosServiceProcessStatusTask;

#[napi]
impl Task for MacosServiceProcessStatusTask {
    type Output = String;
    type JsValue = String;

    fn compute(&mut self) -> Result<Self::Output> {
        kokorobox_native::get_macos_service_process_status()
            .map(|status| status.as_str().to_string())
            .map_err(map_err)
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(output)
    }
}

#[napi]
pub fn get_macos_service_process_status() -> AsyncTask<MacosServiceProcessStatusTask> {
    AsyncTask::new(MacosServiceProcessStatusTask)
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
