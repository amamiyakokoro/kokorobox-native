#[path = "icon.rs"]
mod icon;

use kokorobox_traffic_presenter::PresenterState;
use kokorobox_traffic_presenter::runtime::{PresenterBackend, run_command_stream};
use std::io;
use tray_icon::{TrayIcon, TrayIconBuilder};

struct LinuxPresenter {
    tray: TrayIcon,
}

impl LinuxPresenter {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // KSNI exposes the tray title as the tooltip title too. Keep the
        // description empty so shells do not render the traffic label twice.
        let tray = TrayIconBuilder::new()
            .with_id("kokorobox-traffic")
            .with_icon(icon::status_icon()?)
            .with_title("↑ —  ↓ —")
            .build()?;
        tray.set_visible(false)?;
        Ok(Self { tray })
    }
}

impl PresenterBackend for LinuxPresenter {
    type Error = tray_icon::Error;

    fn update(&mut self, state: PresenterState) -> Result<(), Self::Error> {
        let label = state.single_line_label();
        self.tray.set_title(Some(&label));
        self.tray.set_visible(state.visible)
    }

    fn shutdown(&mut self) -> Result<(), Self::Error> {
        self.tray.set_visible(false)
    }
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut presenter = LinuxPresenter::new()?;
    run_command_stream(io::stdin().lock(), &mut presenter)?;
    Ok(())
}
