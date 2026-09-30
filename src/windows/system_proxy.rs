//! Read HKCU in the Desktop user's native process, never in the service account.
//! No subprocesses, network probes, or runtime/service dependencies.
use crate::system_proxy::*;
use anyhow::{Result, bail};
use std::{ffi::c_void, mem::size_of};
use windows::{
    Win32::{
        Foundation::{ERROR_FILE_NOT_FOUND, GlobalFree, HGLOBAL},
        Networking::{WinHttp::*, WinInet::*},
        System::Registry::*,
    },
    core::{PCWSTR, PWSTR, w},
};

const INTERNET_SETTINGS: PCWSTR =
    w!("Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings");
fn wide(v: &str) -> Vec<u16> {
    v.encode_utf16().chain(Some(0)).collect()
}

fn read_string(name: PCWSTR) -> Result<String> {
    // Bounded REG_SZ reads; absent values mean no configuration, other errors stay unknown.
    let mut value = vec![0u16; 32768];
    let mut bytes = (value.len() * 2) as u32;
    let result = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            INTERNET_SETTINGS,
            name,
            RRF_RT_REG_SZ,
            None,
            Some(value.as_mut_ptr().cast()),
            Some(&mut bytes),
        )
    };
    if result == ERROR_FILE_NOT_FOUND {
        return Ok(String::new());
    }
    result
        .ok()
        .map_err(|_| anyhow::anyhow!("registry-read-failed"))?;
    value.truncate((bytes as usize / 2).min(value.len()));
    Ok(String::from_utf16_lossy(&value)
        .trim_end_matches('\0')
        .into())
}
fn read_enabled() -> Result<bool> {
    let mut value = 0u32;
    let mut bytes = size_of::<u32>() as u32;
    let result = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            INTERNET_SETTINGS,
            w!("ProxyEnable"),
            RRF_RT_REG_DWORD,
            None,
            Some((&raw mut value).cast()),
            Some(&mut bytes),
        )
    };
    if result == ERROR_FILE_NOT_FOUND {
        return Ok(false);
    }
    result
        .ok()
        .map_err(|_| anyhow::anyhow!("registry-read-failed"))?;
    Ok(value != 0)
}
fn win_http() -> WinHttpProxyState {
    let mut info = WINHTTP_PROXY_INFO::default();
    if unsafe { WinHttpGetDefaultProxyConfiguration(&mut info) }.is_err() {
        return WinHttpProxyState {
            status: "unavailable".into(),
            mode: None,
            proxy: None,
            bypass: None,
            error_code: Some("winhttp-read-failed".into()),
        };
    }
    let text = |p: PWSTR| {
        if p.is_null() {
            None
        } else {
            Some(unsafe { p.to_string() }.unwrap_or_default())
        }
    };
    let state = WinHttpProxyState {
        status: "available".into(),
        mode: Some(
            if info.dwAccessType == WINHTTP_ACCESS_TYPE_NO_PROXY {
                "direct"
            } else if info.dwAccessType == WINHTTP_ACCESS_TYPE_NAMED_PROXY {
                "proxy"
            } else {
                "advanced"
            }
            .into(),
        ),
        proxy: text(info.lpszProxy),
        bypass: text(info.lpszProxyBypass),
        error_code: None,
    };
    unsafe {
        if !info.lpszProxy.is_null() {
            let _ = GlobalFree(Some(HGLOBAL(info.lpszProxy.0.cast())));
        }
        if !info.lpszProxyBypass.is_null() {
            let _ = GlobalFree(Some(HGLOBAL(info.lpszProxyBypass.0.cast())));
        }
    }
    state
}

