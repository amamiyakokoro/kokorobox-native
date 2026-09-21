use napi::bindgen_prelude::*;
use napi_derive::napi;

use crate::error::map_err;

#[napi(object)]
pub struct JsServiceLifecycleOptions {
    pub executable_path: String,
    pub action: String,
    pub public_key: Option<String>,
    pub authorized_sid: Option<String>,
    pub authorized_uid: Option<u32>,
}

pub struct ServiceLifecycleTask {
    options: Option<JsServiceLifecycleOptions>,
}

#[napi]
impl Task for ServiceLifecycleTask {
    type Output = ();
    type JsValue = ();

    fn compute(&mut self) -> Result<Self::Output> {
        let options = self
            .options
            .take()
            .ok_or_else(|| Error::from_reason("service lifecycle options were already consumed"))?;
        let action =
            kokorobox_native::ServiceLifecycleAction::parse(&options.action).map_err(map_err)?;
        kokorobox_native::run_service_lifecycle(&kokorobox_native::ServiceLifecycleOptions {
            executable_path: options.executable_path,
            action,
            public_key: options.public_key,
            authorized_sid: options.authorized_sid,
            authorized_uid: options.authorized_uid,
        })
        .map_err(map_err)
    }

    fn resolve(&mut self, _env: Env, _output: Self::Output) -> Result<Self::JsValue> {
        Ok(())
    }
}

#[napi]
pub fn run_service_lifecycle_elevated(
    options: JsServiceLifecycleOptions,
) -> AsyncTask<ServiceLifecycleTask> {
    AsyncTask::new(ServiceLifecycleTask {
        options: Some(options),
    })
}

enum PrivilegedOperation {
    CleanupLegacyMacosService,
    StopMacosManagedService,
}

pub struct PrivilegedTask {
    operation: PrivilegedOperation,
}

#[napi]
impl Task for PrivilegedTask {
    type Output = ();
    type JsValue = ();

    fn compute(&mut self) -> Result<Self::Output> {
        match self.operation {
            PrivilegedOperation::CleanupLegacyMacosService => {
                kokorobox_native::cleanup_legacy_macos_service()
            }
            PrivilegedOperation::StopMacosManagedService => {
                kokorobox_native::stop_macos_managed_service()
            }
        }
        .map_err(map_err)
    }

    fn resolve(&mut self, _env: Env, _output: Self::Output) -> Result<Self::JsValue> {
        Ok(())
    }
}

#[napi]
pub fn cleanup_legacy_macos_service() -> AsyncTask<PrivilegedTask> {
    AsyncTask::new(PrivilegedTask {
        operation: PrivilegedOperation::CleanupLegacyMacosService,
    })
}

#[napi]
pub fn stop_macos_managed_service() -> AsyncTask<PrivilegedTask> {
    AsyncTask::new(PrivilegedTask {
        operation: PrivilegedOperation::StopMacosManagedService,
    })
}

pub struct RepairManagedFilePermissionsTask {
    target: String,
    managed_root: String,
    uid: u32,
    gid: u32,
}

#[napi]
impl Task for RepairManagedFilePermissionsTask {
    type Output = ();
    type JsValue = ();

    fn compute(&mut self) -> Result<Self::Output> {
        kokorobox_native::repair_managed_file_permissions(
            &self.target,
            &self.managed_root,
            self.uid,
            self.gid,
        )
        .map_err(map_err)
    }

    fn resolve(&mut self, _env: Env, _output: Self::Output) -> Result<Self::JsValue> {
        Ok(())
    }
}

#[napi]
pub fn repair_managed_file_permissions(
    target: String,
    managed_root: String,
    uid: u32,
    gid: u32,
) -> AsyncTask<RepairManagedFilePermissionsTask> {
    AsyncTask::new(RepairManagedFilePermissionsTask {
        target,
        managed_root,
        uid,
        gid,
    })
}

#[napi]
pub fn relaunch_current_application_with_privilege(
    arguments: Vec<String>,
    elevated: bool,
) -> Result<()> {
    kokorobox_native::relaunch_current_application_with_privilege(&arguments, elevated)
        .map_err(map_err)
}
