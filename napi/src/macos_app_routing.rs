use napi::bindgen_prelude::*;
use napi_derive::napi;

use crate::error::map_err;

#[napi(object)]
pub struct JsMacosApplicationRoutingRule {
    pub signing_identifier: String,
    pub identifier_kind: String,
    pub rule_protocol: String,
    pub action: String,
    pub enabled: bool,
    pub priority: u32,
}

#[napi(object)]
pub struct JsMacosApplicationRoutingConfiguration {
    pub proxy_available: bool,
    pub proxy_udp_dns: bool,
    pub diagnostic_logging: bool,
    pub rules: Vec<JsMacosApplicationRoutingRule>,
}

#[napi(object)]
pub struct JsMacosApplicationRoutingStatus {
    pub state: String,
    pub needs_user_approval: bool,
    pub message: Option<String>,
}

impl TryFrom<JsMacosApplicationRoutingRule> for kokorobox_native::MacosApplicationRoutingRule {
    type Error = Error;

    fn try_from(value: JsMacosApplicationRoutingRule) -> Result<Self> {
        let identifier_kind = match value.identifier_kind.as_str() {
            "SIGNING_IDENTIFIER" => {
                kokorobox_native::MacosApplicationRoutingIdentifierKind::SigningIdentifier
            }
            "PROCESS_NAME" => kokorobox_native::MacosApplicationRoutingIdentifierKind::ProcessName,
            _ => {
                return Err(Error::from_reason(
                    "Invalid macOS application identifier kind",
                ));
            }
        };
        let rule_protocol = match value.rule_protocol.as_str() {
            "TCP" => kokorobox_native::MacosApplicationRoutingProtocol::Tcp,
            "UDP" => kokorobox_native::MacosApplicationRoutingProtocol::Udp,
            "BOTH" => kokorobox_native::MacosApplicationRoutingProtocol::Both,
            _ => {
                return Err(Error::from_reason(
                    "Invalid macOS application-routing protocol",
                ));
            }
        };
        let action = match value.action.as_str() {
            "PROXY" => kokorobox_native::MacosApplicationRoutingAction::Proxy,
            "DIRECT" => kokorobox_native::MacosApplicationRoutingAction::Direct,
            "BLOCK" => kokorobox_native::MacosApplicationRoutingAction::Block,
            _ => {
                return Err(Error::from_reason(
                    "Invalid macOS application-routing action",
                ));
            }
        };
        Ok(Self {
            signing_identifier: value.signing_identifier,
            identifier_kind,
            rule_protocol,
            action,
            enabled: value.enabled,
            priority: value.priority,
        })
    }
}

impl TryFrom<JsMacosApplicationRoutingConfiguration>
    for kokorobox_native::MacosApplicationRoutingConfiguration
{
    type Error = Error;

    fn try_from(value: JsMacosApplicationRoutingConfiguration) -> Result<Self> {
        Ok(Self {
            proxy_available: value.proxy_available,
            proxy_udp_dns: value.proxy_udp_dns,
            diagnostic_logging: value.diagnostic_logging,
            rules: value
                .rules
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<Vec<_>>>()?,
        })
    }
}

impl From<kokorobox_native::MacosApplicationRoutingStatus> for JsMacosApplicationRoutingStatus {
    fn from(value: kokorobox_native::MacosApplicationRoutingStatus) -> Self {
        Self {
            state: value.state.as_str().to_string(),
            needs_user_approval: value.needs_user_approval,
            message: value.message,
        }
    }
}

enum MacosApplicationRoutingOperation {
    Apply(kokorobox_native::MacosApplicationRoutingConfiguration),
    Status,
    Stop,
    OpenSettings,
}

pub struct MacosApplicationRoutingTask {
    operation: Option<MacosApplicationRoutingOperation>,
}

#[napi]
impl Task for MacosApplicationRoutingTask {
    type Output = kokorobox_native::MacosApplicationRoutingStatus;
    type JsValue = JsMacosApplicationRoutingStatus;

    fn compute(&mut self) -> Result<Self::Output> {
        match self
            .operation
            .take()
            .ok_or_else(|| Error::from_reason("macOS application-routing operation was consumed"))?
        {
            MacosApplicationRoutingOperation::Apply(configuration) => {
                kokorobox_native::apply_macos_application_routing(&configuration).map_err(map_err)
            }
            MacosApplicationRoutingOperation::Status => {
                kokorobox_native::get_macos_application_routing_status().map_err(map_err)
            }
            MacosApplicationRoutingOperation::Stop => {
                kokorobox_native::stop_macos_application_routing().map_err(map_err)
            }
            MacosApplicationRoutingOperation::OpenSettings => {
                kokorobox_native::open_macos_application_routing_settings().map_err(map_err)
            }
        }
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(output.into())
    }
}

#[napi]
pub fn apply_macos_application_routing(
    configuration: JsMacosApplicationRoutingConfiguration,
) -> Result<AsyncTask<MacosApplicationRoutingTask>> {
    Ok(AsyncTask::new(MacosApplicationRoutingTask {
        operation: Some(MacosApplicationRoutingOperation::Apply(
            configuration.try_into()?,
        )),
    }))
}

#[napi]
pub fn get_macos_application_routing_status() -> AsyncTask<MacosApplicationRoutingTask> {
    AsyncTask::new(MacosApplicationRoutingTask {
        operation: Some(MacosApplicationRoutingOperation::Status),
    })
}

#[napi]
pub fn stop_macos_application_routing() -> AsyncTask<MacosApplicationRoutingTask> {
    AsyncTask::new(MacosApplicationRoutingTask {
        operation: Some(MacosApplicationRoutingOperation::Stop),
    })
}

#[napi]
pub fn open_macos_application_routing_settings() -> AsyncTask<MacosApplicationRoutingTask> {
    AsyncTask::new(MacosApplicationRoutingTask {
        operation: Some(MacosApplicationRoutingOperation::OpenSettings),
    })
}
