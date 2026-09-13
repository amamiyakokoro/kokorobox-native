use napi::bindgen_prelude::*;
use napi_derive::napi;

use crate::error::map_err;

#[napi(object)]
pub struct JsNativeCapabilities {
    pub application_inspection: bool,
    pub windows_application_scan: bool,
    pub windows_account: bool,
    pub windows_elevation: bool,
    pub windows_firewall: bool,
    pub launch_at_login: bool,
    pub network_context: bool,
    pub macos_service_management: bool,
    pub core_file_privileges: bool,
}

#[napi(object)]
pub struct JsApplicationInfo {
    pub executable_path: String,
    pub executable_name: String,
    pub identifier: String,
    pub identifier_kind: String,
    pub icon_data_url: Option<String>,
}

#[napi(object)]
pub struct JsApplicationScanResult {
    pub applications: Vec<JsApplicationInfo>,
    pub truncated: bool,
    pub unreadable_directory_count: u32,
}

impl From<kokorobox_native::ApplicationInfo> for JsApplicationInfo {
    fn from(value: kokorobox_native::ApplicationInfo) -> Self {
        Self {
            executable_path: value.executable_path,
            executable_name: value.executable_name,
            identifier: value.identifier,
            identifier_kind: value.identifier_kind,
            icon_data_url: value.icon_data_url,
        }
    }
}

#[napi]
pub fn get_native_capabilities() -> JsNativeCapabilities {
    let capabilities = kokorobox_native::native_capabilities();
    JsNativeCapabilities {
        application_inspection: capabilities.application_inspection,
        windows_application_scan: capabilities.windows_application_scan,
        windows_account: capabilities.windows_account,
        windows_elevation: capabilities.windows_elevation,
        windows_firewall: capabilities.windows_firewall,
        launch_at_login: capabilities.launch_at_login,
        network_context: capabilities.network_context,
        macos_service_management: capabilities.macos_service_management,
        core_file_privileges: capabilities.core_file_privileges,
    }
}

pub struct InspectApplicationTask {
    path: String,
}

#[napi]
impl Task for InspectApplicationTask {
    type Output = kokorobox_native::ApplicationInfo;
    type JsValue = JsApplicationInfo;

    fn compute(&mut self) -> Result<Self::Output> {
        kokorobox_native::inspect_application(&self.path).map_err(map_err)
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(output.into())
    }
}

#[napi]
pub fn inspect_application(path: String) -> AsyncTask<InspectApplicationTask> {
    AsyncTask::new(InspectApplicationTask { path })
}

pub struct ScanWindowsApplicationsTask {
    directory: String,
    maximum_results: u32,
    excluded_executable_names: Vec<String>,
}

#[napi]
impl Task for ScanWindowsApplicationsTask {
    type Output = kokorobox_native::ApplicationScanResult;
    type JsValue = JsApplicationScanResult;

    fn compute(&mut self) -> Result<Self::Output> {
        kokorobox_native::scan_windows_applications(
            &self.directory,
            self.maximum_results,
            &self.excluded_executable_names,
        )
        .map_err(map_err)
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(JsApplicationScanResult {
            applications: output.applications.into_iter().map(Into::into).collect(),
            truncated: output.truncated,
            unreadable_directory_count: output.unreadable_directory_count,
        })
    }
}

#[napi]
pub fn scan_windows_applications(
    directory: String,
    maximum_results: Option<u32>,
    excluded_executable_names: Option<Vec<String>>,
) -> AsyncTask<ScanWindowsApplicationsTask> {
    AsyncTask::new(ScanWindowsApplicationsTask {
        directory,
        maximum_results: maximum_results.unwrap_or(512),
        excluded_executable_names: excluded_executable_names.unwrap_or_default(),
    })
}
