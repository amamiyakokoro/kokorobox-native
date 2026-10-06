use napi::bindgen_prelude::*;
use napi_derive::napi;
use crate::error::map_err;

#[napi(object)]
pub struct JsCoreProcessIdentity { pub pid: u32, pub executable: String, pub started: String }
impl From<kokorobox_native::CoreProcessIdentity> for JsCoreProcessIdentity {
    fn from(v: kokorobox_native::CoreProcessIdentity) -> Self { Self { pid: v.pid, executable: v.executable, started: v.started } }
}
pub struct InspectTask { pid: u32, executable: String }
#[napi]
impl Task for InspectTask {
    type Output = Option<kokorobox_native::CoreProcessIdentity>;
    type JsValue = Option<JsCoreProcessIdentity>;
    fn compute(&mut self) -> Result<Self::Output> { kokorobox_native::inspect_core_process(self.pid, &self.executable).map_err(map_err) }
    fn resolve(&mut self, _: Env, output: Self::Output) -> Result<Self::JsValue> { Ok(output.map(Into::into)) }
}
#[napi]
pub fn inspect_core_process(pid: u32, executable: String) -> AsyncTask<InspectTask> { AsyncTask::new(InspectTask { pid, executable }) }
pub struct StopTask { identity: kokorobox_native::CoreProcessIdentity }
#[napi]
impl Task for StopTask {
    type Output = bool;
    type JsValue = bool;
    fn compute(&mut self) -> Result<bool> { kokorobox_native::stop_core_process(&self.identity).map_err(map_err) }
    fn resolve(&mut self, _: Env, output: bool) -> Result<bool> { Ok(output) }
}
#[napi]
pub fn stop_core_process(identity: JsCoreProcessIdentity) -> AsyncTask<StopTask> {
    AsyncTask::new(StopTask { identity: kokorobox_native::CoreProcessIdentity { pid: identity.pid, executable: identity.executable, started: identity.started } })
}
