use std::path::Path;

#[cfg(target_os = "linux")]
use std::path::PathBuf;
#[cfg(any(target_os = "linux", target_os = "windows"))]
use std::process::Command;
#[cfg(target_os = "linux")]
use std::{env, fs};
#[cfg(target_os = "windows")]
use std::{os::windows::process::CommandExt, process::Stdio};

#[cfg(target_os = "windows")]
use windows::{
    Win32::{
        Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS},
        System::Registry::{
            HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ, RRF_RT_REG_SZ,
            RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegGetValueW, RegSetValueExW,
        },
    },
    core::PCWSTR,
};

use anyhow::{Result, anyhow};

#[derive(Debug, Clone)]
pub struct LaunchAtLoginOptions {
    pub identifier: String,
    pub display_name: String,
    pub executable_path: String,
    pub arguments: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct LaunchAtLoginStatus {
    pub enabled: bool,
    pub requires_approval: bool,
    pub backend: String,
}

#[derive(Debug, Clone)]
pub struct NetworkContext {
    pub default_interface: Option<String>,
    pub default_service: Option<String>,
    pub dns_servers: Vec<String>,
    pub ssid: Option<String>,
}

fn validate_options(options: &LaunchAtLoginOptions) -> Result<()> {
    if options.identifier.is_empty()
        || !options.identifier.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-')
        })
    {
        return Err(anyhow!(
            "Launch-at-login identifier contains unsupported characters"
        ));
    }
    if options.display_name.trim().is_empty() {
        return Err(anyhow!("Launch-at-login display name is empty"));
    }
    if !Path::new(&options.executable_path).is_absolute() {
        return Err(anyhow!("Launch-at-login executable path must be absolute"));
    }
    if options
        .arguments
        .iter()
        .any(|argument| argument.contains('\0'))
    {
        return Err(anyhow!(
            "Launch-at-login arguments must not contain NUL characters"
        ));
    }
    Ok(())
}

pub fn get_launch_at_login(options: &LaunchAtLoginOptions) -> Result<LaunchAtLoginStatus> {
    validate_options(options)?;
    platform_get_launch_at_login(options)
}

pub fn set_launch_at_login(
    options: &LaunchAtLoginOptions,
    enabled: bool,
) -> Result<LaunchAtLoginStatus> {
    validate_options(options)?;
    platform_set_launch_at_login(options, enabled)?;
    platform_get_launch_at_login(options)
}

