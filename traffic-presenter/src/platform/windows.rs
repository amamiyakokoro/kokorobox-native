use kokorobox_traffic_presenter::{
    PresenterCommand, PresenterLayout, PresenterState, PresenterTheme, PresenterTransition,
    parse_command,
};
use std::io::{self, BufRead};
use std::mem::size_of;
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};
use windows::Win32::Foundation::{
    COLORREF, CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HWND, LPARAM, LRESULT, POINT, RECT,
    SIZE, WPARAM,
};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_DRAW_TEXT_OPTIONS_CLIP, D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_FEATURE_LEVEL_DEFAULT,
    D2D1_RENDER_TARGET_PROPERTIES, D2D1_RENDER_TARGET_TYPE_DEFAULT, D2D1_RENDER_TARGET_USAGE_NONE,
    D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE, D2D1CreateFactory, ID2D1DCRenderTarget, ID2D1Factory,
};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
    DWRITE_FONT_WEIGHT_NORMAL, DWRITE_MEASURING_MODE_NATURAL, DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
    DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_WORD_WRAPPING_NO_WRAP, DWriteCreateFactory,
    IDWriteFactory, IDWriteTextFormat,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION, BeginPaint,
    COLOR_WINDOW, COLOR_WINDOWTEXT, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC,
    DeleteObject, EndPaint, GetSysColor, GetSysColorBrush, HBITMAP, HDC, HGDIOBJ, InvalidateRect,
    PAINTSTRUCT, SelectObject,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForWindow, SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW,
    FindWindowExW, FindWindowW, GWL_STYLE, GetClientRect, GetMessageW, GetParent,
    GetWindowLongPtrW, HWND_TOP, HWND_TOPMOST, IsWindow, MSG, PostMessageW, PostQuitMessage,
    RegisterClassW, RegisterWindowMessageW, SW_HIDE, SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SetParent,
    SetTimer, SetWindowLongPtrW, SetWindowPos, ShowWindow, TranslateMessage, ULW_ALPHA,
    UpdateLayeredWindow, WINDOW_EX_STYLE, WINDOW_STYLE, WM_APP, WM_CLOSE, WM_DESTROY,
    WM_DISPLAYCHANGE, WM_ERASEBKGND, WM_PAINT, WM_SETTINGCHANGE, WM_TIMER, WNDCLASSW, WS_CHILD,
    WS_CLIPSIBLINGS, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT,
    WS_OVERLAPPED, WS_POPUP,
};
use windows::core::{PCWSTR, w};

const WM_PRESENTER_UPDATE: u32 = WM_APP + 1;
const RETRY_TIMER_ID: usize = 1;
const LIGHT_TEXT: COLORREF = COLORREF(0x00f4_f4_f4);
const LIGHT_TEXT_SHADOW: COLORREF = COLORREF(0x0015_1515);
const DARK_TEXT: COLORREF = COLORREF(0x0015_1515);
const DARK_TEXT_SHADOW: COLORREF = COLORREF(0x00f4_f4_f4);

