use crate::{UwpLoopbackAppCategory, UwpLoopbackPackageType};
use anyhow::{Context, Result, bail};
use std::{
    collections::{HashMap, HashSet},
    ffi::c_void,
    path::{Path, PathBuf},
    ptr, slice,
};
use windows::{
    ApplicationModel::{Package, PackageSignatureKind},
    Management::Deployment::PackageManager,
    Win32::{
        NetworkManagement::WindowsFirewall::{
            INET_FIREWALL_APP_CONTAINER, NetworkIsolationEnumAppContainers,
            NetworkIsolationFreeAppContainers, NetworkIsolationGetAppContainerConfig,
            NetworkIsolationSetAppContainerConfig,
        },
        Security::{GetLengthSid, IsValidSid, PSID, SID_AND_ATTRIBUTES},
        System::{
            Memory::{GetProcessHeap, HEAP_FLAGS, HeapFree},
            WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize},
        },
    },
    core::HSTRING,
};

#[derive(Debug, Clone)]
pub struct UwpLoopbackApp {
    pub id: String,
    pub package_family_name: String,
    pub package_full_name: Option<String>,
    pub display_name: String,
    pub description: Option<String>,
    pub enabled: bool,
    pub category: UwpLoopbackAppCategory,
    pub package_type: Option<UwpLoopbackPackageType>,
    pub framework: bool,
    pub resource_package: bool,
}

#[derive(Debug, Clone)]
struct PackageMetadata {
    package_full_name: String,
    package_family_name: String,
    display_name: Option<String>,
    description: Option<String>,
    category: UwpLoopbackAppCategory,
    package_type: UwpLoopbackPackageType,
    framework: bool,
    resource_package: bool,
}

#[derive(Default)]
struct PackageMetadataIndex {
    by_full_name: HashMap<String, PackageMetadata>,
    by_family_name: HashMap<String, PackageMetadata>,
}

struct WinRtApartment(bool);

impl WinRtApartment {
    fn initialize() -> Self {
        Self(unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.is_ok())
    }
}

