use std::{path::Path, process::Command};

#[cfg(target_os = "linux")]
use std::path::PathBuf;
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
fn applescript_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
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
fn mac_login_item_name(options: &LaunchAtLoginOptions) -> Result<String> {
    let application = mac_application_path(&options.executable_path)?;
    Path::new(&application)
        .file_stem()
        .and_then(|name| name.to_str())
        .map(ToOwned::to_owned)
        .ok_or_else(|| anyhow!("Unable to determine macOS application bundle name"))
}

#[cfg(target_os = "macos")]
fn run_osascript(script: &str) -> Result<String> {
    let output = Command::new("/usr/bin/osascript")
        .args(["-e", script])
        .output()?;
    if !output.status.success() {
        return Err(anyhow!(
            "osascript failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(target_os = "macos")]
fn platform_get_launch_at_login(options: &LaunchAtLoginOptions) -> Result<LaunchAtLoginStatus> {
    let name = mac_login_item_name(options)?;
    let output =
        run_osascript("tell application \"System Events\" to get the name of every login item")?;
    let enabled = output
        .split(',')
        .map(str::trim)
        .any(|login_item| login_item == name);
    Ok(LaunchAtLoginStatus {
        enabled,
        backend: "macos-login-item".to_string(),
    })
}

#[cfg(target_os = "macos")]
fn platform_set_launch_at_login(options: &LaunchAtLoginOptions, enabled: bool) -> Result<()> {
    let application = applescript_escape(&mac_application_path(&options.executable_path)?);
    let name = applescript_escape(&mac_login_item_name(options)?);
    let script = if enabled {
        format!(
            "tell application \"System Events\"\nrepeat with loginItem in login items\nif name of loginItem is \"{name}\" then delete loginItem\nend repeat\nmake login item at end with properties {{path:\"{application}\", hidden:false}}\nend tell"
        )
    } else {
        format!(
            "tell application \"System Events\"\nrepeat with loginItem in login items\nif name of loginItem is \"{name}\" then delete loginItem\nend repeat\nend tell"
        )
    };
    run_osascript(&script).map(|_| ())
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
fn mac_default_service(device: &str) -> Option<String> {
    let order = command_output("/usr/sbin/networksetup", &["-listnetworkserviceorder"])
        .or_else(|| command_output("networksetup", &["-listnetworkserviceorder"]))?;
    parse_mac_default_service(&order, device)
}

#[cfg(target_os = "macos")]
fn parse_mac_default_service(order: &str, device: &str) -> Option<String> {
    let mut service = None;
    for line in order.lines() {
        let trimmed = line.trim();
        if let Some((index, value)) = trimmed
            .strip_prefix('(')
            .and_then(|value| value.split_once(')'))
            && !index.is_empty()
            && index.chars().all(|character| character.is_ascii_digit())
            && !value.trim().is_empty()
        {
            service = Some(value.trim().to_string());
        }
        if line.contains(&format!("Device: {device}")) {
            return service;
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn platform_default_interface() -> Option<String> {
    command_output("/sbin/route", &["-n", "get", "default"])
        .or_else(|| command_output("route", &["-n", "get", "default"]))?
        .lines()
        .find_map(|line| {
            line.trim()
                .strip_prefix("interface:")
                .map(|value| value.trim().to_string())
        })
}

#[cfg(target_os = "macos")]
fn mac_dns_servers(service: &str) -> Vec<String> {
    command_output("/usr/sbin/networksetup", &["-getdnsservers", service])
        .or_else(|| command_output("networksetup", &["-getdnsservers", service]))
        .filter(|output| !output.starts_with("There aren't any DNS Servers set on"))
        .map(|output| {
            output
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(target_os = "macos")]
fn mac_ssid(service: &str) -> Option<String> {
    command_output("/usr/sbin/networksetup", &["-getairportnetwork", service])
        .or_else(|| command_output("networksetup", &["-getairportnetwork", service]))?
        .split_once(':')
        .map(|(_, ssid)| ssid.trim().to_string())
        .filter(|ssid| !ssid.is_empty() && !ssid.contains("not associated"))
}

#[cfg(target_os = "windows")]
fn windows_ip_configuration() -> (Option<String>, Vec<String>) {
    let script = concat!(
        "$route=Get-NetRoute -DestinationPrefix '0.0.0.0/0' -ErrorAction SilentlyContinue | ",
        "Where-Object {$_.NextHop -ne '0.0.0.0'} | Sort-Object RouteMetric,InterfaceMetric | ",
        "Select-Object -First 1; if ($null -eq $route) {$route=Get-NetRoute ",
        "-DestinationPrefix '::/0' -ErrorAction SilentlyContinue | Sort-Object ",
        "RouteMetric,InterfaceMetric | Select-Object -First 1}; if ($null -ne $route) ",
        "{$config=Get-NetIPConfiguration -InterfaceIndex $route.InterfaceIndex; ",
        "[Console]::OutputEncoding=[Text.Encoding]::UTF8; Write-Output ",
        "('INTERFACE=' + $config.InterfaceAlias); @($config.DNSServer.ServerAddresses) | ",
        "ForEach-Object {Write-Output ('DNS=' + $_)}}"
    );
    let Some(output) = command_output(
        "powershell.exe",
        &[
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            script,
        ],
    ) else {
        return (None, Vec::new());
    };
    let mut default_interface = None;
    let mut dns_servers = Vec::new();
    for line in output.lines().map(str::trim) {
        if let Some(value) = line.strip_prefix("INTERFACE=") {
            if !value.is_empty() {
                default_interface = Some(value.to_string());
            }
        } else if let Some(value) = line.strip_prefix("DNS=")
            && value.parse::<std::net::IpAddr>().is_ok()
        {
            dns_servers.push(value.to_string());
        }
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
    let default_interface = platform_default_interface();
    let default_service = default_interface.as_deref().and_then(mac_default_service);
    NetworkContext {
        dns_servers: default_service
            .as_deref()
            .map(mac_dns_servers)
            .unwrap_or_default(),
        ssid: default_service.as_deref().and_then(mac_ssid),
        default_interface,
        default_service,
    }
}

#[cfg(all(test, target_os = "macos"))]
mod macos_tests {
    use super::parse_mac_default_service;

    #[test]
    fn maps_device_without_treating_hardware_line_as_a_service() {
        let order = "An asterisk (*) denotes that a network service is disabled.\n(1) Wi-Fi\n(Hardware Port: Wi-Fi, Device: en0)\n(2) Thunderbolt Bridge\n(Hardware Port: Thunderbolt Bridge, Device: bridge0)\n";

        assert_eq!(
            parse_mac_default_service(order, "en0").as_deref(),
            Some("Wi-Fi")
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