pub fn get_diagnostics() -> SystemProxyDiagnostics {
    let registry = (|| -> Result<_> {
        Ok((
            read_enabled()?,
            read_string(w!("ProxyServer"))?,
            read_string(w!("ProxyOverride"))?,
            read_string(w!("AutoConfigURL"))?,
        ))
    })();
    let count = super::uwp_loopback::loopback_exemption_count();
    let count_failed = count.is_err();
    let app_container = AppContainerState {
        supported: true,
        status: if count.is_ok() {
            "available"
        } else {
            "unavailable"
        }
        .into(),
        loopback_exemption_count: count.ok(),
        error_code: if count_failed {
            Some("appcontainer-read-failed".into())
        } else {
            None
        },
    };
    let mut state = SystemProxyDiagnostics {
        platform: "windows".into(),
        status: "unavailable".into(),
        error_code: Some("registry-read-failed".into()),
        windows: Some(WindowsProxyDetails {
            proxy_server: None,
            proxy_override: None,
            auto_config_url: None,
            win_http: win_http(),
            app_container,
        }),
        ..Default::default()
    };
    if let Ok((enabled, server, bypass, pac)) = registry {
        state.status = "available".into();
        state.error_code = None;
        state.enabled = Some(enabled);
        for entry in server.split(';') {
            let (protocol, address) = entry.split_once('=').unwrap_or(("all", entry));
            if let Some(endpoint) = parse_endpoint(address) {
                match protocol.trim().to_ascii_lowercase().as_str() {
                    "all" => {
                        state.http = Some(endpoint.clone());
                        state.https = Some(endpoint);
                    }
                    "http" => state.http = Some(endpoint),
                    "https" => state.https = Some(endpoint),
                    "socks" => state.socks = Some(endpoint),
                    _ => (),
                }
            }
        }
        state.pac_enabled = Some(!pac.is_empty());
        state.pac_url = Some(pac.clone());
        state.bypass = bypass
            .split([';', ','])
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(str::to_owned)
            .collect();
        if let Some(details) = state.windows.as_mut() {
            details.proxy_server = Some(server);
            details.proxy_override = Some(bypass);
            details.auto_config_url = Some(pac);
        }
    }
    state
}

pub fn set_proxy(settings: &SystemProxySettings) -> Result<()> {
    let address = settings
        .host
        .as_ref()
        .map(|host| {
            format!(
                "{}:{}",
                if host == "::1" { "[::1]" } else { host },
                settings.port.unwrap_or(0)
            )
        })
        .unwrap_or_default();
    let mut server = wide(&address);
    let mut bypass = wide(&settings.bypass.join(";"));
    let mut pac = wide(settings.pac_url.as_deref().unwrap_or(""));
    let flags = PROXY_TYPE_DIRECT
        | match settings.mode.as_str() {
            "manual" => PROXY_TYPE_PROXY,
            "auto" => PROXY_TYPE_AUTO_PROXY_URL,
            _ => 0,
        };
    let mut options = [
        INTERNET_PER_CONN_OPTIONW {
            dwOption: INTERNET_PER_CONN_FLAGS,
            Value: INTERNET_PER_CONN_OPTIONW_0 { dwValue: flags },
        },
        INTERNET_PER_CONN_OPTIONW {
            dwOption: INTERNET_PER_CONN_PROXY_SERVER,
            Value: INTERNET_PER_CONN_OPTIONW_0 {
                pszValue: PWSTR(server.as_mut_ptr()),
            },
        },
        INTERNET_PER_CONN_OPTIONW {
            dwOption: INTERNET_PER_CONN_PROXY_BYPASS,
            Value: INTERNET_PER_CONN_OPTIONW_0 {
                pszValue: PWSTR(bypass.as_mut_ptr()),
            },
        },
        INTERNET_PER_CONN_OPTIONW {
            dwOption: INTERNET_PER_CONN_AUTOCONFIG_URL,
            Value: INTERNET_PER_CONN_OPTIONW_0 {
                pszValue: PWSTR(pac.as_mut_ptr()),
            },
        },
    ];
    let list = INTERNET_PER_CONN_OPTION_LISTW {
        dwSize: size_of::<INTERNET_PER_CONN_OPTION_LISTW>() as u32,
        dwOptionCount: options.len() as u32,
        pOptions: options.as_mut_ptr(),
        ..Default::default()
    };
    unsafe {
        if InternetSetOptionW(
            None,
            INTERNET_OPTION_PER_CONNECTION_OPTION,
            Some((&raw const list).cast::<c_void>()),
            size_of::<INTERNET_PER_CONN_OPTION_LISTW>() as u32,
        )
        .is_err()
        {
            bail!("system-proxy-write-failed");
        }
        InternetSetOptionW(None, INTERNET_OPTION_SETTINGS_CHANGED, None, 0)
            .map_err(|_| anyhow::anyhow!("system-proxy-notify-failed"))?;
        InternetSetOptionW(None, INTERNET_OPTION_REFRESH, None, 0)
            .map_err(|_| anyhow::anyhow!("system-proxy-notify-failed"))?;
    }
    Ok(())
}
