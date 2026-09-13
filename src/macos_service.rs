use anyhow::{Result, anyhow};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacOSManagedServiceStatus {
    NotRegistered,
    Enabled,
    RequiresApproval,
    NotFound,
    Unknown,
}

impl MacOSManagedServiceStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotRegistered => "not-registered",
            Self::Enabled => "enabled",
            Self::RequiresApproval => "requires-approval",
            Self::NotFound => "not-found",
            Self::Unknown => "unknown",
        }
    }
}

fn validate_plist_name(plist_name: &str) -> Result<()> {
    if plist_name.is_empty() || plist_name.len() > 255 {
        return Err(anyhow!("invalid macOS managed-service plist name"));
    }
    if !plist_name.ends_with(".plist")
        || plist_name == ".plist"
        || !plist_name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
    {
        return Err(anyhow!(
            "macOS managed-service plist name must be a safe .plist basename"
        ));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn with_service<T>(
    plist_name: &str,
    operation: impl FnOnce(&objc2_service_management::SMAppService) -> Result<T>,
) -> Result<T> {
    use objc2_foundation::NSString;

    validate_plist_name(plist_name)?;
    if !objc2::available!(macos = 13.0) {
        return Err(anyhow!("SMAppService requires macOS 13 or later"));
    }

    objc2::rc::autoreleasepool(|_| {
        let name = NSString::from_str(plist_name);
        // SAFETY: Availability is checked above and the validated name refers
        // to a plist embedded in the calling application's LaunchDaemons dir.
        let service =
            unsafe { objc2_service_management::SMAppService::daemonServiceWithPlistName(&name) };
        operation(&service)
    })
}

#[cfg(target_os = "macos")]
fn status_of(service: &objc2_service_management::SMAppService) -> MacOSManagedServiceStatus {
    use objc2_service_management::SMAppServiceStatus;

    // SAFETY: The service is retained for the duration of this query.
    match unsafe { service.status() } {
        SMAppServiceStatus::NotRegistered => MacOSManagedServiceStatus::NotRegistered,
        SMAppServiceStatus::Enabled => MacOSManagedServiceStatus::Enabled,
        SMAppServiceStatus::RequiresApproval => MacOSManagedServiceStatus::RequiresApproval,
        SMAppServiceStatus::NotFound => MacOSManagedServiceStatus::NotFound,
        _ => MacOSManagedServiceStatus::Unknown,
    }
}

#[cfg(target_os = "macos")]
pub fn get_macos_managed_service_status(plist_name: &str) -> Result<MacOSManagedServiceStatus> {
    with_service(plist_name, |service| Ok(status_of(service)))
}

#[cfg(not(target_os = "macos"))]
pub fn get_macos_managed_service_status(plist_name: &str) -> Result<MacOSManagedServiceStatus> {
    validate_plist_name(plist_name)?;
    Err(anyhow!(
        "UNSUPPORTED_PLATFORM: macOS managed-service operations require macOS"
    ))
}

#[cfg(target_os = "macos")]
pub fn register_macos_managed_service(plist_name: &str) -> Result<MacOSManagedServiceStatus> {
    with_service(plist_name, |service| {
        let current = status_of(service);
        if matches!(
            current,
            MacOSManagedServiceStatus::Enabled | MacOSManagedServiceStatus::RequiresApproval
        ) {
            return Ok(current);
        }

        // SAFETY: The service and its plist are owned by the signed caller.
        match unsafe { service.registerAndReturnError() } {
            Ok(()) => Ok(status_of(service)),
            Err(error) => {
                let status = status_of(service);
                if status == MacOSManagedServiceStatus::RequiresApproval {
                    Ok(status)
                } else {
                    Err(anyhow!("SMAppService register failed: {error}"))
                }
            }
        }
    })
}

#[cfg(not(target_os = "macos"))]
pub fn register_macos_managed_service(plist_name: &str) -> Result<MacOSManagedServiceStatus> {
    get_macos_managed_service_status(plist_name)
}

#[cfg(target_os = "macos")]
pub fn unregister_macos_managed_service(plist_name: &str) -> Result<MacOSManagedServiceStatus> {
    with_service(plist_name, |service| {
        let current = status_of(service);
        if matches!(
            current,
            MacOSManagedServiceStatus::NotRegistered | MacOSManagedServiceStatus::NotFound
        ) {
            return Ok(current);
        }

        // SAFETY: The service and its plist are owned by the signed caller.
        unsafe { service.unregisterAndReturnError() }
            .map_err(|error| anyhow!("SMAppService unregister failed: {error}"))?;
        Ok(status_of(service))
    })
}

#[cfg(not(target_os = "macos"))]
pub fn unregister_macos_managed_service(plist_name: &str) -> Result<MacOSManagedServiceStatus> {
    get_macos_managed_service_status(plist_name)
}

#[cfg(target_os = "macos")]
pub fn reload_macos_managed_service(plist_name: &str) -> Result<MacOSManagedServiceStatus> {
    with_service(plist_name, |service| {
        let current = status_of(service);
        if current == MacOSManagedServiceStatus::RequiresApproval {
            return Ok(current);
        }
        if current == MacOSManagedServiceStatus::Enabled {
            // SAFETY: The service and its plist are owned by the signed caller.
            unsafe { service.unregisterAndReturnError() }
                .map_err(|error| anyhow!("SMAppService reload unregister failed: {error}"))?;
        }

        // SAFETY: The service and its plist are owned by the signed caller.
        match unsafe { service.registerAndReturnError() } {
            Ok(()) => Ok(status_of(service)),
            Err(error) => {
                let status = status_of(service);
                if status == MacOSManagedServiceStatus::RequiresApproval {
                    Ok(status)
                } else {
                    Err(anyhow!("SMAppService reload register failed: {error}"))
                }
            }
        }
    })
}

#[cfg(not(target_os = "macos"))]
pub fn reload_macos_managed_service(plist_name: &str) -> Result<MacOSManagedServiceStatus> {
    get_macos_managed_service_status(plist_name)
}

#[cfg(target_os = "macos")]
pub fn open_macos_login_items_settings() -> Result<()> {
    if !objc2::available!(macos = 13.0) {
        return Err(anyhow!("SMAppService requires macOS 13 or later"));
    }
    // SAFETY: Availability is checked immediately above.
    unsafe { objc2_service_management::SMAppService::openSystemSettingsLoginItems() };
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn open_macos_login_items_settings() -> Result<()> {
    Err(anyhow!(
        "UNSUPPORTED_PLATFORM: macOS Login Items settings require macOS"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_safe_plist_basename() {
        assert!(validate_plist_name("KokoroBoxService.plist").is_ok());
        assert!(validate_plist_name("com.example.service-1.plist").is_ok());
    }

    #[test]
    fn rejects_paths_and_invalid_plist_names() {
        for name in [
            "",
            ".plist",
            "service",
            "../service.plist",
            "folder/service.plist",
            "folder\\service.plist",
            "service name.plist",
        ] {
            assert!(validate_plist_name(name).is_err(), "accepted {name:?}");
        }
    }

    #[test]
    fn exposes_stable_status_names() {
        assert_eq!(
            MacOSManagedServiceStatus::NotRegistered.as_str(),
            "not-registered"
        );
        assert_eq!(MacOSManagedServiceStatus::Enabled.as_str(), "enabled");
        assert_eq!(
            MacOSManagedServiceStatus::RequiresApproval.as_str(),
            "requires-approval"
        );
        assert_eq!(MacOSManagedServiceStatus::NotFound.as_str(), "not-found");
        assert_eq!(MacOSManagedServiceStatus::Unknown.as_str(), "unknown");
    }
}
