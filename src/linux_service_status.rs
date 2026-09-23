use anyhow::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinuxServiceStatus {
    Running,
    Stopped,
    NotInstalled,
    Unknown,
}

impl LinuxServiceStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Stopped => "stopped",
            Self::NotInstalled => "not-installed",
            Self::Unknown => "unknown",
        }
    }
}

#[cfg(target_os = "linux")]
pub fn get_linux_service_status() -> Result<LinuxServiceStatus> {
    use anyhow::Context;
    use zbus::blocking::{Connection, Proxy};
    use zbus::zvariant::OwnedObjectPath;

    const UNIT: &str = "KokoroBoxService.service";
    const DESTINATION: &str = "org.freedesktop.systemd1";
    const MANAGER_PATH: &str = "/org/freedesktop/systemd1";
    const MANAGER_INTERFACE: &str = "org.freedesktop.systemd1.Manager";

    fn missing_unit(error: &zbus::Error) -> bool {
        matches!(
            error,
            zbus::Error::MethodError(name, _, _)
                if matches!(name.as_str(),
                    "org.freedesktop.systemd1.NoSuchUnit" |
                    "org.freedesktop.systemd1.NoSuchUnitFile")
        )
    }

    let connection = Connection::system().context("Connect to systemd system bus")?;
    let manager = Proxy::new(&connection, DESTINATION, MANAGER_PATH, MANAGER_INTERFACE)
        .context("Connect to systemd manager")?;

    // The service is installed system-wide, not in the user's systemd manager.
    match manager.call::<_, _, String>("GetUnitFileState", &(UNIT,)) {
        Ok(state) if state == "not-found" => return Ok(LinuxServiceStatus::NotInstalled),
        Ok(_) => {}
        Err(error) if missing_unit(&error) => return Ok(LinuxServiceStatus::NotInstalled),
        Err(error) => return Err(error).context("Read KokoroBox Service unit file"),
    }

    let path: OwnedObjectPath = match manager.call("GetUnit", &(UNIT,)) {
        Ok(path) => path,
        Err(error) if missing_unit(&error) => return Ok(LinuxServiceStatus::Stopped),
        Err(error) => return Err(error).context("Find KokoroBox Service unit"),
    };
    let unit = Proxy::new(
        &connection,
        DESTINATION,
        path,
        "org.freedesktop.systemd1.Unit",
    )
    .context("Connect to KokoroBox Service unit")?;
    let active_state: String = unit
        .get_property("ActiveState")
        .context("Read Service state")?;
    Ok(match active_state.as_str() {
        "active" | "reloading" | "activating" => LinuxServiceStatus::Running,
        "inactive" | "deactivating" => LinuxServiceStatus::Stopped,
        _ => LinuxServiceStatus::Unknown,
    })
}

#[cfg(not(target_os = "linux"))]
pub fn get_linux_service_status() -> Result<LinuxServiceStatus> {
    anyhow::bail!("Linux Service status is unavailable on this platform")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_strings_match_desktop_contract() {
        assert_eq!(LinuxServiceStatus::Running.as_str(), "running");
        assert_eq!(LinuxServiceStatus::Stopped.as_str(), "stopped");
        assert_eq!(LinuxServiceStatus::NotInstalled.as_str(), "not-installed");
        assert_eq!(LinuxServiceStatus::Unknown.as_str(), "unknown");
    }
}
