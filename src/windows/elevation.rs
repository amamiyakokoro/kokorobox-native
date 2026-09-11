use std::mem::size_of;
use std::ptr::addr_of;

use anyhow::{Result, anyhow};
use windows::Win32::Foundation::{ERROR_CANCELLED, HANDLE};
use windows::Win32::System::Threading::{
    CREATE_UNICODE_ENVIRONMENT, CreateProcessW, DeleteProcThreadAttributeList,
    EXTENDED_STARTUPINFO_PRESENT, GetExitCodeProcess, INFINITE, InitializeProcThreadAttributeList,
    LPPROC_THREAD_ATTRIBUTE_LIST, OpenProcess, PROC_THREAD_ATTRIBUTE_PARENT_PROCESS,
    PROCESS_CREATE_PROCESS, PROCESS_INFORMATION, STARTUPINFOEXW, UpdateProcThreadAttribute,
    WaitForSingleObject,
};
use windows::Win32::UI::Shell::{
    SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetShellWindow, GetWindowThreadProcessId, SW_HIDE, SW_SHOWNORMAL,
};
use windows::core::{PCWSTR, PWSTR};

use super::handle::Handle;

const HRESULT_FROM_WIN32_ERROR_CANCELLED: i32 = 0x800704C7u32 as i32;

fn quote_windows_arg(arg: &str) -> String {
    if !arg.is_empty()
        && !arg
            .chars()
            .any(|ch| matches!(ch, ' ' | '\t' | '\n' | '\r' | '"'))
    {
        return arg.to_string();
    }

    let mut quoted = String::from("\"");
    let mut backslashes = 0usize;
    for ch in arg.chars() {
        match ch {
            '\\' => backslashes += 1,
            '"' => {
                quoted.push_str(&"\\".repeat(backslashes * 2 + 1));
                quoted.push('"');
                backslashes = 0;
            }
            _ => {
                quoted.push_str(&"\\".repeat(backslashes));
                backslashes = 0;
                quoted.push(ch);
            }
        }
    }
    quoted.push_str(&"\\".repeat(backslashes * 2));
    quoted.push('"');
    quoted
}