impl Drop for WinRtApartment {
    fn drop(&mut self) {
        if self.0 {
            unsafe { RoUninitialize() };
        }
    }
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

fn non_empty(value: String) -> Option<String> {
    (!value.trim().is_empty()).then_some(value)
}

fn is_indirect_resource(value: &str) -> bool {
    value.trim_start().starts_with("@{") || value.trim_start().starts_with("ms-resource:")
}

fn package_type(framework: bool, resource_package: bool, optional: bool) -> UwpLoopbackPackageType {
    if framework {
        UwpLoopbackPackageType::Framework
    } else if resource_package {
        UwpLoopbackPackageType::Resource
    } else if optional {
        UwpLoopbackPackageType::Optional
    } else {
        UwpLoopbackPackageType::Main
    }
}

fn is_path_below(path: &str, parent: &Path) -> bool {
    Path::new(path)
        .components()
        .zip(parent.components())
        .all(|(left, right)| left.as_os_str().eq_ignore_ascii_case(right.as_os_str()))
        && Path::new(path).components().count() >= parent.components().count()
}

fn is_windows_system_path(installed_path: &str) -> bool {
    let Some(windows_directory) = std::env::var_os("WINDIR").map(PathBuf::from) else {
        return false;
    };
    is_path_below(installed_path, &windows_directory.join("SystemApps"))
        || is_path_below(installed_path, &windows_directory.join("SystemResources"))
}

fn is_microsoft_publisher(publisher: &str, publisher_display_name: &str) -> bool {
    publisher.split(',').any(|part| {
        matches!(
            part.trim().to_ascii_lowercase().as_str(),
            "cn=microsoft corporation" | "cn=microsoft windows"
        )
    }) || publisher_display_name
        .trim()
        .to_ascii_lowercase()
        .starts_with("microsoft")
}

fn classify_package(
    signature_kind: Option<PackageSignatureKind>,
    framework: bool,
    resource_package: bool,
    installed_path: &str,
    publisher: &str,
    publisher_display_name: &str,
) -> UwpLoopbackAppCategory {
    if signature_kind == Some(PackageSignatureKind::System)
        || framework
        || resource_package
        || is_windows_system_path(installed_path)
    {
        UwpLoopbackAppCategory::System
    } else if is_microsoft_publisher(publisher, publisher_display_name) {
        UwpLoopbackAppCategory::Microsoft
    } else {
        UwpLoopbackAppCategory::User
    }
}

fn metadata_for_package(package: &Package) -> Option<PackageMetadata> {
    let id = package.Id().ok()?;
    let package_full_name = id.FullName().ok()?.to_string();
    let package_family_name = id.FamilyName().ok()?.to_string();
    let framework = package.IsFramework().unwrap_or(false);
    let resource_package = package.IsResourcePackage().unwrap_or(false);
    let optional = package.IsOptional().unwrap_or(false);
    let publisher = id.Publisher().map(|value| value.to_string()).unwrap_or_default();
    let publisher_display_name = package
        .PublisherDisplayName()
        .map(|value| value.to_string())
        .unwrap_or_default();
    let installed_path = package
        .InstalledPath()
        .map(|value| value.to_string())
        .unwrap_or_default();
    Some(PackageMetadata {
        package_full_name,
        package_family_name,
        display_name: package
            .DisplayName()
            .ok()
            .and_then(|value| non_empty(value.to_string())),
        description: package
            .Description()
            .ok()
            .and_then(|value| non_empty(value.to_string())),
        category: classify_package(
            package.SignatureKind().ok(),
            framework,
            resource_package,
            &installed_path,
            &publisher,
            &publisher_display_name,
        ),
        package_type: package_type(framework, resource_package, optional),
        framework,
        resource_package,
    })
}

fn package_metadata_index() -> PackageMetadataIndex {
    let _apartment = WinRtApartment::initialize();
    let result = (|| -> windows::core::Result<PackageMetadataIndex> {
        let manager = PackageManager::new()?;
        let packages = manager.FindPackagesByUserSecurityId(&HSTRING::new())?;
        let mut index = PackageMetadataIndex::default();
        for package in packages {
            let Some(metadata) = package.ok().and_then(|value| metadata_for_package(&value)) else {
                continue;
            };
            index.by_full_name.insert(
                metadata.package_full_name.to_ascii_lowercase(),
                metadata.clone(),
            );
            let family_key = metadata.package_family_name.to_ascii_lowercase();
            let replace_family = index
                .by_family_name
                .get(&family_key)
                .is_none_or(|existing| {
                    existing.package_type != UwpLoopbackPackageType::Main
                        && metadata.package_type == UwpLoopbackPackageType::Main
                });
            if replace_family {
                index.by_family_name.insert(family_key, metadata);
            }
        }
        Ok(index)
    })();
    result.unwrap_or_default()
}

fn fallback_category(package_family_name: &str, working_directory: &str) -> UwpLoopbackAppCategory {
    if is_windows_system_path(working_directory) {
        UwpLoopbackAppCategory::System
    } else if package_family_name
        .trim()
        .to_ascii_lowercase()
        .starts_with("microsoft.")
    {
        UwpLoopbackAppCategory::Microsoft
    } else {
        UwpLoopbackAppCategory::User
    }
}

pub fn list_uwp_loopback_apps() -> Result<Vec<UwpLoopbackApp>> {
    let containers = AppContainers::get()?;
    let config = Config::get()?;
    let allowed: HashSet<_> = config
        .entries()
        .iter()
        .filter_map(|entry| sid_key(entry.Sid))
        .collect();
    let mut apps = Vec::new();
    let metadata_index = package_metadata_index();
    let mut seen = HashSet::new();
    for container in containers.entries() {
        let Some(id) = sid_key(PSID(container.appContainerSid.cast())) else {
            continue;
        };
        if !seen.insert(id.clone()) {
            continue;
        }
        let package_family_name = wide_string(container.appContainerName);
        if package_family_name.is_empty() {
            continue;
        }
        let package_full_name = non_empty(wide_string(container.packageFullName));
        let metadata = package_full_name
            .as_ref()
            .and_then(|name| metadata_index.by_full_name.get(&name.to_ascii_lowercase()))
            .or_else(|| {
                metadata_index
                    .by_family_name
                    .get(&package_family_name.to_ascii_lowercase())
            });
        let container_display_name = non_empty(wide_string(container.displayName));
        let container_description = non_empty(wide_string(container.description));
        let working_directory = wide_string(container.workingDirectory);
        let fallback_category = fallback_category(&package_family_name, &working_directory);
        let display_name = container_display_name
            .as_ref()
            .filter(|value| !is_indirect_resource(value))
            .cloned()
            .or_else(|| metadata.and_then(|value| value.display_name.clone()))
            .or(container_display_name)
            .unwrap_or_else(|| package_family_name.clone());
        apps.push(UwpLoopbackApp {
            enabled: allowed.contains(&id),
            id,
            package_family_name,
            package_full_name: package_full_name
                .or_else(|| metadata.map(|value| value.package_full_name.clone())),
            display_name,
            description: container_description
                .filter(|value| !is_indirect_resource(value))
                .or_else(|| metadata.and_then(|value| value.description.clone())),
            category: metadata.map_or(fallback_category, |value| value.category),
            package_type: metadata.map(|value| value.package_type),
            framework: metadata.is_some_and(|value| value.framework),
            resource_package: metadata.is_some_and(|value| value.resource_package),
        });
    }
    apps.sort_by(|a, b| {
        a.display_name
            .to_lowercase()
            .cmp(&b.display_name.to_lowercase())
    });
    Ok(apps)
}

pub fn set_uwp_loopback_exemption(id: &str, enabled: bool) -> Result<()> {
    let containers = AppContainers::get()?;
    let target = containers
        .entries()
        .iter()
        .find(|container| {
            sid_key(PSID(container.appContainerSid.cast())).as_deref() == Some(id)
        })
        .context("UWP app container is no longer installed")?;
    let config = Config::get()?;
    let mut entries = config.entries().to_vec();
    let existing = entries
        .iter()
        .any(|entry| sid_key(entry.Sid).as_deref() == Some(id));
    match (existing, enabled) {
        (false, true) => entries.push(SID_AND_ATTRIBUTES {
            Sid: PSID(target.appContainerSid.cast()),
            Attributes: 0,
        }),
        (true, false) => {
            entries.retain(|entry| sid_key(entry.Sid).as_deref() != Some(id));
        }
        _ => return Ok(()),
    }
    let code = unsafe { NetworkIsolationSetAppContainerConfig(&entries) };
    if code != 0 {
        bail!("NetworkIsolationSetAppContainerConfig failed: {code}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn framework_and_resource_packages_are_system_components() {
        assert_eq!(
            classify_package(None, true, false, "", "", ""),
            UwpLoopbackAppCategory::System
        );
        assert_eq!(
            classify_package(None, false, true, "", "", ""),
            UwpLoopbackAppCategory::System
        );
    }

    #[test]
    fn microsoft_publisher_is_not_confused_with_system_signature() {
        assert_eq!(
            classify_package(
                Some(PackageSignatureKind::Store),
                false,
                false,
                r"C:\Program Files\WindowsApps\Microsoft.GamingApp",
                "CN=Microsoft Corporation, O=Microsoft Corporation, L=Redmond, S=Washington, C=US",
                "Microsoft Corporation",
            ),
            UwpLoopbackAppCategory::Microsoft
        );
    }

    #[test]
    fn package_type_uses_the_most_specific_package_flag() {
        assert_eq!(
            package_type(true, true, true),
            UwpLoopbackPackageType::Framework
        );
        assert_eq!(
            package_type(false, true, true),
            UwpLoopbackPackageType::Resource
        );
        assert_eq!(
            package_type(false, false, true),
            UwpLoopbackPackageType::Optional
        );
        assert_eq!(
            package_type(false, false, false),
            UwpLoopbackPackageType::Main
        );
    }

    #[test]
    fn recognizes_indirect_manifest_resources() {
        assert!(is_indirect_resource("@{Microsoft.App_1.0?ms-resource://AppName}"));
        assert!(is_indirect_resource("ms-resource:AppName"));
        assert!(!is_indirect_resource("Microsoft Store"));
    }
}
