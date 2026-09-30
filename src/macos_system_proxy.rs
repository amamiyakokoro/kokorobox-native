//! macOS configuration only. Runtime/core/socket/HTTP probes belong to Service.
use crate::system_proxy::{
    MacNetworkService, MacOSProxyDetails, MacProxyProtocol, MacProxyState, ProxyEndpoint,
    SystemProxyDiagnostics, SystemProxyMutation, SystemProxySettings,
};
use anyhow::{Result, bail};
use std::collections::BTreeMap;
use system_configuration::{
    core_foundation::{
        array::CFArray,
        base::{CFType, TCFType, ToVoid},
        dictionary::CFDictionary,
        number::CFNumber,
        propertylist::CFPropertyList,
        string::CFString,
    },
    dynamic_store::{SCDynamicStore, SCDynamicStoreBuilder},
    network_configuration::SCNetworkService,
    preferences::SCPreferences,
    sys::{network_configuration::*, preferences::*},
};

#[link(name = "SystemConfiguration", kind = "framework")]
unsafe extern "C" {
    fn SCError() -> i32;
}
fn error_code() -> &'static str {
    match unsafe { SCError() } {
        1003 => "permission-denied",
        2001 | 2002 => "service-unavailable",
        1006 => "configuration-busy",
        _ => "operation-failed",
    }
}
fn preferences() -> Option<SCPreferences> {
    let name = CFString::new("KokoroBox System Proxy");
    let ptr = unsafe {
        SCPreferencesCreate(
            std::ptr::null(),
            name.as_concrete_TypeRef(),
            std::ptr::null(),
        )
    };
    (!ptr.is_null()).then(|| unsafe { SCPreferences::wrap_under_create_rule(ptr) })
}
fn value(dictionary: &CFDictionary, key: &str) -> Option<CFType> {
    dictionary
        .find(CFString::new(key).to_void())
        .map(|ptr| unsafe { CFType::wrap_under_get_rule(*ptr) })
}
fn text(dictionary: &CFDictionary, key: &str) -> Option<String> {
    value(dictionary, key)?
        .downcast_into::<CFString>()
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
}
fn number(dictionary: &CFDictionary, key: &str) -> Option<i64> {
    value(dictionary, key)?
        .downcast_into::<CFNumber>()?
        .to_i64()
}
fn strings(dictionary: &CFDictionary, key: &str) -> Vec<String> {
    value(dictionary, key)
        .and_then(CFType::downcast_into::<CFArray>)
        .map(|a| {
            a.iter()
                .filter_map(|p| {
                    unsafe { CFType::wrap_under_get_rule(*p) }
                        .downcast_into::<CFString>()
                        .map(|s| s.to_string())
                })
                .collect()
        })
        .unwrap_or_default()
}
fn dictionary(store: &SCDynamicStore, key: &str) -> Option<CFDictionary> {
    store
        .get(key)
        .and_then(CFPropertyList::downcast_into::<CFDictionary>)
}
fn protocol(dictionary: &CFDictionary, prefix: &str) -> MacProxyProtocol {
    let enabled = number(dictionary, &format!("{prefix}Enable")).unwrap_or(0) != 0;
    let endpoint = text(dictionary, &format!("{prefix}Proxy"))
        .zip(number(dictionary, &format!("{prefix}Port")))
        .and_then(|(host, port)| {
            u16::try_from(port)
                .ok()
                .filter(|p| *p != 0)
                .map(|port| ProxyEndpoint { host, port })
        });
    MacProxyProtocol { enabled, endpoint }
}
fn proxy_state(dictionary: &CFDictionary) -> MacProxyState {
    MacProxyState {
        http: protocol(dictionary, "HTTP"),
        https: protocol(dictionary, "HTTPS"),
        socks: protocol(dictionary, "SOCKS"),
        pac_enabled: number(dictionary, "ProxyAutoConfigEnable").unwrap_or(0) != 0,
        pac_url: text(dictionary, "ProxyAutoConfigURLString"),
        auto_discovery: number(dictionary, "ProxyAutoDiscoveryEnable").unwrap_or(0) != 0,
        bypass: strings(dictionary, "ExceptionsList"),
        exclude_simple_hostnames: number(dictionary, "ExcludeSimpleHostnames").unwrap_or(0) != 0,
    }
}
fn enabled(state: &MacProxyState) -> bool {
    state.http.enabled
        || state.https.enabled
        || state.socks.enabled
        || state.pac_enabled
        || state.auto_discovery
}
fn current_services(prefs: &SCPreferences) -> (Vec<SCNetworkService>, Option<String>) {
    let set = unsafe { SCNetworkSetCopyCurrent(prefs.as_concrete_TypeRef()) };
    if set.is_null() {
        return (vec![], None);
    }
    // Hold the copied set for all borrowed accesses.
    let _owned = unsafe { CFType::wrap_under_create_rule(set.cast()) };
    let name = unsafe { SCNetworkSetGetName(set) };
    let location =
        (!name.is_null()).then(|| unsafe { CFString::wrap_under_get_rule(name) }.to_string());
    let ptr = unsafe { SCNetworkSetCopyServices(set) };
    let services = if ptr.is_null() {
        vec![]
    } else {
        unsafe { CFArray::<SCNetworkService>::wrap_under_create_rule(ptr) }
            .iter()
            .map(|s| (*s).clone())
            .collect()
    };
    (services, location)
}
fn primary_services(store: &SCDynamicStore) -> Vec<String> {
    let mut ids = vec![];
    for key in ["State:/Network/Global/IPv4", "State:/Network/Global/IPv6"] {
        if let Some(id) = dictionary(store, key).and_then(|d| text(&d, "PrimaryService"))
            && !ids.contains(&id)
        {
            ids.push(id);
        }
    }
    ids
}
fn stored_service_proxies(service: &SCNetworkService) -> Option<CFDictionary> {
    let kind = CFString::new("Proxies");
    let ptr = unsafe {
        SCNetworkServiceCopyProtocol(service.as_concrete_TypeRef(), kind.as_concrete_TypeRef())
    };
    if ptr.is_null() {
        return None;
    }
    let _owned = unsafe { CFType::wrap_under_create_rule(ptr.cast()) };
    let config = unsafe { SCNetworkProtocolGetConfiguration(ptr) };
    Some(if config.is_null() {
        CFDictionary::from_CFType_pairs(&[] as &[(CFString, CFType)]).to_untyped()
    } else {
        unsafe { CFDictionary::wrap_under_get_rule(config) }
    })
}
fn service_state(
    store: &SCDynamicStore,
    service: &SCNetworkService,
    primary: &[String],
) -> Option<MacNetworkService> {
    let id = service.id()?.to_string();
    let name_ptr = unsafe { SCNetworkServiceGetName(service.as_concrete_TypeRef()) };
    let name = if name_ptr.is_null() {
        "Unknown".into()
    } else {
        unsafe { CFString::wrap_under_get_rule(name_ptr) }.to_string()
    };
    let interface = service
        .network_interface()
        .and_then(|i| i.bsd_name())
        .map(|s| s.to_string());
    let primary = primary.contains(&id);
    let active = primary
        || ["IPv4", "IPv6"].iter().any(|family| {
            dictionary(store, &format!("State:/Network/Service/{id}/{family}"))
                .is_some_and(|d| !strings(&d, "Addresses").is_empty())
        });
    let configuration = dictionary(store, &format!("State:/Network/Service/{id}/Proxies"))
        .or_else(|| stored_service_proxies(service));
    Some(MacNetworkService {
        id,
        name,
        interface,
        active,
        primary,
        // The high-level crate currently inverts this Boolean; use Apple's API.
        enabled: unsafe { SCNetworkServiceGetEnabled(service.as_concrete_TypeRef()) } != 0,
        status: if configuration.is_some() {
            "available"
        } else {
            "unavailable"
        }
        .into(),
        proxies: configuration.as_ref().map(proxy_state),
    })
}
pub fn get_diagnostics() -> SystemProxyDiagnostics {
    let mut result = SystemProxyDiagnostics {
        platform: "darwin".into(),
        status: "unavailable".into(),
        error_code: Some("configuration-unavailable".into()),
        ..Default::default()
    };
    let mut mac = MacOSProxyDetails::default();
    if let Some(store) = SCDynamicStoreBuilder::new("KokoroBox proxy diagnostics").build() {
        mac.active_service_ids = primary_services(&store);
        if let Some(prefs) = preferences() {
            let (services, location) = current_services(&prefs);
            mac.network_location = location;
            mac.services = services
                .iter()
                .filter_map(|s| service_state(&store, s, &mac.active_service_ids))
                .collect();
        }
        if let Some(proxies) = store.get_proxies() {
            let proxies = proxies.to_untyped();
            let state = proxy_state(&proxies);
            result.enabled = Some(enabled(&state));
            result.http = state
                .http
                .enabled
                .then(|| state.http.endpoint.clone())
                .flatten();
            result.https = state
                .https
                .enabled
                .then(|| state.https.endpoint.clone())
                .flatten();
            result.socks = state
                .socks
                .enabled
                .then(|| state.socks.endpoint.clone())
                .flatten();
            result.pac_enabled = Some(state.pac_enabled);
            result.pac_url = state.pac_url.clone();
            result.bypass = state.bypass.clone();
            mac.effective = Some(state);
            result.status = "available".into();
            result.error_code = None;
        }
    } else {
        result.error_code = Some("service-unavailable".into());
    }
    if mac.network_location.is_none() {
        mac.location_error_code = Some("configuration-unavailable".into());
    }
    if mac.active_service_ids.is_empty() {
        mac.service_error_code = Some("network-service-not-found".into());
    }
    result.macos = Some(mac);
    result
}

