//! A small always-on-top status pill near the bottom of the target's monitor.
//! It never activates, takes keyboard focus, or receives mouse input, so the
//! focus checks that protect insertion are unaffected.

use super::wide;
use anyhow::{Result, ensure};
use std::{cell::RefCell, mem::zeroed, ptr::null};
use windows_sys::Win32::{
    Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM},
    Graphics::Gdi::{
        BeginPaint, CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, CreateFontW, CreateRoundRectRgn,
        CreateSolidBrush, DEFAULT_CHARSET, DEFAULT_PITCH, DT_END_ELLIPSIS, DT_SINGLELINE,
        DT_VCENTER, DeleteObject, DrawTextW, Ellipse, EndPaint, FW_SEMIBOLD, FillRect,
        GetMonitorInfoW, GetStockObject, InvalidateRect, MONITOR_DEFAULTTOPRIMARY, MONITORINFO,
        MonitorFromWindow, NULL_PEN, OUT_DEFAULT_PRECIS, PAINTSTRUCT, SelectObject, SetBkMode,
        SetTextColor, SetWindowRgn, TRANSPARENT,
    },
    System::LibraryLoader::GetModuleHandleW,
    UI::HiDpi::GetDpiForWindow,
    UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, GetClientRect, HWND_TOPMOST, LWA_ALPHA,
        RegisterClassW, SW_HIDE, SWP_NOACTIVATE, SWP_NOSIZE, SWP_SHOWWINDOW,
        SetLayeredWindowAttributes, SetWindowPos, ShowWindow, WM_PAINT, WNDCLASSW, WS_EX_LAYERED,
        WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
    },
};

const WIDTH: i32 = 340;
const HEIGHT: i32 = 40;

pub const RED: COLORREF = rgb(235, 72, 72);
pub const AMBER: COLORREF = rgb(245, 182, 66);
pub const GREEN: COLORREF = rgb(76, 201, 120);
pub const GRAY: COLORREF = rgb(150, 156, 168);

const fn rgb(red: u8, green: u8, blue: u8) -> COLORREF {
    red as u32 | (green as u32) << 8 | (blue as u32) << 16
}

/// Convert 96-DPI layout units to the overlay monitor's pixels.
fn px(value: i32, dpi: i32) -> i32 {
    value * dpi / 96
}

thread_local! {
    // The window procedure runs on the thread that owns the overlay.
    static CONTENT: RefCell<(Vec<u16>, COLORREF, i32)> = const { RefCell::new((Vec::new(), 0, 96)) };
}

pub struct Overlay(HWND);

impl Overlay {
    pub fn new() -> Result<Self> {
        let class = wide("SaykeyOverlay");
        // SAFETY: class and window parameters are valid for the calls; the
        // class name outlives registration and creation.
        unsafe {
            let instance = GetModuleHandleW(null());
            let mut window_class: WNDCLASSW = zeroed();
            window_class.lpfnWndProc = Some(procedure);
            window_class.hInstance = instance;
            window_class.lpszClassName = class.as_ptr();
            // Registration fails harmlessly when a previous session registered it.
            RegisterClassW(&window_class);
            let window = CreateWindowExW(
                WS_EX_TOPMOST
                    | WS_EX_TOOLWINDOW
                    | WS_EX_NOACTIVATE
                    | WS_EX_LAYERED
                    | WS_EX_TRANSPARENT,
                class.as_ptr(),
                class.as_ptr(),
                WS_POPUP,
                0,
                0,
                WIDTH,
                HEIGHT,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                instance,
                null(),
            );
            ensure!(!window.is_null(), "Cannot create the status overlay");
            SetLayeredWindowAttributes(window, 0, 235, LWA_ALPHA);
            Ok(Self(window))
        }
    }

