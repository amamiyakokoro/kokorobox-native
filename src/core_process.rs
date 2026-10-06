//! Narrow process boundary for Desktop-owned Mihomo cores. Never signal a PID
//! until the executable and creation identity have been verified.
use anyhow::{Context, Result, bail};
use std::path::PathBuf;
#[cfg(target_os = "macos")]
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreProcessIdentity {
    pub pid: u32,
    pub executable: String,
    pub started: String,
}

pub(crate) fn core_path(path: &str) -> Result<PathBuf> {
    if !std::path::Path::new(path).is_absolute() {
        bail!("core executable path must be absolute");
    }
    let path = std::fs::canonicalize(path).context("resolve core executable")?;
    let name = path
        .file_name()
        .and_then(|v| v.to_str())
        .unwrap_or_default();
    if !matches!(
        name.to_ascii_lowercase().as_str(),
        "mihomo" | "mihomo-alpha" | "mihomo.exe" | "mihomo-alpha.exe"
    ) {
        bail!("process executable must be Mihomo");
    }
    Ok(path)
}

pub fn inspect_core_process(pid: u32, executable: &str) -> Result<Option<CoreProcessIdentity>> {
    if pid == 0 || pid > i32::MAX as u32 || pid == std::process::id() {
        bail!("invalid core process id");
    }
    let expected = core_path(executable)?;
    let Some((actual, started)) = platform::inspect(pid)? else {
        return Ok(None);
    };
    if std::fs::canonicalize(actual).ok().as_deref() != Some(expected.as_path()) {
        return Ok(None);
    }
    Ok(Some(CoreProcessIdentity {
        pid,
        executable: expected.to_string_lossy().into_owned(),
        started,
    }))
}

pub fn stop_core_process(identity: &CoreProcessIdentity) -> Result<bool> {
    if identity.started.is_empty() {
        bail!("core process creation identity is required");
    }
    if inspect_core_process(identity.pid, &identity.executable)?.as_ref() != Some(identity) {
        return Ok(false);
    }
    platform::stop(identity)
}