fn updated_configuration(
    original: &CFDictionary,
    settings: &SystemProxySettings,
) -> CFDictionary<CFString, CFType> {
    // Preserve all unrelated keys, including PAC/WPAD in manual mode. Private
    // dictionary contents never cross IPC or enter logs.
    let (keys, values) = original.get_keys_and_values();
    let mut entries: BTreeMap<String, CFType> = keys
        .into_iter()
        .zip(values)
        .filter_map(|(k, v)| {
            let key = unsafe { CFType::wrap_under_get_rule(k) }
                .downcast_into::<CFString>()?
                .to_string();
            Some((key, unsafe { CFType::wrap_under_get_rule(v) }))
        })
        .collect();
    for prefix in ["HTTP", "HTTPS", "SOCKS"] {
        entries.insert(
            format!("{prefix}Enable"),
            CFNumber::from(i32::from(settings.mode == "manual")).as_CFType(),
        );
        if settings.mode == "manual" {
            entries.insert(
                format!("{prefix}Proxy"),
                CFString::new(
                    settings
                        .host
                        .as_deref()
                        .unwrap_or("")
                        .trim_matches(['[', ']']),
                )
                .as_CFType(),
            );
            entries.insert(
                format!("{prefix}Port"),
                CFNumber::from(i32::from(settings.port.unwrap_or(0))).as_CFType(),
            );
        }
    }
    if settings.mode == "manual" {
        let bypass = settings
            .bypass
            .iter()
            .map(|s| CFString::new(s))
            .collect::<Vec<_>>();
        entries.insert(
            "ExceptionsList".into(),
            CFArray::from_CFTypes(&bypass).as_CFType(),
        );
    } else if settings.mode == "auto" {
        entries.insert(
            "ProxyAutoConfigEnable".into(),
            CFNumber::from(1).as_CFType(),
        );
        entries.insert(
            "ProxyAutoConfigURLString".into(),
            CFString::new(settings.pac_url.as_deref().unwrap_or("")).as_CFType(),
        );
    } else if text(original, "ProxyAutoConfigURLString").is_some_and(|s| local_pac(&s)) {
        entries.insert(
            "ProxyAutoConfigEnable".into(),
            CFNumber::from(0).as_CFType(),
        );
    }
    CFDictionary::from_CFType_pairs(
        &entries
            .into_iter()
            .map(|(k, v)| (CFString::new(&k), v))
            .collect::<Vec<_>>(),
    )
}
fn local_pac(url: &str) -> bool {
    url.strip_prefix("http://127.0.0.1:")
        .and_then(|s| s.strip_suffix("/pac"))
        .and_then(|s| s.parse::<u16>().ok())
        .is_some_and(|p| p != 0)
}
fn targets(services: &[MacNetworkService], only_active: bool) -> Vec<&MacNetworkService> {
    services
        .iter()
        .filter(|s| s.enabled && (!only_active || s.active))
        .collect()
}
struct Lock<'a>(&'a SCPreferences);
impl Drop for Lock<'_> {
    fn drop(&mut self) {
        unsafe {
            SCPreferencesUnlock(self.0.as_concrete_TypeRef());
        }
    }
}
fn write_preferences(
    prefs: &SCPreferences,
    ids: &[String],
    settings: &SystemProxySettings,
) -> Result<()> {
    if unsafe { SCPreferencesLock(prefs.as_concrete_TypeRef(), 0) } == 0 {
        bail!(error_code());
    }
    let _lock = Lock(prefs);
    for id in ids {
        let id = CFString::new(id);
        let service =
            unsafe { SCNetworkServiceCopy(prefs.as_concrete_TypeRef(), id.as_concrete_TypeRef()) };
        if service.is_null() {
            bail!("network-service-not-found");
        }
        let _service = unsafe { CFType::wrap_under_create_rule(service.cast()) };
        let kind = CFString::new("Proxies");
        let protocol = unsafe { SCNetworkServiceCopyProtocol(service, kind.as_concrete_TypeRef()) };
        if protocol.is_null() {
            bail!("configuration-unavailable");
        }
        let _protocol = unsafe { CFType::wrap_under_create_rule(protocol.cast()) };
        let original = unsafe { SCNetworkProtocolGetConfiguration(protocol) };
        let original = if original.is_null() {
            CFDictionary::from_CFType_pairs(&[] as &[(CFString, CFType)]).to_untyped()
        } else {
            unsafe { CFDictionary::wrap_under_get_rule(original) }
        };
        let config = updated_configuration(&original, settings);
        if unsafe { SCNetworkProtocolSetConfiguration(protocol, config.as_concrete_TypeRef()) } == 0
        {
            bail!(error_code());
        }
    }
    if unsafe { SCPreferencesCommitChanges(prefs.as_concrete_TypeRef()) } == 0 {
        bail!(error_code());
    }
    if unsafe { SCPreferencesApplyChanges(prefs.as_concrete_TypeRef()) } == 0 {
        bail!(error_code());
    }
    Ok(())
}
fn privileged_commands(
    services: &[&MacNetworkService],
    settings: &SystemProxySettings,
) -> Vec<Vec<String>> {
    let mut commands = vec![];
    for service in services {
        let mut add = |flag: &str, args: Vec<String>| {
            commands.push(
                [
                    vec![
                        "/usr/sbin/networksetup".into(),
                        flag.into(),
                        service.name.clone(),
                    ],
                    args,
                ]
                .concat(),
            )
        };
        if settings.mode == "manual" {
            for flag in [
                "-setwebproxy",
                "-setsecurewebproxy",
                "-setsocksfirewallproxy",
            ] {
                add(
                    flag,
                    vec![
                        settings
                            .host
                            .clone()
                            .unwrap_or_default()
                            .trim_matches(['[', ']'])
                            .into(),
                        settings.port.unwrap_or(0).to_string(),
                    ],
                );
            }
            add(
                "-setproxybypassdomains",
                if settings.bypass.is_empty() {
                    vec!["Empty".into()]
                } else {
                    settings.bypass.clone()
                },
            );
        } else {
            for flag in [
                "-setwebproxystate",
                "-setsecurewebproxystate",
                "-setsocksfirewallproxystate",
            ] {
                add(flag, vec!["off".into()]);
            }
            if settings.mode == "auto" {
                add(
                    "-setautoproxyurl",
                    vec![settings.pac_url.clone().unwrap_or_default()],
                );
                add("-setautoproxystate", vec!["on".into()]);
            } else if service
                .proxies
                .as_ref()
                .and_then(|p| p.pac_url.as_deref())
                .is_some_and(local_pac)
            {
                add("-setautoproxystate", vec!["off".into()]);
            }
        }
    }
    commands
}
pub fn set_proxy(settings: &SystemProxySettings) -> Result<SystemProxyMutation> {
    let snapshot = get_diagnostics();
    let mac = snapshot
        .macos
        .ok_or_else(|| anyhow::anyhow!("configuration-unavailable"))?;
    if mac.active_service_ids.is_empty()
        || !mac.active_service_ids.iter().all(|id| {
            mac.services
                .iter()
                .any(|s| s.id == *id && s.status == "available")
        })
    {
        bail!("network-service-not-found");
    }
    let targets = targets(&mac.services, settings.only_active_device);
    if targets.is_empty() {
        bail!("network-service-not-found");
    }
    let ids = targets.iter().map(|s| s.id.clone()).collect::<Vec<_>>();
    let prefs = preferences().ok_or_else(|| anyhow::anyhow!("configuration-unavailable"))?;
    if let Err(error) = write_preferences(&prefs, &ids, settings) {
        if error.to_string() != "permission-denied" {
            return Err(error);
        }
        // Reuse Native's existing administrator mechanism. One prompt only,
        // fixed OS utility, validated local settings and correctly quoted names.
        let shell = privileged_commands(&targets, settings)
            .iter()
            .map(|args| {
                args.iter()
                    .map(|s| crate::privileged_operations::shell_quote(s))
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect::<Vec<_>>()
            .join(" && ");
        crate::privileged_operations::run_macos_privileged_shell_bounded(&shell)?;
    }
    let after = get_diagnostics();
    let automatic_settings_preserved = targets.iter().filter_map(|s| s.proxies.as_ref()).any(|p| {
        p.auto_discovery
            || (settings.mode == "manual" && p.pac_enabled)
            || (settings.mode == "disabled"
                && p.pac_enabled
                && !p.pac_url.as_deref().is_some_and(local_pac))
    }) || after.macos.as_ref().is_some_and(|m| {
        m.services
            .iter()
            .filter(|s| ids.contains(&s.id))
            .filter_map(|s| s.proxies.as_ref())
            .any(|p| p.auto_discovery || (settings.mode != "auto" && p.pac_enabled))
    });
    Ok(SystemProxyMutation {
        automatic_settings_preserved,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn dict(pairs: &[(&str, CFType)]) -> CFDictionary {
        CFDictionary::from_CFType_pairs(
            &pairs
                .iter()
                .map(|(k, v)| (CFString::new(k), v.clone()))
                .collect::<Vec<_>>(),
        )
        .to_untyped()
    }
    #[test]
    fn normalizes_independent_protocols_and_active_automatic_settings() {
        let d = dict(&[
            ("HTTPEnable", CFNumber::from(1).as_CFType()),
            ("HTTPProxy", CFString::new("127.0.0.1").as_CFType()),
            ("HTTPPort", CFNumber::from(18423).as_CFType()),
            ("HTTPSEnable", CFNumber::from(0).as_CFType()),
            ("ProxyAutoConfigEnable", CFNumber::from(1).as_CFType()),
            ("ProxyAutoDiscoveryEnable", CFNumber::from(1).as_CFType()),
        ]);
        let p = proxy_state(&d);
        assert!(p.http.enabled);
        assert_eq!(p.http.endpoint.unwrap().port, 18423);
        assert!(!p.https.enabled);
        assert!(p.pac_enabled);
        assert!(p.auto_discovery);
    }
    #[test]
    fn manual_restore_preserves_pac_and_discovery_without_copying_credentials_to_dto() {
        let d = dict(&[
            ("ProxyAutoConfigEnable", CFNumber::from(1).as_CFType()),
            (
                "ProxyAutoConfigURLString",
                CFString::new("https://secret/private?token").as_CFType(),
            ),
            ("ProxyAutoDiscoveryEnable", CFNumber::from(1).as_CFType()),
            ("PrivateAuthentication", CFString::new("SECRET").as_CFType()),
        ]);
        let settings = SystemProxySettings {
            mode: "manual".into(),
            host: Some("127.0.0.1".into()),
            port: Some(18423),
            bypass: vec!["<local>".into()],
            pac_url: None,
            only_active_device: true,
        };
        let config = updated_configuration(&d, &settings).to_untyped();
        let state = proxy_state(&config);
        assert!(state.http.enabled && state.https.enabled && state.socks.enabled);
        assert!(state.pac_enabled && state.auto_discovery);
        assert!(text(&config, "PrivateAuthentication").is_some());
        assert!(!format!("{state:?}").contains("SECRET"));
        let service = MacNetworkService {
            name: "Wi-Fi user's service".into(),
            proxies: Some(state),
            ..Default::default()
        };
        let commands = privileged_commands(&[&service], &settings);
        assert!(
            commands
                .iter()
                .all(|c| !c[1].contains("autoproxy") && !c[1].contains("autodiscovery"))
        );
        assert!(commands.iter().all(|c| c[2] == service.name));
    }
    #[test]
    fn inactive_services_are_not_active_targets_but_multiple_service_intent_is_preserved() {
        let services = vec![
            MacNetworkService {
                id: "wifi".into(),
                enabled: true,
                active: false,
                ..Default::default()
            },
            MacNetworkService {
                id: "ethernet".into(),
                enabled: true,
                active: true,
                primary: true,
                ..Default::default()
            },
            MacNetworkService {
                id: "disabled".into(),
                enabled: false,
                active: true,
                ..Default::default()
            },
        ];
        assert_eq!(
            targets(&services, true)
                .iter()
                .map(|s| s.id.as_str())
                .collect::<Vec<_>>(),
            ["ethernet"]
        );
        assert_eq!(targets(&services, false).len(), 2);
    }
    #[test]
    fn disable_does_not_remove_foreign_pac_or_discovery() {
        let d = dict(&[
            ("ProxyAutoConfigEnable", CFNumber::from(1).as_CFType()),
            (
                "ProxyAutoConfigURLString",
                CFString::new("https://private/PAC").as_CFType(),
            ),
            ("ProxyAutoDiscoveryEnable", CFNumber::from(1).as_CFType()),
        ]);
        let settings = SystemProxySettings {
            mode: "disabled".into(),
            host: None,
            port: None,
            bypass: vec![],
            pac_url: None,
            only_active_device: true,
        };
        let p = proxy_state(&updated_configuration(&d, &settings).to_untyped());
        assert!(p.pac_enabled && p.auto_discovery);
        assert!(local_pac("http://127.0.0.1:40321/pac"));
        assert!(!local_pac("http://127.0.0.1:40321/pac?secret"));
    }
}
