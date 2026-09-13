#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
pub use linux::run;
#[cfg(target_os = "macos")]
pub use macos::run;
#[cfg(target_os = "windows")]
pub use windows::run;

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    Err("unsupported traffic presenter platform".into())
}
