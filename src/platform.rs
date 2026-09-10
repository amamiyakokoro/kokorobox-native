use std::{path::Path, process::Command};

#[cfg(target_os = "linux")]
use std::path::PathBuf;
#[cfg(any(target_os = "linux", target_os = "windows"))]
use std::{env, fs};

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
    Ok(NetworkContext {
        default_interface: platform_default_interface(),
        dns_servers: platform_dns_servers(),
        ssid: platform_ssid(),
    })
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
fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(target_os = "windows")]
fn windows_task_xml(options: &LaunchAtLoginOptions) -> String {
    let arguments = options
        .arguments
        .iter()
        .map(|argument| format!("\"{}\"", argument.replace('"', "\\\"")))
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-16\"?>\n<Task version=\"1.2\" xmlns=\"http://schemas.microsoft.com/windows/2004/02/mit/task\">\n  <Triggers><LogonTrigger><Enabled>true</Enabled><Delay>PT3S</Delay></LogonTrigger></Triggers>\n  <Principals><Principal id=\"Author\"><LogonType>InteractiveToken</LogonType><RunLevel>HighestAvailable</RunLevel></Principal></Principals>\n  <Settings><MultipleInstancesPolicy>Parallel</MultipleInstancesPolicy><DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries><StopIfGoingOnBatteries>false</StopIfGoingOnBatteries><AllowHardTerminate>false</AllowHardTerminate><StartWhenAvailable>false</StartWhenAvailable><RunOnlyIfNetworkAvailable>false</RunOnlyIfNetworkAvailable><AllowStartOnDemand>true</AllowStartOnDemand><Enabled>true</Enabled><Hidden>false</Hidden><RunOnlyIfIdle>false</RunOnlyIfIdle><WakeToRun>false</WakeToRun><ExecutionTimeLimit>PT0S</ExecutionTimeLimit><Priority>3</Priority></Settings>\n  <Actions Context=\"Author\"><Exec><Command>{}</Command>{}</Exec></Actions>\n</Task>\n",
        xml_escape(&options.executable_path),
        if arguments.is_empty() {
            String::new()
        } else {
            format!("<Arguments>{}</Arguments>", xml_escape(&arguments))
        }
    )
}

#[cfg(target_os = "windows")]
fn windows_task_exists(identifier: &str) -> bool {
    Command::new("schtasks.exe")
        .args(["/query", "/tn", identifier])
        .status()
        .is_ok_and(|status| status.success())
}

#[cfg(target_os = "windows")]
fn platform_get_launch_at_login(options: &LaunchAtLoginOptions) -> Result<LaunchAtLoginStatus> {
    Ok(LaunchAtLoginStatus {
        enabled: windows_task_exists(&options.identifier),
        backend: "windows-task-scheduler".to_string(),
    })
}

#[cfg(target_os = "windows")]
fn platform_set_launch_at_login(options: &LaunchAtLoginOptions, enabled: bool) -> Result<()> {
    if !enabled {
        if !windows_task_exists(&options.identifier) {
            return Ok(());
        }
        let exit_code = crate::run_elevated(
            "schtasks.exe",
            &[
                "/delete".to_string(),
                "/tn".to_string(),
                options.identifier.clone(),
                "/f".to_string(),
            ],
        )?;
        if exit_code != 0 {
            return Err(anyhow!(
                "schtasks.exe failed to delete launch task ({exit_code})"
            ));
        }
        return Ok(());
    }

    let task_path =
        env::temp_dir().join(format!("{}-{}.xml", options.identifier, std::process::id()));
    let utf16 = windows_task_xml(options)
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    let mut content = vec![0xff, 0xfe];
    content.extend(utf16);
    fs::write(&task_path, content)?;
    let arguments = vec![
        "/create".to_string(),
        "/tn".to_string(),
        options.identifier.clone(),
        "/xml".to_string(),
        task_path.to_string_lossy().into_owned(),
        "/f".to_string(),
    ];
    let result = crate::run_elevated("schtasks.exe", &arguments);
    let _ = fs::remove_file(task_path);
    let exit_code = result?;
    if exit_code != 0 {
        return Err(anyhow!(
            "schtasks.exe failed to create launch task ({exit_code})"
        ));
    }
    Ok(())
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
fn platform_dns_servers() -> Vec<String> {
    fs::read_to_string("/etc/resolv.conf")
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
        .unwrap_or_default()
}

#[cfg(target_os = "linux")]
fn platform_ssid() -> Option<String> {
    command_output("/usr/sbin/iwgetid", &["--raw"])
        .or_else(|| command_output("iwgetid", &["--raw"]))
        .map(|ssid| ssid.trim().to_string())
        .filter(|ssid| !ssid.is_empty())
}

#[cfg(target_os = "macos")]
fn mac_default_service(device: &str) -> Option<String> {
    let order = command_output("/usr/sbin/networksetup", &["-listnetworkserviceorder"])
        .or_else(|| command_output("networksetup", &["-listnetworkserviceorder"]))?;
    let mut service = None;
    for line in order.lines() {
        if line.trim_start().starts_with('(') {
            service = line
                .trim()
                .split_once(')')
                .map(|(_, value)| value.trim().to_string());
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
fn platform_dns_servers() -> Vec<String> {
    let Some(device) = platform_default_interface() else {
        return Vec::new();
    };
    let Some(service) = mac_default_service(&device) else {
        return Vec::new();
    };
    command_output("/usr/sbin/networksetup", &["-getdnsservers", &service])
        .or_else(|| command_output("networksetup", &["-getdnsservers", &service]))
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
fn platform_ssid() -> Option<String> {
    let device = platform_default_interface()?;
    let service = mac_default_service(&device)?;
    command_output("/usr/sbin/networksetup", &["-getairportnetwork", &service])
        .or_else(|| command_output("networksetup", &["-getairportnetwork", &service]))?
        .split_once(':')
        .map(|(_, ssid)| ssid.trim().to_string())
        .filter(|ssid| !ssid.is_empty() && !ssid.contains("not associated"))
}

#[cfg(target_os = "windows")]
fn platform_default_interface() -> Option<String> {
    None
}

#[cfg(target_os = "windows")]
fn platform_dns_servers() -> Vec<String> {
    Vec::new()
}

#[cfg(target_os = "windows")]
fn platform_ssid() -> Option<String> {
    command_output("netsh.exe", &["wlan", "show", "interfaces"])?
        .lines()
        .find_map(|line| {
            let line = line.trim();
            (!line.starts_with("BSSID") && line.starts_with("SSID"))
                .then(|| {
                    line.split_once(':')
                        .map(|(_, value)| value.trim().to_string())
                })
                .flatten()
        })
        .filter(|ssid| !ssid.is_empty())
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
fn platform_default_interface() -> Option<String> {
    None
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn platform_dns_servers() -> Vec<String> {
    Vec::new()
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn platform_ssid() -> Option<String> {
    None
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
