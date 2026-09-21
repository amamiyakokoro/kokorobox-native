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
    pub requires_approval: bool,
    pub backend: String,
}

#[napi(object)]
pub struct JsNetworkContext {
    pub online: bool,
    pub default_interface: Option<String>,
    pub default_service: Option<String>,
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

impl From<JsNetworkContext> for kokorobox_native::NetworkContext {
    fn from(value: JsNetworkContext) -> Self {
        Self {
            online: value.online,
            default_interface: value.default_interface,
            default_service: value.default_service,
            dns_servers: value.dns_servers,
            ssid: value.ssid,
        }
    }
}

impl From<kokorobox_native::LaunchAtLoginStatus> for JsLaunchAtLoginStatus {
    fn from(value: kokorobox_native::LaunchAtLoginStatus) -> Self {
        Self {
            enabled: value.enabled,
            requires_approval: value.requires_approval,
            backend: value.backend,
        }
    }
}

impl From<kokorobox_native::NetworkContext> for JsNetworkContext {
    fn from(value: kokorobox_native::NetworkContext) -> Self {
        Self {
            online: value.online,
            default_interface: value.default_interface,
            default_service: value.default_service,
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

pub struct SetActiveNetworkDnsTask {
    servers: Vec<String>,
}

#[napi]
impl Task for SetActiveNetworkDnsTask {
    type Output = ();
    type JsValue = ();

    fn compute(&mut self) -> Result<Self::Output> {
        kokorobox_native::set_active_network_dns(&self.servers).map_err(map_err)
    }

    fn resolve(&mut self, _env: Env, _output: Self::Output) -> Result<Self::JsValue> {
        Ok(())
    }
}

#[napi]
pub fn set_active_network_dns(servers: Vec<String>) -> AsyncTask<SetActiveNetworkDnsTask> {
    AsyncTask::new(SetActiveNetworkDnsTask { servers })
}

pub struct WaitForNetworkContextChangeTask {
    previous: kokorobox_native::NetworkContext,
    timeout_ms: u32,
}

#[napi]
impl Task for WaitForNetworkContextChangeTask {
    type Output = Option<kokorobox_native::NetworkContext>;
    type JsValue = Option<JsNetworkContext>;

    fn compute(&mut self) -> Result<Self::Output> {
        kokorobox_native::wait_for_network_context_change(
            &self.previous,
            std::time::Duration::from_millis(u64::from(self.timeout_ms)),
        )
        .map_err(map_err)
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(output.map(Into::into))
    }
}

#[napi]
pub fn wait_for_network_context_change(
    previous: JsNetworkContext,
    timeout_ms: Option<u32>,
) -> AsyncTask<WaitForNetworkContextChangeTask> {
    AsyncTask::new(WaitForNetworkContextChangeTask {
        previous: previous.into(),
        timeout_ms: timeout_ms.unwrap_or(30_000).clamp(500, 60_000),
    })
}
