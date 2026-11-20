use block2::RcBlock;
use kokorobox_traffic_presenter::{
    PresenterCommand, PresenterState, PresenterTransition, parse_command,
};
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Bool};
use objc2_app_kit::{
    NSCellImagePosition, NSColor, NSFont, NSFontAttributeName, NSFontWeightMedium,
    NSForegroundColorAttributeName, NSImage, NSStatusBar, NSStatusItem, NSStringDrawing,
};
use objc2_foundation::{NSDictionary, NSPoint, NSRect, NSSize, NSString};
use std::io::{self, BufRead};
use tao::event::Event;
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS};

// Leave room for the longest labels, such as "↑ 100.0 MB/s", plus the item insets.
const STATUS_ITEM_WIDTH: f64 = 80.0;
const IMAGE_SIZE: NSSize = NSSize::new(70.0, 22.0);
const FONT_SIZE: f64 = 9.0;

fn traffic_image(state: &PresenterState) -> Retained<NSImage> {
    let upload = state.upload_label();
    let download = state.download_label();
    let draw = RcBlock::new(move |_: NSRect| {
        // SAFETY: These AppKit constants are available on supported macOS versions.
        let (weight, font_key, color_key) = unsafe {
            (
                NSFontWeightMedium,
                NSFontAttributeName,
                NSForegroundColorAttributeName,
            )
        };
        let font = NSFont::monospacedSystemFontOfSize_weight(FONT_SIZE, weight);
        let ink = NSColor::blackColor();
        let attributes = NSDictionary::<NSString, AnyObject>::from_slices(
            &[font_key, color_key],
            &[&font, &ink],
        );
        // SAFETY: The font and color values have the types required by these attributes.
        unsafe {
            NSString::from_str(&upload)
                .drawAtPoint_withAttributes(NSPoint::new(1.0, 0.0), Some(&attributes));
            NSString::from_str(&download).drawAtPoint_withAttributes(
                NSPoint::new(1.0, IMAGE_SIZE.height / 2.0),
                Some(&attributes),
            );
        }
        Bool::YES
    });
    let image = NSImage::imageWithSize_flipped_drawingHandler(IMAGE_SIZE, true, &draw);
    // AppKit supplies the status bar foreground color and renders at the display scale.
    image.setTemplate(true);
    image
}

fn update_status_item(
    status_item: &mut Option<Retained<NSStatusItem>>,
    state: &PresenterState,
    main_thread: MainThreadMarker,
) -> Result<(), &'static str> {
    if !state.visible {
        if let Some(item) = status_item.take() {
            NSStatusBar::systemStatusBar().removeStatusItem(&item);
        }
        return Ok(());
    }
    let item = status_item.get_or_insert_with(|| {
        NSStatusBar::systemStatusBar().statusItemWithLength(STATUS_ITEM_WIDTH)
    });
    let button = item
        .button(main_thread)
        .ok_or("missing status item button")?;
    let label = NSString::from_str(&state.single_line_label());
    let image = traffic_image(state);
    image.setAccessibilityDescription(Some(&label));
    button.setImagePosition(NSCellImagePosition::ImageOnly);
    button.setImage(Some(&image));
    button.setToolTip(Some(&label));
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

    let main_thread = MainThreadMarker::new().expect("status item must run on the main thread");
    let mut state = PresenterState::default();
    let mut status_item = None;
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::UserEvent(UserEvent::Command(command)) => {
                if state.apply(command) == PresenterTransition::Shutdown {
                    *control_flow = ControlFlow::Exit;
                    return;
                }
                if let Err(error) = update_status_item(&mut status_item, &state, main_thread) {
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
