use napi::bindgen_prelude::*;
use napi_derive::napi;
use crate::error::map_err;
#[napi(object)]
pub struct JsCoreValidationOptions { pub executable: String, pub config_path: String, pub work_dir: String, pub safe_paths: Vec<String> }
#[napi(object)]
pub struct JsCoreValidationResult { pub outcome: String, pub output: String }
pub struct ValidationTask { options: kokorobox_native::CoreValidationOptions }
#[napi]
impl Task for ValidationTask {
 type Output = kokorobox_native::CoreValidationResult;
 type JsValue = JsCoreValidationResult;
 fn compute(&mut self) -> Result<Self::Output> { kokorobox_native::validate_core_profile(&self.options).map_err(map_err) }
 fn resolve(&mut self, _: Env, output: Self::Output) -> Result<Self::JsValue> { Ok(JsCoreValidationResult { outcome: output.outcome, output: output.output }) }
}
#[napi]
pub fn validate_core_profile(options: JsCoreValidationOptions) -> AsyncTask<ValidationTask> {
 AsyncTask::new(ValidationTask { options: kokorobox_native::CoreValidationOptions { executable: options.executable, config_path: options.config_path, work_dir: options.work_dir, safe_paths: options.safe_paths } })
}
