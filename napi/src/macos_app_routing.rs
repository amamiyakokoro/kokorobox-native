use napi::bindgen_prelude::*;
use napi_derive::napi;

use crate::error::map_err;

pub struct InvokeMacOSApplicationRoutingTask {
    request: String,
}

#[napi]
impl Task for InvokeMacOSApplicationRoutingTask {
    type Output = String;
    type JsValue = String;

    fn compute(&mut self) -> Result<Self::Output> {
        kokorobox_native::invoke_macos_application_routing(&self.request).map_err(map_err)
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(output)
    }
}

#[napi]
pub fn invoke_macos_application_routing(
    request: String,
) -> AsyncTask<InvokeMacOSApplicationRoutingTask> {
    AsyncTask::new(InvokeMacOSApplicationRoutingTask { request })
}