#[cfg(target_os = "macos")]
mod platform {
    use super::*;
    pub fn inspect(pid: u32) -> Result<Option<(PathBuf, String)>> {
        // SAFETY: libproc writes into correctly sized, initialized buffers.
        let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
        let size = std::mem::size_of_val(&info) as i32;
        let result = unsafe {
            libc::proc_pidinfo(
                pid as i32,
                libc::PROC_PIDTBSDINFO,
                0,
                &mut info as *mut _ as *mut _,
                size,
            )
        };
        if result != size {
            let error = std::io::Error::last_os_error();
            if matches!(error.raw_os_error(), Some(libc::ESRCH | libc::ENOENT)) {
                return Ok(None);
            }
            return Err(error.into());
        }
        if info.pbi_status == libc::SZOMB {
            return Ok(None);
        }
        // Compare real UID: a setuid Mihomo may legitimately have effective UID 0.
        if info.pbi_ruid != unsafe { libc::getuid() } {
            return Ok(None);
        }
        let mut buffer = [0u8; 4096];
        let length = unsafe {
            libc::proc_pidpath(pid as i32, buffer.as_mut_ptr().cast(), buffer.len() as u32)
        };
        if length <= 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        let end = buffer.iter().position(|v| *v == 0).unwrap_or(buffer.len());
        Ok(Some((
            PathBuf::from(std::str::from_utf8(&buffer[..end])?),
            format!("{}:{}", info.pbi_start_tvsec, info.pbi_start_tvusec),
        )))
    }
    pub fn stop(identity: &CoreProcessIdentity) -> Result<bool> {
        // macOS offers no pidfd. Recheck path, real UID and microsecond creation
        // time before EACH signal; never escalate against a reused PID.
        for (signal, wait) in [(libc::SIGINT, 3), (libc::SIGTERM, 3), (libc::SIGKILL, 2)] {
            if inspect_core_process(identity.pid, &identity.executable)?.as_ref() != Some(identity)
            {
                return Ok(true);
            }
            if unsafe { libc::kill(identity.pid as i32, signal) } != 0 {
                let error = std::io::Error::last_os_error();
                if error.raw_os_error() == Some(libc::ESRCH) {
                    return Ok(true);
                }
                return Err(error.into());
            }
            let deadline = Instant::now() + Duration::from_secs(wait);
            while Instant::now() < deadline {
                if inspect_core_process(identity.pid, &identity.executable)?.as_ref()
                    != Some(identity)
                {
                    return Ok(true);
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
        bail!("core process did not exit before deadline")
    }
}

#[cfg(any(target_os = "linux", test))]
fn process_real_uid(status: &str) -> Result<u32> {
    status
        .lines()
        .find_map(|line| line.strip_prefix("Uid:"))
        .and_then(|value| value.split_whitespace().next())
        .and_then(|value| value.parse().ok())
        .context("missing real process UID")
}

#[cfg(target_os = "linux")]
mod platform {
    use super::*;
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    pub fn inspect(pid: u32) -> Result<Option<(PathBuf, String)>> {
        let root = PathBuf::from(format!("/proc/{pid}"));
        let status = match std::fs::read_to_string(root.join("status")) {
            Ok(v) => v,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        if process_real_uid(&status)? != unsafe { libc::getuid() } {
            return Ok(None);
        }
        let path = match std::fs::read_link(root.join("exe")) {
            Ok(v) => v,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        let stat = std::fs::read_to_string(root.join("stat"))?;
        let tail = stat.rsplit_once(')').context("invalid process stat")?.1;
        let started = tail
            .split_whitespace()
            .nth(19)
            .context("missing creation time")?;
        let boot = std::fs::read_to_string("/proc/sys/kernel/random/boot_id")?;
        Ok(Some((path, format!("{}:{}", boot.trim(), started))))
    }
    pub fn stop(identity: &CoreProcessIdentity) -> Result<bool> {
        // Bind to a kernel process handle before revalidating the identity.
        // Old kernels fail closed rather than reverting to PID-only signals.
        let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, identity.pid, 0) } as i32;
        if fd < 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        let fd = unsafe { OwnedFd::from_raw_fd(fd) };
        if inspect_core_process(identity.pid, &identity.executable)?.as_ref() != Some(identity) {
            return Ok(false);
        }
        for (signal, wait) in [
            (libc::SIGINT, 3000),
            (libc::SIGTERM, 3000),
            (libc::SIGKILL, 2000),
        ] {
            let result = unsafe {
                libc::syscall(
                    libc::SYS_pidfd_send_signal,
                    fd.as_raw_fd(),
                    signal,
                    std::ptr::null::<libc::siginfo_t>(),
                    0,
                )
            };
            if result != 0 {
                let error = std::io::Error::last_os_error();
                if error.raw_os_error() == Some(libc::ESRCH) {
                    return Ok(true);
                }
                return Err(error.into());
            }
            let mut poll = libc::pollfd {
                fd: fd.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            let result = unsafe { libc::poll(&mut poll, 1, wait) };
            if result > 0 {
                return Ok(true);
            }
            if result < 0 {
                return Err(std::io::Error::last_os_error().into());
            }
        }
        bail!("core process did not exit before deadline")
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use super::*;
    use windows::Win32::{
        Foundation::{CloseHandle, FILETIME, HANDLE, WAIT_OBJECT_0},
        System::Threading::{
            GetProcessTimes, OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
            PROCESS_SYNCHRONIZE, PROCESS_TERMINATE, QueryFullProcessImageNameW, TerminateProcess,
            WaitForSingleObject,
        },
    };
    use windows::core::PWSTR;
    struct Process(HANDLE);
    impl Drop for Process {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
    fn open(pid: u32, terminate: bool) -> Result<Process> {
        let access = PROCESS_QUERY_LIMITED_INFORMATION
            | PROCESS_SYNCHRONIZE
            | if terminate {
                PROCESS_TERMINATE
            } else {
                Default::default()
            };
        Ok(Process(unsafe { OpenProcess(access, false, pid) }?))
    }
    fn identity(process: &Process) -> Result<(PathBuf, String)> {
        let mut buffer = [0u16; 32768];
        let mut size = buffer.len() as u32;
        unsafe {
            QueryFullProcessImageNameW(
                process.0,
                PROCESS_NAME_WIN32,
                PWSTR(buffer.as_mut_ptr()),
                &mut size,
            )
        }?;
        let mut created = FILETIME::default();
        let mut exited = FILETIME::default();
        let mut kernel = FILETIME::default();
        let mut user = FILETIME::default();
        unsafe { GetProcessTimes(process.0, &mut created, &mut exited, &mut kernel, &mut user) }?;
        Ok((
            PathBuf::from(String::from_utf16(&buffer[..size as usize])?),
            format!("{}:{}", created.dwHighDateTime, created.dwLowDateTime),
        ))
    }
    pub fn inspect(pid: u32) -> Result<Option<(PathBuf, String)>> {
        match open(pid, false) {
            Ok(process) => {
                if unsafe { WaitForSingleObject(process.0, 0) } == WAIT_OBJECT_0 {
                    return Ok(None);
                }
                if !crate::windows::process_owned_by_current_user(process.0)? {
                    return Ok(None);
                }
                Ok(Some(identity(&process)?))
            }
            Err(error) => {
                if error
                    .downcast_ref::<windows::core::Error>()
                    .is_some_and(|e| e.code().0 as u32 == 0x80070057)
                {
                    return Ok(None);
                }
                Err(error)
            }
        }
    }
    pub fn stop(expected: &CoreProcessIdentity) -> Result<bool> {
        let process = open(expected.pid, true)?;
        if !crate::windows::process_owned_by_current_user(process.0)? {
            return Ok(false);
        }
        let (path, started) = identity(&process)?;
        if std::fs::canonicalize(path)? != std::path::Path::new(&expected.executable)
            || started != expected.started
        {
            return Ok(false);
        }
        // Windows has no POSIX grace signals. Terminate the verified handle.
        unsafe { TerminateProcess(process.0, 0) }?;
        if unsafe { WaitForSingleObject(process.0, 8000) } != WAIT_OBJECT_0 {
            bail!("core process did not exit before deadline");
        }
        Ok(true)
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
mod platform {
    use super::*;
    pub fn inspect(_: u32) -> Result<Option<(PathBuf, String)>> {
        bail!("unsupported platform")
    }
    pub fn stop(_: &CoreProcessIdentity) -> Result<bool> {
        bail!("unsupported platform")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_current_process_and_non_core_executables() {
        assert!(inspect_core_process(std::process::id(), "/tmp/mihomo").is_err());
        assert!(inspect_core_process(0, "/tmp/mihomo").is_err());
        assert!(core_path(std::env::current_exe().unwrap().to_str().unwrap()).is_err());
    }
    #[test]
    fn missing_creation_identity_cannot_signal_a_process() {
        assert!(
            stop_core_process(&CoreProcessIdentity {
                pid: 123,
                executable: "/tmp/mihomo".into(),
                started: String::new()
            })
            .is_err()
        );
    }
    #[cfg(unix)]
    #[test]
    fn verifies_real_child_path_and_creation_before_stopping() {
        let root = std::env::temp_dir().join(format!("kokoro-core-test-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("mihomo");
        std::fs::copy("/bin/sleep", &path).unwrap();
        let mut child = std::process::Command::new(&path).arg("30").spawn().unwrap();
        let result = (|| -> Result<()> {
            let identity = inspect_core_process(child.id(), path.to_str().unwrap())?
                .context("missing child identity")?;
            let other = root.join("mihomo-alpha");
            std::fs::copy("/bin/sleep", &other)?;
            assert!(inspect_core_process(child.id(), other.to_str().unwrap())?.is_none());
            let mut stale = identity.clone();
            stale.started.push_str("-stale");
            assert!(!stop_core_process(&stale)?);
            assert!(child.try_wait()?.is_none());
            assert!(stop_core_process(&identity)?);
            child.wait()?;
            Ok(())
        })();
        let _ = child.kill();
        let _ = child.wait();
        let _ = std::fs::remove_dir_all(root);
        result.unwrap();
    }
    #[test]
    fn process_ownership_uses_real_uid_for_setuid_cores() {
        assert_eq!(
            process_real_uid("Name: mihomo\nUid:\t1000\t0\t0\t0\n").unwrap(),
            1000
        );
        assert!(process_real_uid("Uid: invalid").is_err());
    }
}
