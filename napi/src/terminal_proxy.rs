use napi::bindgen_prelude::*;
use napi_derive::napi;

use crate::error::map_err;

pub struct SetTerminalProxyEnvironmentTask {
    host: String,
    port: u16,
    bypass: Vec<String>,
}

#[napi]
impl Task for SetTerminalProxyEnvironmentTask {
    type Output = bool;
    type JsValue = bool;

    fn compute(&mut self) -> Result<Self::Output> {
        kokorobox_native::set_terminal_proxy_environment(&self.host, self.port, &self.bypass)
            .map_err(map_err)
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(output)
    }
}

#[napi]
pub fn set_terminal_proxy_environment(
    host: String,
    port: u16,
    bypass: Vec<String>,
) -> AsyncTask<SetTerminalProxyEnvironmentTask> {
    AsyncTask::new(SetTerminalProxyEnvironmentTask { host, port, bypass })
}

pub struct ClearTerminalProxyEnvironmentTask;

#[napi]
impl Task for ClearTerminalProxyEnvironmentTask {
    type Output = Option<bool>;
    type JsValue = Option<bool>;

    fn compute(&mut self) -> Result<Self::Output> {
        kokorobox_native::clear_terminal_proxy_environment().map_err(map_err)
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(output)
    }
}

#[napi]
pub fn clear_terminal_proxy_environment() -> AsyncTask<ClearTerminalProxyEnvironmentTask> {
    AsyncTask::new(ClearTerminalProxyEnvironmentTask)
}