static STATE: OnceLock<Mutex<PresenterState>> = OnceLock::new();
static CONTROLLER_WINDOW: AtomicIsize = AtomicIsize::new(0);
static PRESENTER_WINDOW: AtomicIsize = AtomicIsize::new(0);
static TASKBAR_CREATED: AtomicU32 = AtomicU32::new(0);
static ATTACHMENT_FAILURE_REPORTED: AtomicBool = AtomicBool::new(false);
static PRESENTER_FAILURE_REPORTED: AtomicBool = AtomicBool::new(false);
static RENDER_FAILURE_REPORTED: AtomicBool = AtomicBool::new(false);

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
        refresh_presenter();
        return LRESULT(0);
    }

    match message {
        WM_PRESENTER_UPDATE | WM_DISPLAYCHANGE | WM_SETTINGCHANGE | WM_TIMER => {
            refresh_presenter();
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

fn refresh_presenter() {
    // SAFETY: Every caller runs on the presenter UI thread.
    match unsafe { ensure_presenter_window() } {
        Ok(()) => {
            PRESENTER_FAILURE_REPORTED.store(false, Ordering::Release);
        }
        Err(error) => {
            if !PRESENTER_FAILURE_REPORTED.swap(true, Ordering::AcqRel) {
                eprintln!("kokorobox-traffic-presenter: update taskbar window: {error}");
            }
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
                    WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_LAYERED | WS_EX_TRANSPARENT,
                    w!("KokoroBoxTrafficPresenter"),
                    w!("KokoroBox traffic"),
                    WS_POPUP | WS_CLIPSIBLINGS,
                    0,
                    0,
                    0,
                    0,
                    None,
                    None,
                    Some(instance.into()),
                    None,
                )
            }?;
            store_hwnd(&PRESENTER_WINDOW, Some(window));
            window
        }
    };

    let attached = unsafe { attach_presenter_to_taskbar(presenter, taskbar) };
    if attached {
        ATTACHMENT_FAILURE_REPORTED.store(false, Ordering::Release);
    } else {
        if !ATTACHMENT_FAILURE_REPORTED.swap(true, Ordering::AcqRel) {
            eprintln!(
                "kokorobox-traffic-presenter: could not attach to the taskbar; using top-level fallback"
            );
        }
    }

    position_presenter(taskbar, presenter, attached)?;
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

unsafe fn attach_presenter_to_taskbar(presenter: HWND, taskbar: HWND) -> bool {
    // SetParent deliberately does not update WS_CHILD/WS_POPUP. A popup can therefore be
    // parented by Explorer while GetParent still reports no parent, which makes the caller use
    // screen coordinates for a taskbar-relative window and places it outside the taskbar.
    unsafe { set_presenter_style(presenter, true) };
    if unsafe { GetParent(presenter) }.ok() == Some(taskbar) {
        return true;
    }

    let _ = unsafe { SetParent(presenter, Some(taskbar)) };
    if unsafe { GetParent(presenter) }.ok() == Some(taskbar) {
        return true;
    }

    // Keep the fallback as a real top-level popup. This also undoes a partially successful
    // SetParent call before position_presenter starts using screen coordinates.
    let _ = unsafe { SetParent(presenter, None) };
    unsafe { set_presenter_style(presenter, false) };
    false
}

fn presenter_style(attached: bool) -> WINDOW_STYLE {
    if attached {
        WS_CHILD | WS_CLIPSIBLINGS
    } else {
        WS_POPUP | WS_CLIPSIBLINGS
    }
}

unsafe fn set_presenter_style(presenter: HWND, attached: bool) {
    let current = unsafe { GetWindowLongPtrW(presenter, GWL_STYLE) } as u32;
    let parent_bits = WS_CHILD.0 | WS_POPUP.0;
    let next = (current & !parent_bits) | presenter_style(attached).0;
    if next != current {
        let _ = unsafe { SetWindowLongPtrW(presenter, GWL_STYLE, next as isize) };
    }
}

fn position_presenter(taskbar: HWND, presenter: HWND, attached: bool) -> windows::core::Result<()> {
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

    let (mut x, mut y, width, height) = if horizontal {
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

    if !attached {
        x += taskbar_rect.left;
        y += taskbar_rect.top;
    }

    unsafe {
        SetWindowPos(
            presenter,
            Some(if attached { HWND_TOP } else { HWND_TOPMOST }),
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
    let _device = unsafe { BeginPaint(hwnd, &mut paint) };
    let mut client = RECT::default();
    let _ = unsafe { GetClientRect(hwnd, &mut client) };
    let width = client.right - client.left;
    let height = client.bottom - client.top;
    if width > 0 && height > 0 {
        match render_presenter_directwrite(hwnd, width, height, state) {
            Ok(()) => {
                RENDER_FAILURE_REPORTED.store(false, Ordering::Release);
            }
            Err(error) => {
                if !RENDER_FAILURE_REPORTED.swap(true, Ordering::AcqRel) {
                    eprintln!("kokorobox-traffic-presenter: render taskbar text: {error}");
                }
            }
        }
    }
    let _ = unsafe { EndPaint(hwnd, &paint) };
}

struct LayeredBitmap {
    dc: HDC,
    bitmap: HBITMAP,
    previous_bitmap: HGDIOBJ,
}

impl LayeredBitmap {
    fn new(width: i32, height: i32) -> windows::core::Result<Self> {
        let dc = unsafe { CreateCompatibleDC(None) };
        if dc.0.is_null() {
            return Err(windows::core::Error::from_thread());
        }

        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits = std::ptr::null_mut();
        let bitmap =
            match unsafe { CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut bits, None, 0) } {
                Ok(bitmap) => bitmap,
                Err(error) => {
                    let _ = unsafe { DeleteDC(dc) };
                    return Err(error);
                }
            };
        let previous_bitmap = unsafe { SelectObject(dc, bitmap.into()) };
        Ok(Self {
            dc,
            bitmap,
            previous_bitmap,
        })
    }
}

impl Drop for LayeredBitmap {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.dc, self.previous_bitmap);
            let _ = DeleteObject(self.bitmap.into());
            let _ = DeleteDC(self.dc);
        }
    }
}

