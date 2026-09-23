use anyhow::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowsServiceStatus {
    Running,
    Stopped,
    Paused,
    NotInstalled,
    Unknown,
}

impl WindowsServiceStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Stopped => "stopped",
            Self::Paused => "paused",
            Self::NotInstalled => "not-installed",
            Self::Unknown => "unknown",
        }
    }
}

#[cfg(target_os = "windows")]
pub fn get_windows_service_status() -> Result<WindowsServiceStatus> {
    use std::mem::{MaybeUninit, size_of};

    use anyhow::Context;
    use windows::Win32::Foundation::ERROR_SERVICE_DOES_NOT_EXIST;
    use windows::Win32::System::Services::{
        CloseServiceHandle, OpenSCManagerW, OpenServiceW, QueryServiceStatusEx, SC_HANDLE,
        SC_MANAGER_CONNECT, SC_STATUS_PROCESS_INFO, SERVICE_PAUSE_PENDING, SERVICE_PAUSED,
        SERVICE_QUERY_STATUS, SERVICE_RUNNING, SERVICE_START_PENDING, SERVICE_STATUS_PROCESS,
        SERVICE_STOPPED,
    };
    use windows::core::w;

    struct ServiceHandle(SC_HANDLE);
    impl Drop for ServiceHandle {
        fn drop(&mut self) {
            let _ = unsafe { CloseServiceHandle(self.0) };
        }
    }

    let manager = ServiceHandle(
        unsafe { OpenSCManagerW(None, None, SC_MANAGER_CONNECT) }
            .context("OpenSCManagerW failed")?,
    );
    let service =
        match unsafe { OpenServiceW(manager.0, w!("KokoroBoxService"), SERVICE_QUERY_STATUS) } {
            Ok(handle) => ServiceHandle(handle),
            Err(error) if error.code() == ERROR_SERVICE_DOES_NOT_EXIST.into() => {
                return Ok(WindowsServiceStatus::NotInstalled);
            }
            Err(error) => return Err(error).context("OpenServiceW failed"),
        };

    let mut status = MaybeUninit::<SERVICE_STATUS_PROCESS>::zeroed();
    let mut bytes_needed = 0;
    let bytes = unsafe {
        std::slice::from_raw_parts_mut(
            status.as_mut_ptr().cast::<u8>(),
            size_of::<SERVICE_STATUS_PROCESS>(),
        )
    };
    unsafe {
        QueryServiceStatusEx(
            service.0,
            SC_STATUS_PROCESS_INFO,
            Some(bytes),
            &mut bytes_needed,
        )
    }
    .context("QueryServiceStatusEx failed")?;
    if bytes_needed as usize != size_of::<SERVICE_STATUS_PROCESS>() {
        return Ok(WindowsServiceStatus::Unknown);
    }
    let status = unsafe { status.assume_init() };
    Ok(match status.dwCurrentState {
        SERVICE_RUNNING | SERVICE_START_PENDING => WindowsServiceStatus::Running,
        SERVICE_PAUSED | SERVICE_PAUSE_PENDING => WindowsServiceStatus::Paused,
        SERVICE_STOPPED => WindowsServiceStatus::Stopped,
        _ => WindowsServiceStatus::Unknown,
    })
}

#[cfg(not(target_os = "windows"))]
pub fn get_windows_service_status() -> Result<WindowsServiceStatus> {
    anyhow::bail!("Windows Service status is unavailable on this platform")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_strings_match_desktop_contract() {
        assert_eq!(WindowsServiceStatus::Running.as_str(), "running");
        assert_eq!(WindowsServiceStatus::Stopped.as_str(), "stopped");
        assert_eq!(WindowsServiceStatus::Paused.as_str(), "paused");
        assert_eq!(WindowsServiceStatus::NotInstalled.as_str(), "not-installed");
        assert_eq!(WindowsServiceStatus::Unknown.as_str(), "unknown");
    }
}
