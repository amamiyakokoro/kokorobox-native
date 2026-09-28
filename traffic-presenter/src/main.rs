#![deny(clippy::all)]
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod platform;
#[path = "../../src/stderr_log.rs"]
mod stderr_log;

fn main() {
    if let Err(error) = platform::run() {
        stderr_log::write("error", "traffic-presenter", format_args!("{error}"));
        std::process::exit(1);
    }
}
