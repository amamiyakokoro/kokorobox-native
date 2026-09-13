use kokorobox_traffic_presenter::{
    PresenterCommand, PresenterLayout, PresenterState, PresenterTransition, parse_command,
};
use std::io::{self, BufRead};
use std::sync::atomic::{AtomicIsize, AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};
use windows::Win32::Foundation::{
    CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HWND, LPARAM, LRESULT, RECT, WPARAM,
};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, COLOR_WINDOW, COLOR_WINDOWTEXT, DEFAULT_GUI_FONT, DT_CENTER, DT_END_ELLIPSIS,
    DT_SINGLELINE, DT_VCENTER, DrawTextW, EndPaint, FillRect, GetStockObject, GetSysColor,
    GetSysColorBrush, InvalidateRect, PAINTSTRUCT, SelectObject, SetBkMode, SetTextColor,
    TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW,
    FindWindowExW, FindWindowW, GetClientRect, GetMessageW, HWND_TOP, IsWindow, MSG, PostMessageW,
    PostQuitMessage, RegisterClassW, RegisterWindowMessageW, SW_HIDE, SW_SHOWNOACTIVATE,
    SWP_NOACTIVATE, SetTimer, SetWindowPos, ShowWindow, TranslateMessage, WINDOW_EX_STYLE, WM_APP,
    WM_CLOSE, WM_DESTROY, WM_DISPLAYCHANGE, WM_ERASEBKGND, WM_PAINT, WM_SETTINGCHANGE, WM_TIMER,
    WNDCLASSW, WS_CHILD, WS_CLIPSIBLINGS, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_OVERLAPPED,
};
use windows::core::{PCWSTR, w};

const WM_PRESENTER_UPDATE: u32 = WM_APP + 1;
const RETRY_TIMER_ID: usize = 1;

static STATE: OnceLock<Mutex<PresenterState>> = OnceLock::new();
static CONTROLLER_WINDOW: AtomicIsize = AtomicIsize::new(0);
static PRESENTER_WINDOW: AtomicIsize = AtomicIsize::new(0);
static TASKBAR_CREATED: AtomicU32 = AtomicU32::new(0);

fn hwnd_from_atomic(value: &AtomicIsize) -> Option<HWND> {
    let raw = value.load(Ordering::Acquire);
    (raw != 0).then_some(HWND(raw as *mut _))
}

