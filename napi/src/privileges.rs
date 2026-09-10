use napi::bindgen_prelude::*;
use napi_derive::napi;

use crate::error::map_err;

#[napi(object)]
pub struct JsCorePrivilegeStatus {
    pub path: String,
    pub granted: bool,
}

impl From<kokorobox_native::CorePrivilegeStatus> for JsCorePrivilegeStatus {
    fn from(value: kokorobox_native::CorePrivilegeStatus) -> Self {
        Self {
            path: value.path,
            granted: value.granted,
        }
    }
}

#[napi]
pub fn get_core_privilege_status(paths: Vec<String>) -> Result<Vec<JsCorePrivilegeStatus>> {
    kokorobox_native::get_core_privilege_status(&paths)
        .map(|statuses| statuses.into_iter().map(Into::into).collect())
        .map_err(map_err)
}

pub struct SetCorePrivilegesTask {
    paths: Vec<String>,
    enabled: bool,
}

#[napi]
impl Task for SetCorePrivilegesTask {
    type Output = Vec<kokorobox_native::CorePrivilegeStatus>;
    type JsValue = Vec<JsCorePrivilegeStatus>;

    fn compute(&mut self) -> Result<Self::Output> {
        kokorobox_native::set_core_privileges(&self.paths, self.enabled).map_err(map_err)
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(output.into_iter().map(Into::into).collect())
    }
}

#[napi]
pub fn set_core_privileges(paths: Vec<String>, enabled: bool) -> AsyncTask<SetCorePrivilegesTask> {
    AsyncTask::new(SetCorePrivilegesTask { paths, enabled })
}
