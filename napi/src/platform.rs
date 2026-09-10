use napi::bindgen_prelude::*;
use napi_derive::napi;

use crate::error::map_err;

#[napi(object)]
pub struct JsLaunchAtLoginOptions {
    pub identifier: String,
    pub display_name: String,
    pub executable_path: String,
    pub arguments: Option<Vec<String>>,
}

#[napi(object)]
pub struct JsLaunchAtLoginStatus {
    pub enabled: bool,
    pub backend: String,
}

#[napi(object)]
pub struct JsNetworkContext {
    pub default_interface: Option<String>,
    pub dns_servers: Vec<String>,
    pub ssid: Option<String>,
}

impl From<JsLaunchAtLoginOptions> for kokorobox_native::LaunchAtLoginOptions {
    fn from(value: JsLaunchAtLoginOptions) -> Self {
        Self {
            identifier: value.identifier,
            display_name: value.display_name,
            executable_path: value.executable_path,
            arguments: value.arguments.unwrap_or_default(),
        }
    }
}

impl From<kokorobox_native::LaunchAtLoginStatus> for JsLaunchAtLoginStatus {
    fn from(value: kokorobox_native::LaunchAtLoginStatus) -> Self {
        Self {
            enabled: value.enabled,
            backend: value.backend,
        }
    }
}

impl From<kokorobox_native::NetworkContext> for JsNetworkContext {
    fn from(value: kokorobox_native::NetworkContext) -> Self {
        Self {
            default_interface: value.default_interface,
            dns_servers: value.dns_servers,
            ssid: value.ssid,
        }
    }
}

pub struct GetLaunchAtLoginTask {
    options: kokorobox_native::LaunchAtLoginOptions,
}

#[napi]
impl Task for GetLaunchAtLoginTask {
    type Output = kokorobox_native::LaunchAtLoginStatus;
    type JsValue = JsLaunchAtLoginStatus;

    fn compute(&mut self) -> Result<Self::Output> {
        kokorobox_native::get_launch_at_login(&self.options).map_err(map_err)
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(output.into())
    }
}

#[napi]
pub fn get_launch_at_login(options: JsLaunchAtLoginOptions) -> AsyncTask<GetLaunchAtLoginTask> {
    AsyncTask::new(GetLaunchAtLoginTask {
        options: options.into(),
    })
}

pub struct SetLaunchAtLoginTask {
    options: kokorobox_native::LaunchAtLoginOptions,
    enabled: bool,
}

#[napi]
impl Task for SetLaunchAtLoginTask {
    type Output = kokorobox_native::LaunchAtLoginStatus;
    type JsValue = JsLaunchAtLoginStatus;

    fn compute(&mut self) -> Result<Self::Output> {
        kokorobox_native::set_launch_at_login(&self.options, self.enabled).map_err(map_err)
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(output.into())
    }
}

#[napi]
pub fn set_launch_at_login(
    options: JsLaunchAtLoginOptions,
    enabled: bool,
) -> AsyncTask<SetLaunchAtLoginTask> {
    AsyncTask::new(SetLaunchAtLoginTask {
        options: options.into(),
        enabled,
    })
}

pub struct GetNetworkContextTask;

#[napi]
impl Task for GetNetworkContextTask {
    type Output = kokorobox_native::NetworkContext;
    type JsValue = JsNetworkContext;

    fn compute(&mut self) -> Result<Self::Output> {
        kokorobox_native::get_network_context().map_err(map_err)
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(output.into())
    }
}

#[napi]
pub fn get_network_context() -> AsyncTask<GetNetworkContextTask> {
    AsyncTask::new(GetNetworkContextTask)
}
