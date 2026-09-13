#![deny(clippy::all)]
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod platform;

fn main() {
    if let Err(error) = platform::run() {
        eprintln!("kokorobox-traffic-presenter: {error}");
        std::process::exit(1);
    }
}
