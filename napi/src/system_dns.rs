use crate::error::map_err;
use napi::bindgen_prelude::*;
use napi_derive::napi;

#[napi(object)]
pub struct JsDNSQuery {
    pub domain: String,
    pub outcome: String,
}
#[napi(object)]
pub struct JsSystemDNSDiagnostics {
    pub outcome: String,
    pub queries: Vec<JsDNSQuery>,
    pub interface: Option<String>,
    pub service: Option<String>,
    pub servers: Vec<String>,
}
pub struct SystemDNSDiagnosticsTask;
#[napi]
impl Task for SystemDNSDiagnosticsTask {
    type Output = kokorobox_native::SystemDNSDiagnostics;
    type JsValue = JsSystemDNSDiagnostics;
    fn compute(&mut self) -> Result<Self::Output> {
        kokorobox_native::get_system_dns_diagnostics().map_err(map_err)
    }
    fn resolve(&mut self, _: Env, v: Self::Output) -> Result<Self::JsValue> {
        Ok(JsSystemDNSDiagnostics {
            outcome: v.outcome,
            queries: v
                .queries
                .into_iter()
                .map(|q| JsDNSQuery {
                    domain: q.domain,
                    outcome: q.outcome,
                })
                .collect(),
            interface: v.interface,
            service: v.service,
            servers: v.servers,
        })
    }
}
#[napi]
pub fn get_system_dns_diagnostics() -> AsyncTask<SystemDNSDiagnosticsTask> {
    AsyncTask::new(SystemDNSDiagnosticsTask)
}
