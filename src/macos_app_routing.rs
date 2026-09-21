use anyhow::{Result, anyhow};

const MAXIMUM_REQUEST_BYTES: usize = 256 * 1024;

fn validate_request(request: &str) -> Result<()> {
    if request.is_empty()
        || request.len() > MAXIMUM_REQUEST_BYTES
        || request.as_bytes().contains(&0)
    {
        return Err(anyhow!("macOS application-routing request size is invalid"));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
pub fn invoke_macos_application_routing(request: &str) -> Result<String> {
    use std::ffi::{CStr, CString, c_char};
    use std::ptr;

    unsafe extern "C" {
        fn kokorobox_macos_app_routing_invoke(
            request_json: *const c_char,
            response_json: *mut *mut c_char,
            error_message: *mut *mut c_char,
        ) -> bool;
        fn kokorobox_macos_app_routing_free(value: *mut c_char);
    }

    validate_request(request)?;
    let request = CString::new(request)?;
    let mut response = ptr::null_mut();
    let mut error = ptr::null_mut();
    // SAFETY: The Objective-C bridge copies both output strings and exposes a
    // matching deallocator. The input CString remains alive for the call.
    let ok =
        unsafe { kokorobox_macos_app_routing_invoke(request.as_ptr(), &mut response, &mut error) };
    let read = |value: *mut c_char| -> Option<String> {
        if value.is_null() {
            return None;
        }
        // SAFETY: The bridge returns a NUL-terminated allocation on success.
        let result = unsafe { CStr::from_ptr(value) }
            .to_string_lossy()
            .into_owned();
        // SAFETY: This pointer was allocated by the bridge and is released once.
        unsafe { kokorobox_macos_app_routing_free(value) };
        Some(result)
    };
    let response = read(response);
    let error = read(error);
    if !ok {
        return Err(anyhow!(
            "{}",
            error.unwrap_or_else(|| "macOS application routing failed".to_string())
        ));
    }
    response.ok_or_else(|| anyhow!("macOS application routing returned no response"))
}

#[cfg(not(target_os = "macos"))]
pub fn invoke_macos_application_routing(request: &str) -> Result<String> {
    validate_request(request)?;
    Err(anyhow!(
        "UNSUPPORTED_PLATFORM: macOS application routing requires macOS"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_oversized_and_nul_requests() {
        assert!(validate_request("").is_err());
        assert!(validate_request(&"x".repeat(MAXIMUM_REQUEST_BYTES + 1)).is_err());
        assert!(validate_request("bad\0request").is_err());
        assert!(validate_request(r#"{"version":1,"command":"status"}"#).is_ok());
    }
}