fn to_wide_null(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn shell_execute_process_handle(info: &SHELLEXECUTEINFOW) -> HANDLE {
    unsafe { addr_of!((*info).hProcess).read_unaligned() }
}

fn shell_execute_and_exit_code(info: &mut SHELLEXECUTEINFOW) -> Result<u32> {
    unsafe {
        ShellExecuteExW(info).map_err(|error| {
            if error.code().0 == HRESULT_FROM_WIN32_ERROR_CANCELLED
                || error.code().0 == ERROR_CANCELLED.to_hresult().0
            {
                anyhow!("User canceled")
            } else {
                anyhow!("ShellExecuteExW failed: {error}")
            }
        })?;
        let process_handle = shell_execute_process_handle(info);
        if process_handle.is_invalid() {
            return Ok(0);
        }

        let process = Handle(process_handle);
        let mut exit_code = 0u32;
        WaitForSingleObject(process.0, INFINITE);
        GetExitCodeProcess(process.0, &mut exit_code)
            .map_err(|error| anyhow!("GetExitCodeProcess failed: {error}"))?;
        Ok(exit_code)
    }
}

fn shell_execute(info: &mut SHELLEXECUTEINFOW) -> Result<()> {
    unsafe {
        ShellExecuteExW(info).map_err(|error| {
            if error.code().0 == HRESULT_FROM_WIN32_ERROR_CANCELLED
                || error.code().0 == ERROR_CANCELLED.to_hresult().0
            {
                anyhow!("User canceled")
            } else {
                anyhow!("ShellExecuteExW failed: {error}")
            }
        })
    }
}

fn command_line(command: &str, args: &[String]) -> Vec<u16> {
    let mut parts = Vec::with_capacity(args.len() + 1);
    parts.push(quote_windows_arg(command));
    parts.extend(args.iter().map(|arg| quote_windows_arg(arg)));
    to_wide_null(&parts.join(" "))
}

struct ProcThreadAttributeList(LPPROC_THREAD_ATTRIBUTE_LIST);

impl Drop for ProcThreadAttributeList {
    fn drop(&mut self) {
        unsafe {
            DeleteProcThreadAttributeList(self.0);
        }
    }
}

pub fn run_elevated(command: &str, args: &[String]) -> Result<u32> {
    let verb = to_wide_null("runas");
    let file = to_wide_null(command);
    let parameters = args
        .iter()
        .map(|arg| quote_windows_arg(arg))
        .collect::<Vec<_>>()
        .join(" ");
    let parameters = to_wide_null(&parameters);

    let mut info = SHELLEXECUTEINFOW {
        cbSize: size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC,
        lpVerb: PCWSTR(verb.as_ptr()),
        lpFile: PCWSTR(file.as_ptr()),
        lpParameters: PCWSTR(parameters.as_ptr()),
        nShow: SW_HIDE.0,
        ..Default::default()
    };

    shell_execute_and_exit_code(&mut info)
}

pub fn launch_elevated(command: &str, args: &[String]) -> Result<()> {
    let verb = to_wide_null("runas");
    let file = to_wide_null(command);
    let parameters = args
        .iter()
        .map(|arg| quote_windows_arg(arg))
        .collect::<Vec<_>>()
        .join(" ");
    let parameters = to_wide_null(&parameters);

    let mut info = SHELLEXECUTEINFOW {
        cbSize: size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOASYNC,
        lpVerb: PCWSTR(verb.as_ptr()),
        lpFile: PCWSTR(file.as_ptr()),
        lpParameters: PCWSTR(parameters.as_ptr()),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };

    shell_execute(&mut info)
}

pub fn launch_unelevated(command: &str, args: &[String]) -> Result<()> {
    unsafe {
        let shell_window = GetShellWindow();
        if shell_window.0.is_null() {
            return Err(anyhow!("GetShellWindow returned no desktop shell"));
        }

        let mut shell_process_id = 0u32;
        GetWindowThreadProcessId(shell_window, Some(&mut shell_process_id));
        if shell_process_id == 0 {
            return Err(anyhow!(
                "GetWindowThreadProcessId returned no shell process"
            ));
        }

        // Make Explorer the logical parent instead of copying its token. Windows
        // then creates the child in the interactive shell's security context
        // without requiring SE_IMPERSONATE_NAME in this elevated process.
        let shell_process = Handle(
            OpenProcess(PROCESS_CREATE_PROCESS, false, shell_process_id)
                .map_err(|error| anyhow!("OpenProcess for desktop shell failed: {error}"))?,
        );

        let mut attribute_bytes = 0usize;
        let _ = InitializeProcThreadAttributeList(None, 1, None, &mut attribute_bytes);
        if attribute_bytes == 0 {
            return Err(anyhow!("Unable to size process attribute list"));
        }
        let word_size = size_of::<usize>();
        let mut attribute_storage = vec![0usize; attribute_bytes.div_ceil(word_size)];
        let attribute_list = LPPROC_THREAD_ATTRIBUTE_LIST(attribute_storage.as_mut_ptr().cast());
        InitializeProcThreadAttributeList(Some(attribute_list), 1, None, &mut attribute_bytes)
            .map_err(|error| anyhow!("InitializeProcThreadAttributeList failed: {error}"))?;
        let _attribute_list = ProcThreadAttributeList(attribute_list);

        let parent_process = shell_process.0;
        UpdateProcThreadAttribute(
            attribute_list,
            0,
            PROC_THREAD_ATTRIBUTE_PARENT_PROCESS as usize,
            Some(addr_of!(parent_process).cast()),
            size_of::<HANDLE>(),
            None,
            None,
        )
        .map_err(|error| anyhow!("UpdateProcThreadAttribute(parent) failed: {error}"))?;

        let application = to_wide_null(command);
        let mut command_line = command_line(command, args);
        let mut startup = STARTUPINFOEXW::default();
        startup.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
        startup.StartupInfo.wShowWindow = SW_SHOWNORMAL.0 as u16;
        startup.lpAttributeList = attribute_list;
        let mut process_info = PROCESS_INFORMATION::default();
        CreateProcessW(
            PCWSTR(application.as_ptr()),
            Some(PWSTR(command_line.as_mut_ptr())),
            None,
            None,
            false,
            CREATE_UNICODE_ENVIRONMENT | EXTENDED_STARTUPINFO_PRESENT,
            None,
            PCWSTR::null(),
            &startup.StartupInfo,
            &mut process_info,
        )
        .map_err(|error| anyhow!("CreateProcessW from desktop shell failed: {error}"))?;
        let _process = Handle(process_info.hProcess);
        let _thread = Handle(process_info.hThread);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{command_line, to_wide_null};

    #[test]
    fn command_line_quotes_windows_paths_and_arguments() {
        let actual = command_line(
            r"C:\Program Files\KokoroBox\KokoroBox.exe",
            &["--example".to_string(), "value with spaces".to_string()],
        );
        let expected = to_wide_null(
            r#""C:\Program Files\KokoroBox\KokoroBox.exe" --example "value with spaces""#,
        );
        assert_eq!(actual, expected);
    }
}