    /// Show `text` with a colored dot, centered above the taskbar on `near`'s monitor.
    pub fn show(&self, text: &str, color: COLORREF, near: HWND) {
        // SAFETY: handles are only passed to Windows; info has the required size.
        unsafe {
            let monitor = MonitorFromWindow(near, MONITOR_DEFAULTTOPRIMARY);
            let mut info: MONITORINFO = zeroed();
            info.cbSize = size_of::<MONITORINFO>() as u32;
            let work = if GetMonitorInfoW(monitor, &mut info) != 0 {
                info.rcWork
            } else {
                RECT {
                    left: 0,
                    top: 0,
                    right: 800,
                    bottom: 600,
                }
            };
            // Move onto the target monitor first so its DPI applies.
            SetWindowPos(
                self.0,
                HWND_TOPMOST,
                work.left,
                work.top,
                0,
                0,
                SWP_NOACTIVATE | SWP_NOSIZE,
            );
            let dpi = (GetDpiForWindow(self.0) as i32).max(96);
            let (width, height) = (px(WIDTH, dpi), px(HEIGHT, dpi));
            CONTENT.with(|content| {
                *content.borrow_mut() = (text.encode_utf16().collect(), color, dpi);
            });
            // The system owns the region after SetWindowRgn succeeds.
            SetWindowRgn(
                self.0,
                CreateRoundRectRgn(0, 0, width + 1, height + 1, height, height),
                1,
            );
            SetWindowPos(
                self.0,
                HWND_TOPMOST,
                (work.left + work.right - width) / 2,
                work.bottom - height - px(40, dpi),
                width,
                height,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
            InvalidateRect(self.0, null(), 1);
        }
    }

    pub fn hide(&self) {
        // SAFETY: the window belongs to this thread.
        unsafe {
            ShowWindow(self.0, SW_HIDE);
        }
    }
}

impl Drop for Overlay {
    fn drop(&mut self) {
        // SAFETY: the window was created by this thread and is destroyed once.
        unsafe {
            DestroyWindow(self.0);
        }
    }
}

unsafe extern "system" fn procedure(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message != WM_PAINT {
        // SAFETY: default handling for every other message.
        return unsafe { DefWindowProcW(window, message, wparam, lparam) };
    }
    // SAFETY: standard paint sequence; every GDI object created here is deleted.
    unsafe {
        let mut paint: PAINTSTRUCT = zeroed();
        let dc = BeginPaint(window, &mut paint);
        let mut rect: RECT = zeroed();
        GetClientRect(window, &mut rect);
        let background = CreateSolidBrush(rgb(28, 30, 36));
        FillRect(dc, &rect, background);
        DeleteObject(background);
        CONTENT.with(|content| {
            let (text, color, dpi) = &*content.borrow();
            let dpi = *dpi;
            // Windows can paint before the first show; an empty Vec's pointer
            // is dangling and must not reach DrawTextW.
            if text.is_empty() {
                return;
            }
            let dot = CreateSolidBrush(*color);
            let previous_brush = SelectObject(dc, dot);
            let previous_pen = SelectObject(dc, GetStockObject(NULL_PEN));
            Ellipse(dc, px(16, dpi), px(14, dpi), px(28, dpi), px(26, dpi));
            SelectObject(dc, previous_pen);
            SelectObject(dc, previous_brush);
            DeleteObject(dot);
            let face = wide("Segoe UI");
            let font = CreateFontW(
                -px(15, dpi),
                0,
                0,
                0,
                FW_SEMIBOLD as i32,
                0,
                0,
                0,
                DEFAULT_CHARSET as u32,
                OUT_DEFAULT_PRECIS as u32,
                CLIP_DEFAULT_PRECIS as u32,
                CLEARTYPE_QUALITY as u32,
                DEFAULT_PITCH as u32,
                face.as_ptr(),
            );
            let previous_font = SelectObject(dc, font);
            SetBkMode(dc, TRANSPARENT as i32);
            SetTextColor(dc, rgb(245, 246, 248));
            let mut area = RECT {
                left: px(38, dpi),
                top: 0,
                right: rect.right - px(14, dpi),
                bottom: rect.bottom,
            };
            DrawTextW(
                dc,
                text.as_ptr(),
                text.len() as i32,
                &mut area,
                DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS,
            );
            SelectObject(dc, previous_font);
            DeleteObject(font);
        });
        EndPaint(window, &paint);
    }
    0
}