pub fn get_network_context() -> Result<NetworkContext> {
    Ok(platform_network_context())
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
fn command_output(program: &str, arguments: &[&str]) -> Option<String> {
    let output = Command::new(program).args(arguments).output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

#[cfg(target_os = "linux")]
fn config_home() -> Option<PathBuf> {
    env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
}

#[cfg(target_os = "linux")]
fn autostart_path(options: &LaunchAtLoginOptions) -> Result<PathBuf> {
    Ok(config_home()
        .ok_or_else(|| anyhow!("Unable to determine XDG config directory"))?
        .join("autostart")
        .join(format!("{}.desktop", options.identifier)))
}

#[cfg(target_os = "linux")]
fn desktop_exec_argument(argument: &str) -> String {
    if !argument
        .chars()
        .any(|character| character.is_whitespace() || matches!(character, '"' | '\\' | '$' | '`'))
    {
        return argument.to_string();
    }
    format!(
        "\"{}\"",
        argument
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('$', "\\$")
            .replace('`', "\\`")
    )
}

#[cfg(target_os = "linux")]
fn desktop_entry(options: &LaunchAtLoginOptions) -> String {
    let executable = std::iter::once(options.executable_path.as_str())
        .chain(options.arguments.iter().map(String::as_str))
        .map(desktop_exec_argument)
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "[Desktop Entry]\nVersion=1.0\nType=Application\nName={}\nExec={}\nTerminal=false\nX-GNOME-Autostart-enabled=true\n",
        options.display_name.replace('\n', " "),
        executable
    )
}

#[cfg(target_os = "linux")]
fn platform_get_launch_at_login(options: &LaunchAtLoginOptions) -> Result<LaunchAtLoginStatus> {
    Ok(LaunchAtLoginStatus {
        enabled: autostart_path(options)?.is_file(),
        requires_approval: false,
        backend: "linux-xdg-autostart".to_string(),
    })
}

#[cfg(target_os = "linux")]
fn platform_set_launch_at_login(options: &LaunchAtLoginOptions, enabled: bool) -> Result<()> {
    let target = autostart_path(options)?;
    if enabled {
        let directory = target
            .parent()
            .ok_or_else(|| anyhow!("Invalid XDG autostart path"))?;
        fs::create_dir_all(directory)?;
        let temporary = target.with_extension(format!("desktop.{}.tmp", std::process::id()));
        fs::write(&temporary, desktop_entry(options))?;
        fs::rename(temporary, target)?;
    } else if target.exists() {
        fs::remove_file(target)?;
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn mac_application_path(executable_path: &str) -> Result<String> {
    let marker = ".app";
    let Some(index) = executable_path.find(marker) else {
        return Err(anyhow!(
            "macOS login items require an application bundle path"
        ));
    };
    Ok(executable_path[..index + marker.len()].to_string())
}

#[cfg(target_os = "macos")]
fn mac_main_app_service(
    options: &LaunchAtLoginOptions,
) -> Result<objc2::rc::Retained<objc2_service_management::SMAppService>> {
    mac_application_path(&options.executable_path)?;
    if !options.arguments.is_empty() {
        return Err(anyhow!(
            "macOS main-app launch at login does not support command-line arguments"
        ));
    }

    // SAFETY: `mainAppService` is a process-wide ServiceManagement singleton
    // accessor and returns a retained object owned by this scope.
    Ok(unsafe { objc2_service_management::SMAppService::mainAppService() })
}

#[cfg(target_os = "macos")]
fn platform_get_launch_at_login(options: &LaunchAtLoginOptions) -> Result<LaunchAtLoginStatus> {
    use objc2_service_management::SMAppServiceStatus;

    let service = mac_main_app_service(options)?;
    // SAFETY: The retained service remains alive for the status query.
    let status = unsafe { service.status() };
    Ok(LaunchAtLoginStatus {
        enabled: status == SMAppServiceStatus::Enabled,
        requires_approval: status == SMAppServiceStatus::RequiresApproval,
        backend: "macos-sm-app-service".to_string(),
    })
}

#[cfg(target_os = "macos")]
fn platform_set_launch_at_login(options: &LaunchAtLoginOptions, enabled: bool) -> Result<()> {
    use objc2_service_management::SMAppServiceStatus;

    let service = mac_main_app_service(options)?;
    // SAFETY: The retained service remains alive for the operation.
    let status = unsafe { service.status() };
    if enabled {
        if matches!(
            status,
            SMAppServiceStatus::NotRegistered | SMAppServiceStatus::NotFound
        ) {
            // SAFETY: The call is made on the main-app service owned by the
            // current signed application bundle.
            unsafe { service.registerAndReturnError() }
                .map_err(|error| anyhow!("Unable to register launch at login: {error}"))?;
        }
    } else if !matches!(
        status,
        SMAppServiceStatus::NotRegistered | SMAppServiceStatus::NotFound
    ) {
        // SAFETY: The call is made on the retained main-app service.
        unsafe { service.unregisterAndReturnError() }
            .map_err(|error| anyhow!("Unable to unregister launch at login: {error}"))?;
    }
    Ok(())
}

#[cfg(target_os = "windows")]
const WINDOWS_RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

#[cfg(target_os = "windows")]
fn wide_null(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(target_os = "windows")]
fn quote_windows_command_argument(argument: &str) -> String {
    if !argument.is_empty()
        && !argument
            .chars()
            .any(|character| matches!(character, ' ' | '\t' | '\n' | '\r' | '"'))
    {
        return argument.to_string();
    }

    let mut quoted = String::from("\"");
    let mut backslashes = 0usize;
    for character in argument.chars() {
        match character {
            '\\' => backslashes += 1,
            '"' => {
                quoted.push_str(&"\\".repeat(backslashes * 2 + 1));
                quoted.push('"');
                backslashes = 0;
            }
            _ => {
                quoted.push_str(&"\\".repeat(backslashes));
                backslashes = 0;
                quoted.push(character);
            }
        }
    }
    quoted.push_str(&"\\".repeat(backslashes * 2));
    quoted.push('"');
    quoted
}

#[cfg(target_os = "windows")]
fn windows_run_value(options: &LaunchAtLoginOptions) -> String {
    std::iter::once(options.executable_path.as_str())
        .chain(options.arguments.iter().map(String::as_str))
        .map(quote_windows_command_argument)
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(target_os = "windows")]
struct RegistryKey(HKEY);

#[cfg(target_os = "windows")]
impl Drop for RegistryKey {
    fn drop(&mut self) {
        unsafe {
            let _ = RegCloseKey(self.0);
        }
    }
}

#[cfg(target_os = "windows")]
fn windows_run_value_exists(identifier: &str) -> Result<bool> {
    let key = wide_null(WINDOWS_RUN_KEY);
    let name = wide_null(identifier);
    let result = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            PCWSTR(key.as_ptr()),
            PCWSTR(name.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            None,
            None,
        )
    };
    if result == ERROR_SUCCESS {
        return Ok(true);
    }
    if result == ERROR_FILE_NOT_FOUND {
        return Ok(false);
    }
    Err(anyhow!(
        "Unable to query launch-at-login registry value (error code: {})",
        result.0
    ))
}

#[cfg(target_os = "windows")]
fn set_windows_run_value(options: &LaunchAtLoginOptions) -> Result<()> {
    let key_path = wide_null(WINDOWS_RUN_KEY);
    let name = wide_null(&options.identifier);
    let mut key = HKEY::default();
    let result = unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(key_path.as_ptr()),
            None,
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            None,
            &mut key,
            None,
        )
    };
    if result != ERROR_SUCCESS {
        return Err(anyhow!(
            "Unable to open launch-at-login registry key (error code: {})",
            result.0
        ));
    }
    let key = RegistryKey(key);
    let value = wide_null(&windows_run_value(options))
        .into_iter()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    let result =
        unsafe { RegSetValueExW(key.0, PCWSTR(name.as_ptr()), None, REG_SZ, Some(&value)) };
    if result != ERROR_SUCCESS {
        return Err(anyhow!(
            "Unable to set launch-at-login registry value (error code: {})",
            result.0
        ));
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn delete_windows_run_value(identifier: &str) -> Result<()> {
    let key_path = wide_null(WINDOWS_RUN_KEY);
    let name = wide_null(identifier);
    let mut key = HKEY::default();
    let result = unsafe {
        windows::Win32::System::Registry::RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(key_path.as_ptr()),
            None,
            KEY_SET_VALUE,
            &mut key,
        )
    };
    if result == ERROR_FILE_NOT_FOUND {
        return Ok(());
    }
    if result != ERROR_SUCCESS {
        return Err(anyhow!(
            "Unable to open launch-at-login registry key (error code: {})",
            result.0
        ));
    }
    let key = RegistryKey(key);
    let result = unsafe { RegDeleteValueW(key.0, PCWSTR(name.as_ptr())) };
    if result != ERROR_SUCCESS && result != ERROR_FILE_NOT_FOUND {
        return Err(anyhow!(
            "Unable to delete launch-at-login registry value (error code: {})",
            result.0
        ));
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn windows_task_exists(identifier: &str) -> bool {
    windows_task_command()
        .args(["/query", "/tn", identifier])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

#[cfg(target_os = "windows")]
fn windows_task_command() -> Command {
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let mut command = Command::new("schtasks.exe");
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

#[cfg(target_os = "windows")]
fn task_command_error(operation: &str, exit_code: Option<i32>) -> anyhow::Error {
    let exit_code = exit_code
        .map(|code| code.to_string())
        .unwrap_or_else(|| "unknown".to_string());
    anyhow!("Unable to {operation} launch-at-login task (exit code: {exit_code})")
}

#[cfg(target_os = "windows")]
fn run_windows_task_command(arguments: &[String], operation: &str) -> Result<()> {
    let status = windows_task_command()
        .args(arguments)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if !status.success() {
        return Err(task_command_error(operation, status.code()));
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn run_elevated_windows_task_command(arguments: &[String], operation: &str) -> Result<()> {
    let exit_code = crate::run_elevated("schtasks.exe", arguments)?;
    if exit_code != 0 {
        return Err(anyhow!(
            "Unable to {operation} legacy launch-at-login task (exit code: {exit_code})"
        ));
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn delete_windows_task_arguments(identifier: &str) -> Vec<String> {
    vec![
        "/delete".to_string(),
        "/tn".to_string(),
        identifier.to_string(),
        "/f".to_string(),
    ]
}

#[cfg(target_os = "windows")]
fn platform_get_launch_at_login(options: &LaunchAtLoginOptions) -> Result<LaunchAtLoginStatus> {
    Ok(LaunchAtLoginStatus {
        enabled: windows_run_value_exists(&options.identifier)?
            || windows_task_exists(&options.identifier),
        requires_approval: false,
        backend: "windows-current-user-run".to_string(),
    })
}

#[cfg(target_os = "windows")]
fn platform_set_launch_at_login(options: &LaunchAtLoginOptions, enabled: bool) -> Result<()> {
    if !enabled {
        delete_windows_run_value(&options.identifier)?;
        if windows_task_exists(&options.identifier) {
            let arguments = delete_windows_task_arguments(&options.identifier);
            run_windows_task_command(&arguments, "delete").or_else(|_| {
                // Versions before 0.5.3 used Task Scheduler. Removing an
                // elevated legacy task may require one final UAC prompt.
                run_elevated_windows_task_command(&arguments, "delete")
            })?;
        }
        return Ok(());
    }

    if windows_task_exists(&options.identifier) {
        let delete_arguments = delete_windows_task_arguments(&options.identifier);
        run_windows_task_command(&delete_arguments, "delete")
            .or_else(|_| run_elevated_windows_task_command(&delete_arguments, "migrate"))?;
    }
    set_windows_run_value(options)
}

#[cfg(all(test, target_os = "windows"))]
mod windows_tests {
    use super::{LaunchAtLoginOptions, windows_run_value};

    #[test]
    fn launch_at_login_registry_value_quotes_every_argument() {
        let value = windows_run_value(&LaunchAtLoginOptions {
            identifier: "KokoroBox".to_string(),
            display_name: "KokoroBox".to_string(),
            executable_path: r"C:\Program Files\KokoroBox\KokoroBox.exe".to_string(),
            arguments: vec!["--flag".to_string(), "value with spaces".to_string()],
        });

        assert_eq!(
            value,
            r#""C:\Program Files\KokoroBox\KokoroBox.exe" --flag "value with spaces""#
        );
    }
}

#[cfg(target_os = "linux")]
fn platform_default_interface() -> Option<String> {
    let routes = fs::read_to_string("/proc/net/route").ok()?;
    routes.lines().skip(1).find_map(|line| {
        let columns = line.split_whitespace().collect::<Vec<_>>();
        (columns.get(1) == Some(&"00000000")
            && columns.get(3).is_some_and(|flags| {
                u32::from_str_radix(flags, 16).is_ok_and(|value| value & 0x2 != 0)
            }))
        .then(|| columns[0].to_string())
    })
}

#[cfg(target_os = "linux")]
fn resolv_conf_dns_servers() -> Vec<String> {
    let mut servers: Vec<String> = fs::read_to_string("/etc/resolv.conf")
        .map(|content| {
            content
                .lines()
                .filter_map(|line| {
                    let mut columns = line.split_whitespace();
                    (columns.next() == Some("nameserver"))
                        .then(|| columns.next().map(ToOwned::to_owned))
                        .flatten()
                })
                .collect()
        })
        .unwrap_or_default();
    servers.sort();
    servers.dedup();
    servers
}

#[cfg(target_os = "linux")]
fn linux_dns_servers(interface: Option<&str>) -> Vec<String> {
    let resolved = interface
        .and_then(|interface| command_output("resolvectl", &["dns", interface]))
        .map(|output| {
            output
                .split_once(':')
                .map(|(_, servers)| servers)
                .unwrap_or_default()
                .split_whitespace()
                .filter(|server| server.parse::<std::net::IpAddr>().is_ok())
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if resolved.is_empty() {
        resolv_conf_dns_servers()
    } else {
        resolved
    }
}

#[cfg(target_os = "linux")]
fn linux_ssid() -> Option<String> {
    command_output("/usr/sbin/iwgetid", &["--raw"])
        .or_else(|| command_output("iwgetid", &["--raw"]))
        .or_else(|| {
            command_output(
                "nmcli",
                &["--terse", "--fields", "ACTIVE,SSID", "device", "wifi"],
            )
            .and_then(|output| {
                output.lines().find_map(|line| {
                    line.strip_prefix("yes:")
                        .map(|ssid| ssid.replace("\\:", ":").replace("\\\\", "\\"))
                })
            })
        })
        .map(|ssid| ssid.trim().to_string())
        .filter(|ssid| !ssid.is_empty())
}

#[cfg(target_os = "linux")]
fn platform_network_context() -> NetworkContext {
    let default_interface = platform_default_interface();
    NetworkContext {
        default_service: default_interface.clone(),
        dns_servers: linux_dns_servers(default_interface.as_deref()),
        ssid: linux_ssid(),
        default_interface,
    }
}

#[cfg(target_os = "macos")]
fn mac_dictionary(
    store: &system_configuration::dynamic_store::SCDynamicStore,
    key: &str,
) -> Option<system_configuration::core_foundation::dictionary::CFDictionary> {
    use system_configuration::core_foundation::propertylist::CFPropertyList;

    store.get(key).and_then(
        CFPropertyList::downcast_into::<
            system_configuration::core_foundation::dictionary::CFDictionary,
        >,
    )
}

#[cfg(target_os = "macos")]
fn mac_dictionary_string(
    dictionary: &system_configuration::core_foundation::dictionary::CFDictionary,
    key: &str,
) -> Option<String> {
    use system_configuration::core_foundation::{
        base::{CFType, TCFType, ToVoid},
        string::CFString,
    };

    let key = CFString::new(key);
    dictionary
        .find(key.to_void())
        .map(|pointer| unsafe { CFType::wrap_under_get_rule(*pointer) })
        .and_then(CFType::downcast_into::<CFString>)
        .map(|value| value.to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(target_os = "macos")]
fn mac_dictionary_strings(
    dictionary: &system_configuration::core_foundation::dictionary::CFDictionary,
    key: &str,
) -> Vec<String> {
    use system_configuration::core_foundation::{
        array::CFArray,
        base::{CFType, TCFType, ToVoid},
        string::CFString,
    };

    let key = CFString::new(key);
    let Some(values) = dictionary
        .find(key.to_void())
        .map(|pointer| unsafe { CFType::wrap_under_get_rule(*pointer) })
        .and_then(CFType::downcast_into::<CFArray>)
    else {
        return Vec::new();
    };

    let mut result = values
        .iter()
        .filter_map(|pointer| {
            unsafe { CFType::wrap_under_get_rule(*pointer) }
                .downcast_into::<CFString>()
                .map(|value| value.to_string())
        })
        .filter(|value| value.parse::<std::net::IpAddr>().is_ok())
        .collect::<Vec<_>>();
    result.sort();
    result.dedup();
    result
}

#[cfg(target_os = "macos")]
fn mac_primary_network(
    store: &system_configuration::dynamic_store::SCDynamicStore,
) -> (Option<String>, Option<String>) {
    ["State:/Network/Global/IPv4", "State:/Network/Global/IPv6"]
        .into_iter()
        .find_map(|key| {
            let dictionary = mac_dictionary(store, key)?;
            let interface = mac_dictionary_string(&dictionary, "PrimaryInterface");
            let service = mac_dictionary_string(&dictionary, "PrimaryService");
            (interface.is_some() || service.is_some()).then_some((interface, service))
        })
        .unwrap_or_default()
}

#[cfg(target_os = "windows")]
fn windows_ip_configuration() -> (Option<String>, Vec<String>) {
    use windows::Win32::{
        Foundation::{ERROR_BUFFER_OVERFLOW, ERROR_SUCCESS},
        NetworkManagement::IpHelper::{
            GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_MULTICAST, GetAdaptersAddresses,
            GetBestInterfaceEx, IP_ADAPTER_ADDRESSES_LH,
        },
        Networking::WinSock::{
            AF_INET, AF_INET6, AF_UNSPEC, IN_ADDR, IN_ADDR_0, IN_ADDR_0_0, IN6_ADDR, IN6_ADDR_0,
            SOCKADDR, SOCKADDR_IN, SOCKADDR_IN6,
        },
    };

    fn best_interface_index() -> Option<(u32, bool)> {
        let mut index = 0;
        let ipv4 = SOCKADDR_IN {
            sin_family: AF_INET,
            sin_addr: IN_ADDR {
                S_un: IN_ADDR_0 {
                    S_un_b: IN_ADDR_0_0 {
                        s_b1: 8,
                        s_b2: 8,
                        s_b3: 8,
                        s_b4: 8,
                    },
                },
            },
            ..Default::default()
        };
        if unsafe { GetBestInterfaceEx((&raw const ipv4).cast::<SOCKADDR>(), &mut index) }
            == ERROR_SUCCESS.0
        {
            return Some((index, false));
        }

        let ipv6 = SOCKADDR_IN6 {
            sin6_family: AF_INET6,
            sin6_addr: IN6_ADDR {
                u: IN6_ADDR_0 {
                    Byte: [
                        0x20, 0x01, 0x48, 0x60, 0x48, 0x60, 0, 0, 0, 0, 0, 0, 0, 0, 0x88, 0x88,
                    ],
                },
            },
            ..Default::default()
        };
        (unsafe { GetBestInterfaceEx((&raw const ipv6).cast::<SOCKADDR>(), &mut index) }
            == ERROR_SUCCESS.0)
            .then_some((index, true))
    }

    unsafe fn socket_address_to_string(address: *const SOCKADDR) -> Option<String> {
        if address.is_null() {
            return None;
        }
        match unsafe { (*address).sa_family } {
            AF_INET => {
                let address = unsafe { &*address.cast::<SOCKADDR_IN>() };
                let bytes = unsafe { address.sin_addr.S_un.S_un_b };
                Some(
                    std::net::Ipv4Addr::new(bytes.s_b1, bytes.s_b2, bytes.s_b3, bytes.s_b4)
                        .to_string(),
                )
            }
            AF_INET6 => {
                let address = unsafe { &*address.cast::<SOCKADDR_IN6>() };
                Some(std::net::Ipv6Addr::from(unsafe { address.sin6_addr.u.Byte }).to_string())
            }
            _ => None,
        }
    }

    let Some((best_index, ipv6)) = best_interface_index() else {
        return (None, Vec::new());
    };
    let flags = GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST;
    let mut byte_count = 0;
    if unsafe { GetAdaptersAddresses(AF_UNSPEC.0 as u32, flags, None, None, &mut byte_count) }
        != ERROR_BUFFER_OVERFLOW.0
        || byte_count == 0
    {
        return (None, Vec::new());
    }

    let word_count = (byte_count as usize).div_ceil(std::mem::size_of::<u64>());
    let mut buffer = vec![0_u64; word_count];
    let adapters = buffer.as_mut_ptr().cast::<IP_ADAPTER_ADDRESSES_LH>();
    if unsafe {
        GetAdaptersAddresses(
            AF_UNSPEC.0 as u32,
            flags,
            None,
            Some(adapters),
            &mut byte_count,
        )
    } != ERROR_SUCCESS.0
    {
        return (None, Vec::new());
    }

    let mut current = adapters;
    let mut selected = None;
    while !current.is_null() {
        let adapter = unsafe { &*current };
        let interface_index = if ipv6 {
            adapter.Ipv6IfIndex
        } else {
            unsafe { adapter.Anonymous1.Anonymous.IfIndex }
        };
        if interface_index == best_index {
            selected = Some(adapter);
            break;
        }
        current = adapter.Next;
    }
    let Some(adapter) = selected else {
        return (None, Vec::new());
    };

    let default_interface = if adapter.FriendlyName.is_null() {
        None
    } else {
        unsafe { adapter.FriendlyName.to_string().ok() }.filter(|name| !name.is_empty())
    };
    let mut dns_servers = Vec::new();
    let mut dns = adapter.FirstDnsServerAddress;
    while !dns.is_null() {
        let address = unsafe { &*dns };
        if let Some(value) = unsafe { socket_address_to_string(address.Address.lpSockaddr) } {
            dns_servers.push(value);
        }
        dns = address.Next;
    }
    dns_servers.sort();
    dns_servers.dedup();
    (default_interface, dns_servers)
}

#[cfg(target_os = "windows")]
fn windows_ssid() -> Option<String> {
    use std::{ffi::c_void, ptr::null_mut, slice};
    use windows::Win32::{
        Foundation::{ERROR_SUCCESS, HANDLE},
        NetworkManagement::WiFi::{
            WLAN_API_VERSION_2_0, WLAN_CONNECTION_ATTRIBUTES, WLAN_INTERFACE_INFO_LIST,
            WlanCloseHandle, WlanEnumInterfaces, WlanFreeMemory, WlanOpenHandle,
            WlanQueryInterface, wlan_interface_state_connected,
            wlan_intf_opcode_current_connection,
        },
    };

    unsafe {
        let mut negotiated_version = 0;
        let mut handle = HANDLE::default();
        if WlanOpenHandle(
            WLAN_API_VERSION_2_0,
            None,
            &mut negotiated_version,
            &mut handle,
        ) != ERROR_SUCCESS.0
        {
            return None;
        }

        let mut list: *mut WLAN_INTERFACE_INFO_LIST = null_mut();
        let mut result = None;
        if WlanEnumInterfaces(handle, None, &mut list) == ERROR_SUCCESS.0 && !list.is_null() {
            let interface_list = &*list;
            let interfaces = slice::from_raw_parts(
                interface_list.InterfaceInfo.as_ptr(),
                interface_list.dwNumberOfItems as usize,
            );
            for interface in interfaces {
                if interface.isState != wlan_interface_state_connected {
                    continue;
                }
                let mut size = 0;
                let mut data: *mut c_void = null_mut();
                if WlanQueryInterface(
                    handle,
                    &interface.InterfaceGuid,
                    wlan_intf_opcode_current_connection,
                    None,
                    &mut size,
                    &mut data,
                    None,
                ) == ERROR_SUCCESS.0
                    && !data.is_null()
                {
                    let attributes = &*(data as *const WLAN_CONNECTION_ATTRIBUTES);
                    let ssid = &attributes.wlanAssociationAttributes.dot11Ssid;
                    let length = (ssid.uSSIDLength as usize).min(ssid.ucSSID.len());
                    let value = String::from_utf8_lossy(&ssid.ucSSID[..length])
                        .trim()
                        .to_string();
                    WlanFreeMemory(data);
                    if !value.is_empty() {
                        result = Some(value);
                        break;
                    }
                }
            }
            WlanFreeMemory(list.cast());
        }
        let _ = WlanCloseHandle(handle, None);
        result
    }
}

#[cfg(target_os = "windows")]
fn platform_network_context() -> NetworkContext {
    let (default_interface, dns_servers) = windows_ip_configuration();
    NetworkContext {
        default_interface,
        default_service: None,
        dns_servers,
        ssid: windows_ssid(),
    }
}

#[cfg(target_os = "macos")]
fn platform_network_context() -> NetworkContext {
    use system_configuration::dynamic_store::SCDynamicStoreBuilder;

    let Some(store) = SCDynamicStoreBuilder::new("KokoroBox network context").build() else {
        return NetworkContext {
            default_interface: None,
            default_service: None,
            dns_servers: Vec::new(),
            ssid: None,
        };
    };
    let (default_interface, service_identifier) = mac_primary_network(&store);
    let default_service = service_identifier.as_deref().and_then(|identifier| {
        mac_dictionary(&store, &format!("Setup:/Network/Service/{identifier}"))
            .and_then(|dictionary| mac_dictionary_string(&dictionary, "UserDefinedName"))
    });
    let dns_servers = service_identifier
        .as_deref()
        .and_then(|identifier| {
            mac_dictionary(&store, &format!("State:/Network/Service/{identifier}/DNS"))
        })
        .or_else(|| mac_dictionary(&store, "State:/Network/Global/DNS"))
        .map(|dictionary| mac_dictionary_strings(&dictionary, "ServerAddresses"))
        .unwrap_or_default();
    let ssid = default_interface.as_deref().and_then(|interface| {
        mac_dictionary(
            &store,
            &format!("State:/Network/Interface/{interface}/AirPort"),
        )
        .and_then(|dictionary| mac_dictionary_string(&dictionary, "SSID_STR"))
    });
    NetworkContext {
        dns_servers,
        ssid,
        default_interface,
        default_service,
    }
}

#[cfg(all(test, target_os = "macos"))]
mod macos_tests {
    use super::{mac_dictionary_string, mac_dictionary_strings};
    use system_configuration::core_foundation::{
        array::CFArray, dictionary::CFDictionary, string::CFString,
    };

    #[test]
    fn reads_dynamic_store_string_values() {
        let dictionary = CFDictionary::from_CFType_pairs(&[(
            CFString::new("PrimaryInterface"),
            CFString::new("en0"),
        )]);

        assert_eq!(
            mac_dictionary_string(&dictionary.to_untyped(), "PrimaryInterface").as_deref(),
            Some("en0")
        );
    }

    #[test]
    fn filters_and_deduplicates_dynamic_store_dns_values() {
        let servers = CFArray::from_CFTypes(&[
            CFString::new("192.168.1.1"),
            CFString::new("not-an-address"),
            CFString::new("192.168.1.1"),
            CFString::new("2001:db8::1"),
        ]);
        let dictionary =
            CFDictionary::from_CFType_pairs(&[(CFString::new("ServerAddresses"), servers)]);

        assert_eq!(
            mac_dictionary_strings(&dictionary.to_untyped(), "ServerAddresses"),
            ["192.168.1.1", "2001:db8::1"]
        );
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn platform_get_launch_at_login(_options: &LaunchAtLoginOptions) -> Result<LaunchAtLoginStatus> {
    Err(anyhow!("Launch-at-login is unsupported on this platform"))
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn platform_set_launch_at_login(_options: &LaunchAtLoginOptions, _enabled: bool) -> Result<()> {
    Err(anyhow!("Launch-at-login is unsupported on this platform"))
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn platform_network_context() -> NetworkContext {
    NetworkContext {
        default_interface: None,
        default_service: None,
        dns_servers: Vec::new(),
        ssid: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unsafe_launch_identifier() {
        let options = LaunchAtLoginOptions {
            identifier: "KokoroBox/unsafe".to_string(),
            display_name: "KokoroBox".to_string(),
            executable_path: "/Applications/KokoroBox.app".to_string(),
            arguments: Vec::new(),
        };
        assert!(validate_options(&options).is_err());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn desktop_entry_escapes_spaced_arguments() {
        let options = LaunchAtLoginOptions {
            identifier: "com.amamiyakokoro.kokorobox".to_string(),
            display_name: "KokoroBox".to_string(),
            executable_path: "/opt/KokoroBox/KokoroBox".to_string(),
            arguments: vec!["--profile path".to_string()],
        };
        assert!(desktop_entry(&options).contains("\"--profile path\""));
    }
}
