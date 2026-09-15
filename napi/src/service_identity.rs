use napi::bindgen_prelude::*;
use napi_derive::napi;

use crate::error::map_err;

#[napi(object)]
pub struct JsServiceIdentityOptions {
    pub service: String,
    pub account: String,
    pub linux_fallback_path: Option<String>,
}

#[napi(object)]
pub struct JsLegacyServiceIdentity {
    pub key_id: Option<String>,
    pub public_key: String,
    pub private_key: String,
}

#[napi(object)]
pub struct JsServiceIdentityInfo {
    pub key_id: String,
    pub public_key: String,
    pub backend: String,
}

impl From<JsServiceIdentityOptions> for kokorobox_native::ServiceIdentityOptions {
    fn from(value: JsServiceIdentityOptions) -> Self {
        Self {
            service: value.service,
            account: value.account,
            linux_fallback_path: value.linux_fallback_path,
        }
    }
}

impl From<JsLegacyServiceIdentity> for kokorobox_native::LegacyServiceIdentity {
    fn from(value: JsLegacyServiceIdentity) -> Self {
        Self {
            key_id: value.key_id,
            public_key: value.public_key,
            private_key: value.private_key,
        }
    }
}

impl From<kokorobox_native::ServiceIdentityInfo> for JsServiceIdentityInfo {
    fn from(value: kokorobox_native::ServiceIdentityInfo) -> Self {
        Self {
            key_id: value.key_id,
            public_key: value.public_key,
            backend: value.backend,
        }
    }
}

#[napi(js_name = "ServiceIdentity")]
pub struct JsServiceIdentity {
    inner: kokorobox_native::ServiceIdentity,
}

#[napi]
impl JsServiceIdentity {
    #[napi]
    pub fn get_info(&self) -> JsServiceIdentityInfo {
        self.inner.info().into()
    }

    #[napi]
    pub fn sign(&self, data: String) -> String {
        self.inner.sign(data.as_bytes())
    }
}

pub struct OpenServiceIdentityTask {
    options: kokorobox_native::ServiceIdentityOptions,
    legacy: Option<kokorobox_native::LegacyServiceIdentity>,
}

#[napi]
impl Task for OpenServiceIdentityTask {
    type Output = kokorobox_native::ServiceIdentity;
    type JsValue = JsServiceIdentity;

    fn compute(&mut self) -> Result<Self::Output> {
        kokorobox_native::open_service_identity(&self.options, self.legacy.take()).map_err(map_err)
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(JsServiceIdentity { inner: output })
    }
}

#[napi]
pub fn open_service_identity(
    options: JsServiceIdentityOptions,
    legacy: Option<JsLegacyServiceIdentity>,
) -> AsyncTask<OpenServiceIdentityTask> {
    AsyncTask::new(OpenServiceIdentityTask {
        options: options.into(),
        legacy: legacy.map(Into::into),
    })
}

pub struct DeleteServiceIdentityTask {
    options: kokorobox_native::ServiceIdentityOptions,
}

#[napi]
impl Task for DeleteServiceIdentityTask {
    type Output = ();
    type JsValue = ();

    fn compute(&mut self) -> Result<Self::Output> {
        kokorobox_native::delete_service_identity(&self.options).map_err(map_err)
    }

    fn resolve(&mut self, _env: Env, _output: Self::Output) -> Result<Self::JsValue> {
        Ok(())
    }
}

#[napi]
pub fn delete_service_identity(
    options: JsServiceIdentityOptions,
) -> AsyncTask<DeleteServiceIdentityTask> {
    AsyncTask::new(DeleteServiceIdentityTask {
        options: options.into(),
    })
}
