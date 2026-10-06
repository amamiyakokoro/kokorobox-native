use std::mem::size_of;

use anyhow::{Result, anyhow};
use windows::Win32::Foundation::{HLOCAL, LocalFree};
use windows::Win32::Security::Authorization::ConvertSidToStringSidW;
use windows::Win32::Security::{
    GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TOKEN_USER, TokenElevation, TokenUser,
};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::core::PWSTR;

use super::handle::Handle;

fn open_current_process_token() -> Result<Handle> {
    let mut token = Default::default();
    unsafe {
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token)
            .map_err(|error| anyhow!("OpenProcessToken failed: {error}"))?;
    }
    Ok(Handle(token))
}

fn token_user_buffer(token: &Handle) -> Result<Vec<usize>> {
    let mut needed = 0u32;
    unsafe {
        let _ = GetTokenInformation(token.0, TokenUser, None, 0, &mut needed);
        if needed == 0 {
            return Err(anyhow!(
                "GetTokenInformation(TokenUser) returned an empty buffer"
            ));
        }

        let word_count = (needed as usize).div_ceil(size_of::<usize>());
        let mut buffer = vec![0usize; word_count];
        GetTokenInformation(
            token.0,
            TokenUser,
            Some(buffer.as_mut_ptr() as *mut _),
            needed,
            &mut needed,
        )
        .map_err(|error| anyhow!("GetTokenInformation(TokenUser) failed: {error}"))?;
        Ok(buffer)
    }
}

fn token_user_from_buffer(buffer: &[usize]) -> &TOKEN_USER {
    unsafe { &*(buffer.as_ptr() as *const TOKEN_USER) }
}

fn sid_string_from_token_user(token_user: &TOKEN_USER) -> Result<String> {
    let mut sid_ptr = PWSTR::null();
    unsafe {
        ConvertSidToStringSidW(token_user.User.Sid, &mut sid_ptr)
            .map_err(|error| anyhow!("ConvertSidToStringSidW failed: {error}"))?;
        let sid = sid_ptr
            .to_string()
            .map_err(|error| anyhow!("Convert SID failed: {error}"));
        let _ = LocalFree(Some(HLOCAL(sid_ptr.0 as *mut _)));
        sid
    }
}

pub fn current_user_sid() -> Result<String> {
    let token = open_current_process_token()?;
    let buffer = token_user_buffer(&token)?;
    sid_string_from_token_user(token_user_from_buffer(&buffer))
}

fn token_elevation(token: &Handle) -> Result<TOKEN_ELEVATION> {
    let mut elevation = TOKEN_ELEVATION::default();
    let mut returned = 0u32;
    unsafe {
        GetTokenInformation(
            token.0,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut _),
            size_of::<TOKEN_ELEVATION>() as u32,
            &mut returned,
        )
        .map_err(|error| anyhow!("GetTokenInformation(TokenElevation) failed: {error}"))?;
    }
    Ok(elevation)
}

pub fn is_running_as_admin() -> Result<bool> {
    let token = open_current_process_token()?;
    Ok(token_elevation(&token)?.TokenIsElevated != 0)
}

/// Compare real account identity, including elevated tokens of the same user.
pub(crate) fn process_owned_by_current_user(
    process: windows::Win32::Foundation::HANDLE,
) -> Result<bool> {
    let mut token = Default::default();
    unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) }?;
    let token = Handle(token);
    let buffer = token_user_buffer(&token)?;
    Ok(sid_string_from_token_user(token_user_from_buffer(&buffer))? == current_user_sid()?)
}
