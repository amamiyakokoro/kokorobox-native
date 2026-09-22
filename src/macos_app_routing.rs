use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};

const PROTOCOL_VERSION: u8 = 1;
const MAXIMUM_REQUEST_BYTES: usize = 256 * 1024;
const MAXIMUM_RULES: usize = 256;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MacosApplicationRoutingIdentifierKind {
    SigningIdentifier,
    ProcessName,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MacosApplicationRoutingProtocol {
    Tcp,
    Udp,
    Both,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MacosApplicationRoutingAction {
    Proxy,
    Direct,
    Block,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MacosApplicationRoutingRule {
    pub signing_identifier: String,
    pub identifier_kind: MacosApplicationRoutingIdentifierKind,
    pub rule_protocol: MacosApplicationRoutingProtocol,
    pub action: MacosApplicationRoutingAction,
    pub enabled: bool,
    pub priority: u32,
}

#[derive(Debug, Clone)]
pub struct MacosApplicationRoutingConfiguration {
    pub proxy_available: bool,
    pub proxy_udp_dns: bool,
    pub diagnostic_logging: bool,
    pub rules: Vec<MacosApplicationRoutingRule>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacosApplicationRoutingState {
    Disabled,
    Starting,
    Running,
    Stopping,
    Error,
}

impl MacosApplicationRoutingState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Starting => "starting",
            Self::Running => "running",
            Self::Stopping => "stopping",
            Self::Error => "error",
        }
    }
}

impl TryFrom<&str> for MacosApplicationRoutingState {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self> {
        match value {
            "disabled" => Ok(Self::Disabled),
            "starting" => Ok(Self::Starting),
            "running" => Ok(Self::Running),
            "stopping" => Ok(Self::Stopping),
            "error" => Ok(Self::Error),
            _ => Err(anyhow!("Unsupported macOS application-routing state")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacosApplicationRoutingStatus {
    pub state: MacosApplicationRoutingState,
    pub needs_user_approval: bool,
    pub message: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WireConfiguration<'a> {
    version: u8,
    fail_closed: bool,
    proxy_available: bool,
    proxy_host: &'static str,
    proxy_port: u16,
    proxy_udp_dns: bool,
    dns_host: &'static str,
    dns_port: u16,
    diagnostic_logging: bool,
    rules: &'a [MacosApplicationRoutingRule],
}

#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
enum WireCommand {
    Apply,
    Stop,
    Status,
    OpenSettings,
}

#[derive(Serialize)]
struct WireRequest<'a> {
    version: u8,
    command: WireCommand,
    #[serde(skip_serializing_if = "Option::is_none")]
    configuration: Option<WireConfiguration<'a>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireResponse {
    version: u8,
    ok: bool,
    state: String,
    needs_user_approval: bool,
    message: Option<String>,
}

fn validate_configuration(configuration: &MacosApplicationRoutingConfiguration) -> Result<()> {
    if configuration.rules.len() > MAXIMUM_RULES {
        return Err(anyhow!(
            "macOS application routing supports at most {MAXIMUM_RULES} rules"
        ));
    }
    for rule in &configuration.rules {
        if rule.signing_identifier.trim().is_empty()
            || rule.signing_identifier.as_bytes().contains(&0)
            || rule.priority == 0
        {
            return Err(anyhow!("Invalid macOS application-routing rule"));
        }
    }
    Ok(())
}

fn request_json(
    command: WireCommand,
    configuration: Option<&MacosApplicationRoutingConfiguration>,
) -> Result<String> {
    if let Some(configuration) = configuration {
        validate_configuration(configuration)?;
    }
    let configuration = configuration.map(|configuration| WireConfiguration {
        version: PROTOCOL_VERSION,
        fail_closed: true,
        proxy_available: configuration.proxy_available,
        proxy_host: "127.0.0.1",
        proxy_port: 7891,
        proxy_udp_dns: configuration.proxy_udp_dns,
        dns_host: "127.0.0.1",
        dns_port: 7892,
        diagnostic_logging: configuration.diagnostic_logging,
        rules: &configuration.rules,
    });
    let request = serde_json::to_string(&WireRequest {
        version: PROTOCOL_VERSION,
        command,
        configuration,
    })?;
    validate_request(&request)?;
    Ok(request)
}

fn parse_response(response: &str) -> Result<MacosApplicationRoutingStatus> {
    let response: WireResponse = serde_json::from_str(response)
        .map_err(|_| anyhow!("Unsupported macOS application-routing module response"))?;
    if response.version != PROTOCOL_VERSION {
        return Err(anyhow!(
            "Unsupported macOS application-routing module response"
        ));
    }
    if !response.ok {
        return Err(anyhow!(
            "{}",
            response
                .message
                .unwrap_or_else(|| "macOS application routing failed".to_string())
        ));
    }
    Ok(MacosApplicationRoutingStatus {
        state: response.state.as_str().try_into()?,
        needs_user_approval: response.needs_user_approval,
        message: response.message,
    })
}

fn invoke_typed(
    command: WireCommand,
    configuration: Option<&MacosApplicationRoutingConfiguration>,
) -> Result<MacosApplicationRoutingStatus> {
    let request = request_json(command, configuration)?;
    let response = invoke_raw(&request)?;
    parse_response(&response)
}

pub fn apply_macos_application_routing(
    configuration: &MacosApplicationRoutingConfiguration,
) -> Result<MacosApplicationRoutingStatus> {
    invoke_typed(WireCommand::Apply, Some(configuration))
}

pub fn get_macos_application_routing_status() -> Result<MacosApplicationRoutingStatus> {
    invoke_typed(WireCommand::Status, None)
}

pub fn stop_macos_application_routing() -> Result<MacosApplicationRoutingStatus> {
    invoke_typed(WireCommand::Stop, None)
}

pub fn open_macos_application_routing_settings() -> Result<MacosApplicationRoutingStatus> {
    invoke_typed(WireCommand::OpenSettings, None)
}

fn validate_request(request: &str) -> Result<()> {
    if request.is_empty()
        || request.len() > MAXIMUM_REQUEST_BYTES
        || request.as_bytes().contains(&0)
    {
        return Err(anyhow!("macOS application-routing request size is invalid"));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn invoke_raw(request: &str) -> Result<String> {
    use std::ffi::{CStr, CString, c_char};
    use std::ptr;

    unsafe extern "C" {
        fn kokorobox_macos_app_routing_invoke(
            request_json: *const c_char,
            response_json: *mut *mut c_char,
            error_message: *mut *mut c_char,
        ) -> bool;
        fn kokorobox_macos_app_routing_free(value: *mut c_char);
    }

    validate_request(request)?;
    let request = CString::new(request)?;
    let mut response = ptr::null_mut();
    let mut error = ptr::null_mut();
    // SAFETY: The Objective-C bridge copies both output strings and exposes a
    // matching deallocator. The input CString remains alive for the call.
    let ok =
        unsafe { kokorobox_macos_app_routing_invoke(request.as_ptr(), &mut response, &mut error) };
    let read = |value: *mut c_char| -> Option<String> {
        if value.is_null() {
            return None;
        }
        // SAFETY: The bridge returns a NUL-terminated allocation on success.
        let result = unsafe { CStr::from_ptr(value) }
            .to_string_lossy()
            .into_owned();
        // SAFETY: This pointer was allocated by the bridge and is released once.
        unsafe { kokorobox_macos_app_routing_free(value) };
        Some(result)
    };
    let response = read(response);
    let error = read(error);
    if !ok {
        return Err(anyhow!(
            "{}",
            error.unwrap_or_else(|| "macOS application routing failed".to_string())
        ));
    }
    response.ok_or_else(|| anyhow!("macOS application routing returned no response"))
}

#[cfg(not(target_os = "macos"))]
fn invoke_raw(request: &str) -> Result<String> {
    validate_request(request)?;
    Err(anyhow!(
        "UNSUPPORTED_PLATFORM: macOS application routing requires macOS"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn configuration() -> MacosApplicationRoutingConfiguration {
        MacosApplicationRoutingConfiguration {
            proxy_available: true,
            proxy_udp_dns: true,
            diagnostic_logging: false,
            rules: vec![MacosApplicationRoutingRule {
                signing_identifier: "com.example.app".to_string(),
                identifier_kind: MacosApplicationRoutingIdentifierKind::SigningIdentifier,
                rule_protocol: MacosApplicationRoutingProtocol::Both,
                action: MacosApplicationRoutingAction::Proxy,
                enabled: true,
                priority: 1,
            }],
        }
    }

    #[test]
    fn typed_apply_owns_the_versioned_wire_envelope() {
        let request: Value = serde_json::from_str(
            &request_json(WireCommand::Apply, Some(&configuration())).unwrap(),
        )
        .unwrap();
        assert_eq!(request["version"], 1);
        assert_eq!(request["command"], "apply");
        assert_eq!(request["configuration"]["failClosed"], true);
        assert_eq!(request["configuration"]["proxyHost"], "127.0.0.1");
        assert_eq!(request["configuration"]["proxyPort"], 7891);
        assert_eq!(request["configuration"]["dnsHost"], "127.0.0.1");
        assert_eq!(request["configuration"]["dnsPort"], 7892);
        assert_eq!(
            request["configuration"]["rules"][0]["identifierKind"],
            "SIGNING_IDENTIFIER"
        );
    }

    #[test]
    fn validates_typed_configuration_and_response() {
        let mut invalid = configuration();
        invalid.rules[0].priority = 0;
        assert!(request_json(WireCommand::Apply, Some(&invalid)).is_err());

        let status = parse_response(
            r#"{"version":1,"ok":true,"state":"running","needsUserApproval":false}"#,
        )
        .unwrap();
        assert_eq!(status.state, MacosApplicationRoutingState::Running);
        assert!(!status.needs_user_approval);
        assert!(
            parse_response(
                r#"{"version":2,"ok":true,"state":"running","needsUserApproval":false}"#
            )
            .is_err()
        );
        assert!(
            parse_response(
                r#"{"version":1,"ok":true,"state":"unknown","needsUserApproval":false}"#
            )
            .is_err()
        );
    }

    #[test]
    fn rejects_empty_oversized_and_nul_requests() {
        assert!(validate_request("").is_err());
        assert!(validate_request(&"x".repeat(MAXIMUM_REQUEST_BYTES + 1)).is_err());
        assert!(validate_request("bad\0request").is_err());
        assert!(validate_request(r#"{"version":1,"command":"status"}"#).is_ok());
    }
}