fn render_presenter_directwrite(
    hwnd: HWND,
    width: i32,
    height: i32,
    state: PresenterState,
) -> windows::core::Result<()> {
    let buffer = LayeredBitmap::new(width, height)?;
    let factory: ID2D1Factory =
        unsafe { D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None) }?;
    let properties = D2D1_RENDER_TARGET_PROPERTIES {
        r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
        pixelFormat: D2D1_PIXEL_FORMAT {
            format: DXGI_FORMAT_B8G8R8A8_UNORM,
            alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
        },
        dpiX: 96.0,
        dpiY: 96.0,
        usage: D2D1_RENDER_TARGET_USAGE_NONE,
        minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
    };
    let render_target: ID2D1DCRenderTarget = unsafe { factory.CreateDCRenderTarget(&properties) }?;
    let target_rect = RECT {
        left: 0,
        top: 0,
        right: width,
        bottom: height,
    };
    unsafe {
        render_target.BindDC(buffer.dc, &target_rect)?;
        render_target.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
        render_target.BeginDraw();
        render_target.Clear(Some(&D2D1_COLOR_F {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        }));
    }

    let dwrite: IDWriteFactory = unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED) }?;
    let font_size = -taskbar_font_height(height, unsafe { GetDpiForWindow(hwnd) }) as f32;
    let text_format = unsafe {
        dwrite.CreateTextFormat(
            w!("Segoe UI Variable Text"),
            None,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            font_size,
            w!(""),
        )
    }?;
    unsafe {
        text_format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
        text_format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
        text_format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
    }

    match state.layout {
        PresenterLayout::Horizontal => draw_directwrite_text(
            &render_target,
            &text_format,
            &state.single_line_label(),
            D2D_RECT_F {
                left: 0.0,
                top: 0.0,
                right: width as f32,
                bottom: height as f32,
            },
            state.theme,
        )?,
        PresenterLayout::Stacked => {
            let middle = height as f32 / 2.0;
            draw_directwrite_text(
                &render_target,
                &text_format,
                &state.upload_label(),
                D2D_RECT_F {
                    left: 0.0,
                    top: 0.0,
                    right: width as f32,
                    bottom: middle,
                },
                state.theme,
            )?;
            draw_directwrite_text(
                &render_target,
                &text_format,
                &state.download_label(),
                D2D_RECT_F {
                    left: 0.0,
                    top: middle,
                    right: width as f32,
                    bottom: height as f32,
                },
                state.theme,
            )?;
        }
    }

    unsafe { render_target.EndDraw(None, None)? };
    let source = POINT { x: 0, y: 0 };
    let size = SIZE {
        cx: width,
        cy: height,
    };
    let blend = BLENDFUNCTION {
        BlendOp: AC_SRC_OVER as u8,
        BlendFlags: 0,
        SourceConstantAlpha: 255,
        AlphaFormat: AC_SRC_ALPHA as u8,
    };
    unsafe {
        UpdateLayeredWindow(
            hwnd,
            None,
            None,
            Some(&size),
            Some(buffer.dc),
            Some(&source),
            COLORREF(0),
            Some(&blend),
            ULW_ALPHA,
        )
    }
}

fn taskbar_font_height(client_height: i32, window_dpi: u32) -> i32 {
    let dpi = if window_dpi == 0 { 96 } else { window_dpi };
    let dpi_scaled = ((14 * dpi + 48) / 96).clamp(12, 32) as i32;
    let available_row_height = (client_height / 2 - 2).max(12);
    -dpi_scaled.min(available_row_height)
}

