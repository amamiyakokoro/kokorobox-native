use kokorobox_traffic_presenter::{
    PresenterCommand, PresenterState, PresenterTransition, parse_command,
};
use std::io::{self, BufRead};
use tao::event::{Event, StartCause};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tray_icon::{TrayIcon, TrayIconBuilder};

enum UserEvent {
    Command(PresenterCommand),
    InputClosed,
    InputError(String),
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
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
                            eprintln!("kokorobox-traffic-presenter: hide status item: {error}");
                            *control_flow = ControlFlow::ExitWithCode(1);
                        } else {
                            tray = Some(new_tray);
                        }
                    }
                    Err(error) => {
                        eprintln!("kokorobox-traffic-presenter: create status item: {error}");
                        *control_flow = ControlFlow::ExitWithCode(1);
                    }
                }
            }
            Event::UserEvent(UserEvent::Command(command)) => {
                if state.apply(command) == PresenterTransition::Shutdown {
                    *control_flow = ControlFlow::Exit;
                    return;
                }
                if let Some(tray) = &tray {
                    let label = state.single_line_label();
                    tray.set_title(Some(&label));
                    if let Err(error) = tray
                        .set_tooltip(Some(&label))
                        .and_then(|()| tray.set_visible(state.visible))
                    {
                        eprintln!("kokorobox-traffic-presenter: update status item: {error}");
                        *control_flow = ControlFlow::ExitWithCode(1);
                    }
                }
            }
            Event::UserEvent(UserEvent::InputClosed) => {
                *control_flow = ControlFlow::Exit;
            }
            Event::UserEvent(UserEvent::InputError(error)) => {
                eprintln!("kokorobox-traffic-presenter: read command: {error}");
                *control_flow = ControlFlow::ExitWithCode(1);
            }
            _ => {}
        }
    })
}