fn store_hwnd(value: &AtomicIsize, hwnd: Option<HWND>) {
    value.store(
        hwnd.map_or(0, |window| window.0 as isize),
        Ordering::Release,
    );
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let taskbar_created = TASKBAR_CREATED.load(Ordering::Relaxed);
    if taskbar_created != 0 && message == taskbar_created {
        // SAFETY: This callback runs on the presenter UI thread.
        let _ = unsafe { ensure_presenter_window() };
        return LRESULT(0);
    }

    match message {
        WM_PRESENTER_UPDATE | WM_DISPLAYCHANGE | WM_SETTINGCHANGE | WM_TIMER => {
            // SAFETY: This callback runs on the presenter UI thread.
            let _ = unsafe { ensure_presenter_window() };
            LRESULT(0)
        }
        WM_PAINT => {
            if hwnd_from_atomic(&PRESENTER_WINDOW) == Some(hwnd) {
                // SAFETY: hwnd is the window currently being painted.
                unsafe { paint_presenter(hwnd) };
                LRESULT(0)
            } else {
                // SAFETY: Unhandled messages are delegated to the default procedure.
                unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
            }
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_DESTROY => {
            if hwnd_from_atomic(&CONTROLLER_WINDOW) == Some(hwnd) {
                // SAFETY: Ends the message loop owned by this thread.
                unsafe { PostQuitMessage(0) };
            } else if hwnd_from_atomic(&PRESENTER_WINDOW) == Some(hwnd) {
                store_hwnd(&PRESENTER_WINDOW, None);
            }
            LRESULT(0)
        }
        _ => {
            // SAFETY: Unhandled messages are delegated to the default procedure.
            unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
        }
    }
}

unsafe fn ensure_presenter_window() -> windows::core::Result<()> {
    let taskbar = unsafe { FindWindowW(w!("Shell_TrayWnd"), PCWSTR::null()) }?;
    let presenter = match hwnd_from_atomic(&PRESENTER_WINDOW) {
        Some(window) if unsafe { IsWindow(Some(window)) }.as_bool() => window,
        _ => {
            let instance = unsafe { GetModuleHandleW(None) }?;
            let window = unsafe {
                CreateWindowExW(
                    WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                    w!("KokoroBoxTrafficPresenter"),
                    w!("KokoroBox traffic"),
                    WS_CHILD | WS_CLIPSIBLINGS,
                    0,
                    0,
                    0,
                    0,
                    Some(taskbar),
                    None,
                    Some(instance.into()),
                    None,
                )
            }?;
            store_hwnd(&PRESENTER_WINDOW, Some(window));
            window
        }
    };

    position_presenter(taskbar, presenter)?;
    let state = *STATE
        .get_or_init(|| Mutex::new(PresenterState::default()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    unsafe {
        let _ = ShowWindow(
            presenter,
            if state.visible {
                SW_SHOWNOACTIVATE
            } else {
                SW_HIDE
            },
        );
        let _ = InvalidateRect(Some(presenter), None, true);
    }
    Ok(())
}

fn position_presenter(taskbar: HWND, presenter: HWND) -> windows::core::Result<()> {
    let mut taskbar_rect = RECT::default();
    // SAFETY: Both handles refer to live windows on this UI thread.
    unsafe { windows::Win32::UI::WindowsAndMessaging::GetWindowRect(taskbar, &mut taskbar_rect) }?;
    let taskbar_width = taskbar_rect.right - taskbar_rect.left;
    let taskbar_height = taskbar_rect.bottom - taskbar_rect.top;
    let horizontal = taskbar_width >= taskbar_height;

    let state = *STATE
        .get_or_init(|| Mutex::new(PresenterState::default()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let desired_width = match state.layout {
        PresenterLayout::Horizontal => 230,
        PresenterLayout::Stacked => 138,
    };

    let tray = unsafe { FindWindowExW(Some(taskbar), None, w!("TrayNotifyWnd"), PCWSTR::null()) };
    let mut tray_rect = RECT::default();
    let has_tray_rect = tray
        .and_then(|window| unsafe {
            windows::Win32::UI::WindowsAndMessaging::GetWindowRect(window, &mut tray_rect)
        })
        .is_ok();

    let (x, y, width, height) = if horizontal {
        let x = if has_tray_rect {
            tray_rect.left - taskbar_rect.left - desired_width - 4
        } else {
            taskbar_width - desired_width - 180
        };
        (
            x.max(0),
            0,
            desired_width.min(taskbar_width),
            taskbar_height,
        )
    } else {
        let desired_height = 64.min(taskbar_height);
        let y = if has_tray_rect {
            tray_rect.top - taskbar_rect.top - desired_height - 4
        } else {
            taskbar_height - desired_height - 120
        };
        (0, y.max(0), taskbar_width, desired_height)
    };

    unsafe {
        SetWindowPos(
            presenter,
            Some(HWND_TOP),
            x,
            y,
            width,
            height,
            SWP_NOACTIVATE,
        )
    }
}

unsafe fn paint_presenter(hwnd: HWND) {
    let state = *STATE
        .get_or_init(|| Mutex::new(PresenterState::default()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut paint = PAINTSTRUCT::default();
    let device = unsafe { BeginPaint(hwnd, &mut paint) };
    let mut client = RECT::default();
    let _ = unsafe { GetClientRect(hwnd, &mut client) };
    unsafe {
        FillRect(device, &client, GetSysColorBrush(COLOR_WINDOW));
        SetBkMode(device, TRANSPARENT);
        SetTextColor(
            device,
            windows::Win32::Foundation::COLORREF(GetSysColor(COLOR_WINDOWTEXT)),
        );
        SelectObject(device, GetStockObject(DEFAULT_GUI_FONT));
    }

    let flags = DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS;
    match state.layout {
        PresenterLayout::Horizontal => {
            draw_text(device, &state.single_line_label(), &mut client, flags);
        }
        PresenterLayout::Stacked => {
            let middle = (client.top + client.bottom) / 2;
            let mut upper = RECT {
                bottom: middle,
                ..client
            };
            let mut lower = RECT {
                top: middle,
                ..client
            };
            draw_text(device, &state.upload_label(), &mut upper, flags);
            draw_text(device, &state.download_label(), &mut lower, flags);
        }
    }
    let _ = unsafe { EndPaint(hwnd, &paint) };
}

fn draw_text(
    device: windows::Win32::Graphics::Gdi::HDC,
    text: &str,
    rect: &mut RECT,
    flags: windows::Win32::Graphics::Gdi::DRAW_TEXT_FORMAT,
) {
    let mut wide: Vec<u16> = text.encode_utf16().collect();
    // SAFETY: DrawTextW only mutates the buffer when DT_MODIFYSTRING is used, which is absent.
    unsafe {
        DrawTextW(device, &mut wide, rect, flags);
    }
}

fn register_window_classes() -> windows::core::Result<()> {
    let instance = unsafe { GetModuleHandleW(None) }?;
    for class_name in [
        w!("KokoroBoxTrafficController"),
        w!("KokoroBoxTrafficPresenter"),
    ] {
        let class = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(window_proc),
            hInstance: instance.into(),
            lpszClassName: class_name,
            hbrBackground: unsafe { GetSysColorBrush(COLOR_WINDOW) },
            ..Default::default()
        };
        if unsafe { RegisterClassW(&class) } == 0 {
            return Err(windows::core::Error::from_thread());
        }
    }
    Ok(())
}

fn apply_command(command: PresenterCommand) {
    let transition = {
        let mut state = STATE
            .get_or_init(|| Mutex::new(PresenterState::default()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.apply(command)
    };
    if let Some(controller) = hwnd_from_atomic(&CONTROLLER_WINDOW) {
        let message = if transition == PresenterTransition::Shutdown {
            WM_CLOSE
        } else {
            WM_PRESENTER_UPDATE
        };
        let _ = unsafe { PostMessageW(Some(controller), message, WPARAM(0), LPARAM(0)) };
    }
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let instance_mutex =
        unsafe { CreateMutexW(None, true, w!("Local\\KokoroBoxTrafficPresenter-6B48E72F")) }?;
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        unsafe { CloseHandle(instance_mutex) }?;
        return Ok(());
    }

    register_window_classes()?;
    TASKBAR_CREATED.store(
        unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) },
        Ordering::Release,
    );
    let module = unsafe { GetModuleHandleW(None) }?;
    let controller = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("KokoroBoxTrafficController"),
            w!("KokoroBox traffic controller"),
            WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            None,
            None,
            Some(module.into()),
            None,
        )
    }?;
    store_hwnd(&CONTROLLER_WINDOW, Some(controller));
    unsafe { SetTimer(Some(controller), RETRY_TIMER_ID, 2_000, None) };
    let _ = unsafe { ensure_presenter_window() };

    std::thread::spawn(|| {
        for line in io::stdin().lock().lines() {
            match line {
                Ok(line) if line.trim().is_empty() => continue,
                Ok(line) => match parse_command(&line) {
                    Ok(command) => apply_command(command),
                    Err(error) => {
                        eprintln!("kokorobox-traffic-presenter: read command: {error}");
                        break;
                    }
                },
                Err(error) => {
                    eprintln!("kokorobox-traffic-presenter: read command: {error}");
                    break;
                }
            }
        }
        if let Some(controller) = hwnd_from_atomic(&CONTROLLER_WINDOW) {
            let _ = unsafe { PostMessageW(Some(controller), WM_CLOSE, WPARAM(0), LPARAM(0)) };
        }
    });

    let mut message = MSG::default();
    while unsafe { GetMessageW(&mut message, None, 0, 0) }.as_bool() {
        unsafe {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }

    if let Some(presenter) = hwnd_from_atomic(&PRESENTER_WINDOW) {
        let _ = unsafe { DestroyWindow(presenter) };
    }
    let _ = unsafe { DestroyWindow(controller) };
    unsafe { CloseHandle(instance_mutex) }?;
    Ok(())
}
