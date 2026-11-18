use kokorobox_traffic_presenter::{
    PresenterCommand, PresenterState, PresenterTransition, parse_command,
};
use objc2::MainThreadMarker;
use objc2_app_kit::{NSFont, NSFontWeightRegular, NSTextAlignment};
use std::io::{self, BufRead};
use tao::event::{Event, StartCause};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS};
use tray_icon::{TrayIcon, TrayIconBuilder};

// The longest formatted direction is twelve characters, such as "↑ 100.0 MB/s".
const TRAFFIC_COLUMN_WIDTH: usize = 12;
const STATUS_ITEM_WIDTH: f64 = 204.0;

fn status_item_title(state: &PresenterState) -> String {
    let upload = state.upload_label();
    let download = state.download_label();
    format!("{upload:<TRAFFIC_COLUMN_WIDTH$}  {download:<TRAFFIC_COLUMN_WIDTH$}")
}

fn update_tray(tray: &TrayIcon, state: &PresenterState) -> tray_icon::Result<()> {
    tray.set_title(Some(status_item_title(state)));
    tray.set_tooltip(Some(state.single_line_label()))?;
    tray.set_visible(state.visible)?;

    // tray-icon recreates the native item when it becomes visible again.
    if let Some(item) = tray.ns_status_item()
        && item.length() != STATUS_ITEM_WIDTH
    {
        item.setLength(STATUS_ITEM_WIDTH);
        let main_thread = MainThreadMarker::new().expect("status item must run on the main thread");
        if let Some(button) = item.button(main_thread) {
            let font =
                NSFont::monospacedSystemFontOfSize_weight(12.0, unsafe { NSFontWeightRegular });
            button.setFont(Some(&font));
            button.setAlignment(NSTextAlignment::Left);
        }
    }

    Ok(())
}

enum UserEvent {
    Command(PresenterCommand),
    InputClosed,
    InputError(String),
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    // This process owns only a menu bar status item. Treating it as a regular
    // application gives the headless presenter its own Dock icon on macOS.
    event_loop.set_activation_policy(ActivationPolicy::Accessory);
    let proxy = event_loop.create_proxy();

    std::thread::spawn(move || {
        for line in io::stdin().lock().lines() {
            let event = match line {
                Ok(line) if line.trim().is_empty() => continue,
                Ok(line) => match parse_command(&line) {
                    Ok(command) => UserEvent::Command(command),
                    Err(error) => UserEvent::InputError(error.to_string()),
                },
                Err(error) => UserEvent::InputError(error.to_string()),
            };
            if proxy.send_event(event).is_err() {
                return;
            }
        }
        let _ = proxy.send_event(UserEvent::InputClosed);
    });

    let mut state = PresenterState::default();
    let mut tray: Option<TrayIcon> = None;
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::NewEvents(StartCause::Init) => {
                let result = TrayIconBuilder::new()
                    .with_id("kokorobox-traffic")
                    .with_title(state.single_line_label())
                    .with_tooltip("KokoroBox traffic")
                    .build();
                match result {
                    Ok(new_tray) => {
                        if let Err(error) = new_tray.set_visible(false) {
                            crate::stderr_log::write(
                                "error",
                                "traffic-presenter",
                                format_args!("hide status item: {error}"),
                            );
                            *control_flow = ControlFlow::ExitWithCode(1);
                        } else {
                            tray = Some(new_tray);
                        }
                    }
                    Err(error) => {
                        crate::stderr_log::write(
                            "error",
                            "traffic-presenter",
                            format_args!("create status item: {error}"),
                        );
                        *control_flow = ControlFlow::ExitWithCode(1);
                    }
                }
            }
            Event::UserEvent(UserEvent::Command(command)) => {
                if state.apply(command) == PresenterTransition::Shutdown {
                    *control_flow = ControlFlow::Exit;
                    return;
                }
                if let Some(tray) = &tray
                    && let Err(error) = update_tray(tray, &state)
                {
                    crate::stderr_log::write(
                        "error",
                        "traffic-presenter",
                        format_args!("update status item: {error}"),
                    );
                    *control_flow = ControlFlow::ExitWithCode(1);
                }
            }
            Event::UserEvent(UserEvent::InputClosed) => {
                *control_flow = ControlFlow::Exit;
            }
            Event::UserEvent(UserEvent::InputError(error)) => {
                crate::stderr_log::write(
                    "error",
                    "traffic-presenter",
                    format_args!("read command: {error}"),
                );
                *control_flow = ControlFlow::ExitWithCode(1);
            }
            _ => {}
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use kokorobox_traffic_presenter::TrafficSnapshot;

    #[test]
    fn keeps_both_traffic_columns_aligned_across_samples() {
        for traffic in [
            None,
            Some(TrafficSnapshot { up: 0, down: 999 }),
            Some(TrafficSnapshot {
                up: 1023,
                down: 1024,
            }),
            Some(TrafficSnapshot {
                up: 100 * 1024,
                down: 100 * 1024 * 1024,
            }),
            Some(TrafficSnapshot {
                up: u64::MAX,
                down: u64::MAX,
            }),
        ] {
            let title = status_item_title(&PresenterState {
                traffic,
                ..Default::default()
            });
            assert!(title.starts_with("↑ "));
            assert_eq!(title.chars().position(|ch| ch == '↓'), Some(14));
            assert_eq!(title.chars().count(), 26);
        }
    }
}
