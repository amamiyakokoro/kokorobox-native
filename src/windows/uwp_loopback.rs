use anyhow::{Context, Result, bail};
use std::{ffi::c_void, ptr, slice};
use windows::Win32::{
    NetworkManagement::WindowsFirewall::{
        INET_FIREWALL_APP_CONTAINER, NetworkIsolationEnumAppContainers,
        NetworkIsolationFreeAppContainers, NetworkIsolationGetAppContainerConfig,
        NetworkIsolationSetAppContainerConfig,
    },
    Security::{GetLengthSid, IsValidSid, PSID, SID_AND_ATTRIBUTES},
    System::Memory::{GetProcessHeap, HEAP_FLAGS, HeapFree},
};

#[derive(Debug, Clone)]
pub struct UwpLoopbackApp {
    pub sid: String,
    pub package_name: String,
    pub display_name: String,
    pub enabled: bool,
}

struct AppContainers(*mut INET_FIREWALL_APP_CONTAINER, u32);

impl AppContainers {
    fn get() -> Result<Self> {
        let mut count = 0;
        let mut entries = ptr::null_mut();
        let code = unsafe { NetworkIsolationEnumAppContainers(0, &mut count, &mut entries) };
        if code != 0 {
            bail!("NetworkIsolationEnumAppContainers failed: {code}");
        }
        if count > 65_536 || (count != 0 && entries.is_null()) {
            if !entries.is_null() {
                unsafe { NetworkIsolationFreeAppContainers(entries) };
            }
            bail!("Windows returned an invalid app-container list");
        }
        Ok(Self(entries, count))
    }

    fn entries(&self) -> &[INET_FIREWALL_APP_CONTAINER] {
        if self.1 == 0 {
            &[]
        } else {
            unsafe { slice::from_raw_parts(self.0, self.1 as usize) }
        }
    }
}

impl Drop for AppContainers {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { NetworkIsolationFreeAppContainers(self.0) };
        }
    }
}

struct Config(*mut SID_AND_ATTRIBUTES, u32);

impl Config {
    fn get() -> Result<Self> {
        let mut count = 0;
        let mut entries = ptr::null_mut();
        let code = unsafe { NetworkIsolationGetAppContainerConfig(&mut count, &mut entries) };
        if code != 0 {
            bail!("NetworkIsolationGetAppContainerConfig failed: {code}");
        }
        if count > 65_536 || (count != 0 && entries.is_null()) {
            let config = Self(entries, count.min(65_536));
            drop(config);
            bail!("Windows returned an invalid loopback configuration");
        }
        Ok(Self(entries, count))
    }

    fn entries(&self) -> &[SID_AND_ATTRIBUTES] {
        if self.1 == 0 {
            &[]
        } else {
            unsafe { slice::from_raw_parts(self.0, self.1 as usize) }
        }
    }
}

impl Drop for Config {
    fn drop(&mut self) {
        if self.0.is_null() {
            return;
        }
        if let Ok(heap) = unsafe { GetProcessHeap() } {
            for entry in self.entries() {
                if !entry.Sid.0.is_null() {
                    let _ = unsafe { HeapFree(heap, HEAP_FLAGS(0), Some(entry.Sid.0)) };
                }
            }
            let _ = unsafe { HeapFree(heap, HEAP_FLAGS(0), Some(self.0.cast::<c_void>())) };
        }
    }
}

fn sid_key(sid: PSID) -> Option<String> {
    if sid.0.is_null() || !unsafe { IsValidSid(sid) }.as_bool() {
        return None;
    }
    let len = unsafe { GetLengthSid(sid) } as usize;
    if !(8..=68).contains(&len) {
        return None;
    }
    Some(
        unsafe { slice::from_raw_parts(sid.0.cast::<u8>(), len) }
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
    )
}

fn wide_string(value: windows::core::PWSTR) -> String {
    if value.is_null() {
        String::new()
    } else {
        unsafe { value.to_string() }.unwrap_or_default()
    }
}

pub fn list_uwp_loopback_apps() -> Result<Vec<UwpLoopbackApp>> {
    let containers = AppContainers::get()?;
    let config = Config::get()?;
    let allowed: std::collections::HashSet<_> = config
        .entries()
        .iter()
        .filter_map(|entry| sid_key(entry.Sid))
        .collect();
    let mut apps = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for container in containers.entries() {
        let Some(sid) = sid_key(PSID(container.appContainerSid.cast())) else {
            continue;
        };
        if !seen.insert(sid.clone()) {
            continue;
        }
        let package_name = wide_string(container.appContainerName);
        if package_name.is_empty() {
            continue;
        }
        let display_name = wide_string(container.displayName);
        apps.push(UwpLoopbackApp {
            enabled: allowed.contains(&sid),
            sid,
            display_name: if display_name.is_empty() {
                package_name.clone()
            } else {
                display_name
            },
            package_name,
        });
    }
    apps.sort_by(|a, b| {
        a.display_name
            .to_lowercase()
            .cmp(&b.display_name.to_lowercase())
    });
    Ok(apps)
}

pub fn set_uwp_loopback_exemption(sid_key_requested: &str, enabled: bool) -> Result<()> {
    let containers = AppContainers::get()?;
    let target = containers
        .entries()
        .iter()
        .find(|container| {
            sid_key(PSID(container.appContainerSid.cast())).as_deref() == Some(sid_key_requested)
        })
        .context("UWP app container is no longer installed")?;
    let config = Config::get()?;
    let mut entries = config.entries().to_vec();
    let existing = entries
        .iter()
        .any(|entry| sid_key(entry.Sid).as_deref() == Some(sid_key_requested));
    match (existing, enabled) {
        (false, true) => entries.push(SID_AND_ATTRIBUTES {
            Sid: PSID(target.appContainerSid.cast()),
            Attributes: 0,
        }),
        (true, false) => {
            entries.retain(|entry| sid_key(entry.Sid).as_deref() != Some(sid_key_requested));
        }
        _ => return Ok(()),
    }
    let code = unsafe { NetworkIsolationSetAppContainerConfig(&entries) };
    if code != 0 {
        bail!("NetworkIsolationSetAppContainerConfig failed: {code}");
    }
    Ok(())
}