fn presenter_text_colors(theme: PresenterTheme) -> (COLORREF, COLORREF) {
    match theme {
        PresenterTheme::Dark => (LIGHT_TEXT, LIGHT_TEXT_SHADOW),
        PresenterTheme::Light => (DARK_TEXT, DARK_TEXT_SHADOW),
        PresenterTheme::System => {
            let system_text = unsafe { GetSysColor(COLOR_WINDOWTEXT) };
            if color_is_dark(system_text) {
                (DARK_TEXT, DARK_TEXT_SHADOW)
            } else {
                (LIGHT_TEXT, LIGHT_TEXT_SHADOW)
            }
        }
    }
}

fn color_is_dark(color: u32) -> bool {
    let red = color & 0xff;
    let green = (color >> 8) & 0xff;
    let blue = (color >> 16) & 0xff;
    red * 299 + green * 587 + blue * 114 < 128_000
}

fn draw_directwrite_text(
    render_target: &ID2D1DCRenderTarget,
    text_format: &IDWriteTextFormat,
    text: &str,
    rect: D2D_RECT_F,
    theme: PresenterTheme,
) -> windows::core::Result<()> {
    let (foreground, shadow_color) = presenter_text_colors(theme);
    if theme == PresenterTheme::System {
        let shadow_rect = D2D_RECT_F {
            left: rect.left + 1.0,
            top: rect.top + 1.0,
            right: rect.right + 1.0,
            bottom: rect.bottom + 1.0,
        };
        draw_directwrite_pass(render_target, text_format, text, &shadow_rect, shadow_color)?;
    }
    draw_directwrite_pass(render_target, text_format, text, &rect, foreground)
}

fn draw_directwrite_pass(
    render_target: &ID2D1DCRenderTarget,
    text_format: &IDWriteTextFormat,
    text: &str,
    rect: &D2D_RECT_F,
    color: COLORREF,
) -> windows::core::Result<()> {
    let brush_color = colorref_to_d2d(color);
    let brush = unsafe { render_target.CreateSolidColorBrush(&brush_color, None) }?;
    let wide: Vec<u16> = text.encode_utf16().collect();
    unsafe {
        render_target.DrawText(
            &wide,
            text_format,
            rect,
            &brush,
            D2D1_DRAW_TEXT_OPTIONS_CLIP,
            DWRITE_MEASURING_MODE_NATURAL,
        );
    }
    Ok(())
}

fn colorref_to_d2d(color: COLORREF) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: (color.0 & 0xff) as f32 / 255.0,
        g: ((color.0 >> 8) & 0xff) as f32 / 255.0,
        b: ((color.0 >> 16) & 0xff) as f32 / 255.0,
        a: 1.0,
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
    // Render directly at each monitor's physical DPI. Without this, Windows bitmap-scales the
    // entire presenter at 125%/150%, which makes small taskbar text visibly soft and pixelated.
    let _ = unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };

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
    refresh_presenter();

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attached_presenter_is_a_child_and_fallback_is_a_popup() {
        let attached = presenter_style(true);
        assert_ne!(attached.0 & WS_CHILD.0, 0);
        assert_eq!(attached.0 & WS_POPUP.0, 0);

        let fallback = presenter_style(false);
        assert_eq!(fallback.0 & WS_CHILD.0, 0);
        assert_ne!(fallback.0 & WS_POPUP.0, 0);
    }

    #[test]
    fn taskbar_text_contrasts_with_both_light_and_dark_themes() {
        let (dark_foreground, dark_shadow) = presenter_text_colors(PresenterTheme::Dark);
        assert!(!color_is_dark(dark_foreground.0));
        assert!(color_is_dark(dark_shadow.0));

        let (light_foreground, light_shadow) = presenter_text_colors(PresenterTheme::Light);
        assert!(color_is_dark(light_foreground.0));
        assert!(!color_is_dark(light_shadow.0));
    }

    #[test]
    fn taskbar_font_scales_with_the_taskbar_without_becoming_extreme() {
        assert_eq!(taskbar_font_height(48, 96), -14);
        assert_eq!(taskbar_font_height(60, 120), -18);
        assert_eq!(taskbar_font_height(72, 144), -21);
        assert_eq!(taskbar_font_height(96, 192), -28);
        assert_eq!(taskbar_font_height(48, 0), -14);
    }
}
