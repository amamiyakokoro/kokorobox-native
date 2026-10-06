use napi::bindgen_prelude::*;
use napi_derive::napi;
use crate::error::map_err;
pub struct ServiceStatusTask { executable: String }
#[napi]
impl Task for ServiceStatusTask {
 type Output = String;
 type JsValue = String;
 fn compute(&mut self) -> Result<String> { kokorobox_native::get_service_process_status(&self.executable).map_err(map_err) }
 fn resolve(&mut self, _: Env, output: String) -> Result<String> { Ok(output) }
}
#[napi]
pub fn get_service_process_status(executable: String) -> AsyncTask<ServiceStatusTask> { AsyncTask::new(ServiceStatusTask { executable }) }
