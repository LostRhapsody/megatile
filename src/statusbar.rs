//! Visual workspace status bar indicator.
//!
//! Displays a floating bar showing the current workspace with dot indicators.
//! The bar uses the system accent color and has a rounded appearance.

use std::sync::OnceLock;
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreatePen, CreateRoundRectRgn, CreateSolidBrush, DeleteObject, Ellipse, EndPaint,
    HBRUSH, HDC, HPEN, InvalidateRect, PAINTSTRUCT, PS_SOLID, RoundRect, SelectObject,
    SetWindowRgn,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, DestroyWindow, GWLP_USERDATA,
    GetClientRect, GetWindowLongPtrW, HMENU, HWND_TOPMOST, IDC_ARROW, LoadCursorW, RegisterClassW,
    SW_HIDE, SW_SHOW, SWP_NOACTIVATE, SetWindowLongPtrW, SetWindowPos, ShowWindow, WINDOW_EX_STYLE,
    WINDOW_STYLE, WM_ERASEBKGND, WM_NCDESTROY, WM_PAINT, WNDCLASSW, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP, WS_VISIBLE,
};
use windows::core::{BOOL, PCWSTR, w};

use crate::windows_lib::get_accent_color;

/// Maximum number of workspaces supported.
pub const STATUSBAR_MAX_WORKSPACES: u8 = 9;
/// Number of workspace dots visible at once (scrolling window).
pub const STATUSBAR_VISIBLE_DOTS: u8 = 5;
/// Height of the status bar in pixels.
pub const STATUSBAR_HEIGHT: i32 = 16;
/// Width of the status bar in pixels.
pub const STATUSBAR_WIDTH: i32 = 150;
/// Gap above the status bar.
pub const STATUSBAR_TOP_GAP: i32 = 2;
/// Gap below the status bar.
pub const STATUSBAR_BOTTOM_GAP: i32 = 4;
/// Total vertical space reserved for the status bar area.
pub const STATUSBAR_VERTICAL_RESERVE: i32 =
    STATUSBAR_TOP_GAP + STATUSBAR_HEIGHT + STATUSBAR_BOTTOM_GAP;

const DOT_DIAMETER: i32 = 8;
const DOT_SPACING: i32 = 24;
const CORNER_RADIUS: i32 = 12;
const PADDING: i32 = 10;
const DEFAULT_ACCENT_COLOR: u32 = 0x007A7A7A;
const INACTIVE_GREY: u8 = 180;

static STATUSBAR_CLASS: OnceLock<Result<(), String>> = OnceLock::new();
const STATUSBAR_CLASS_NAME: PCWSTR = w!("MegaTileStatusBar");

/// Internal state for status bar rendering.
#[derive(Debug)]
struct StatusBarState {
    active_workspace: u8,
    total_workspaces: u8,
    accent_color: u32,
}

/// A floating status bar showing workspace indicators.
pub struct StatusBar {
    /// Window handle for the status bar.
    hwnd: HWND,
    /// Rendering state (boxed to allow passing pointer to window).
    state: Box<StatusBarState>,
}

impl StatusBar {
    /// Creates a new status bar owned by the given window.
    pub fn new(owner_hwnd: HWND) -> Result<Self, String> {
        let hinstance = unsafe {
            GetModuleHandleW(None).map_err(|e| format!("Failed to get module handle: {}", e))
        }?;
        ensure_class(hinstance.into())?;

        let accent_color = get_accent_color().unwrap_or(DEFAULT_ACCENT_COLOR);
        let state = Box::new(StatusBarState {
            active_workspace: 1,
            total_workspaces: STATUSBAR_VISIBLE_DOTS,
            accent_color,
        });

        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(WS_EX_TOPMOST.0 | WS_EX_TOOLWINDOW.0 | WS_EX_NOACTIVATE.0),
                STATUSBAR_CLASS_NAME,
                w!(""),
                WINDOW_STYLE(WS_POPUP.0 | WS_VISIBLE.0),
                0,
                0,
                STATUSBAR_WIDTH,
                STATUSBAR_HEIGHT,
                Some(owner_hwnd),
                Some(HMENU::default()),
                Some(hinstance.into()),
                None,
            )
            .map_err(|e| format!("Failed to create status bar window: {}", e))?
        };

        let mut statusbar = StatusBar { hwnd, state };
        statusbar.sync_state_pointer();
        statusbar.update_region(STATUSBAR_WIDTH, STATUSBAR_HEIGHT);
        Ok(statusbar)
    }

    /// Sets the position and size of the status bar.
    pub fn set_position(&self, x: i32, y: i32, width: i32, height: i32) {
        unsafe {
            let _ = SetWindowPos(
                self.hwnd,
                Some(HWND_TOPMOST),
                x,
                y,
                width,
                height,
                SWP_NOACTIVATE,
            );
            self.update_region(width, height);
        }
    }

    /// Updates the workspace indicator display.
    pub fn update_indicator(&mut self, active_workspace: u8, total_workspaces: u8) {
        self.state.active_workspace = active_workspace.clamp(1, STATUSBAR_MAX_WORKSPACES);
        self.state.total_workspaces = total_workspaces.clamp(1, STATUSBAR_MAX_WORKSPACES);
        if let Ok(color) = get_accent_color() {
            self.state.accent_color = color;
        }
        unsafe {
            let _ = InvalidateRect(Some(self.hwnd), None, BOOL(0).into());
        }
    }

    /// Shows the status bar.
    pub fn show(&self) {
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_SHOW);
        }
    }

    /// Hides the status bar.
    pub fn hide(&self) {
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_HIDE);
        }
    }

    fn sync_state_pointer(&mut self) {
        let ptr = self.state.as_mut() as *mut StatusBarState as isize;
        unsafe {
            SetWindowLongPtrW(self.hwnd, GWLP_USERDATA, ptr);
        }
    }

    fn update_region(&self, width: i32, height: i32) {
        unsafe {
            let region = CreateRoundRectRgn(0, 0, width, height, CORNER_RADIUS, CORNER_RADIUS);
            let _ = SetWindowRgn(self.hwnd, Some(region), BOOL(1).into());
        }
    }
}

impl Drop for StatusBar {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

fn ensure_class(hinstance: HINSTANCE) -> Result<(), String> {
    STATUSBAR_CLASS
        .get_or_init(|| unsafe {
            let wc = WNDCLASSW {
                lpfnWndProc: Some(statusbar_wnd_proc),
                hInstance: hinstance,
                lpszClassName: STATUSBAR_CLASS_NAME,
                hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
                style: CS_HREDRAW | CS_VREDRAW,
                ..Default::default()
            };

            if RegisterClassW(&wc) == 0 {
                Err("Failed to register status bar window class".to_string())
            } else {
                Ok(())
            }
        })
        .clone()
}

extern "system" fn statusbar_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match msg {
            WM_PAINT => {
                paint_statusbar(hwnd);
                return LRESULT(0);
            }
            WM_ERASEBKGND => return LRESULT(1),
            WM_NCDESTROY => {
                let _ = SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            }
            _ => {}
        }

        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}

unsafe fn paint_statusbar(hwnd: HWND) {
    unsafe {
        let state_ptr = get_state_ptr(hwnd);
        if state_ptr.is_null() {
            return;
        }

        let state = &*state_ptr;

        let mut ps = PAINTSTRUCT::default();
        let hdc = BeginPaint(hwnd, &mut ps);
        if hdc.0.is_null() {
            return;
        }

        let mut rect = RECT::default();
        let _ = GetClientRect(hwnd, &mut rect);

        draw_background(hdc, &rect, state.accent_color);
        draw_workspace_dots(hdc, &rect, state);

        let _ = EndPaint(hwnd, &ps);
    };
}

unsafe fn draw_background(hdc: HDC, rect: &RECT, accent_color: u32) {
    let bg_color = subtle_background(accent_color);
    unsafe {
        let brush = CreateSolidBrush(COLORREF(bg_color));
        let pen = CreatePen(PS_SOLID, 1, COLORREF(accent_color));

        let old_pen = SelectObject(hdc, pen.into());
        let old_brush = SelectObject(hdc, brush.into());

        let _ = RoundRect(
            hdc,
            rect.left,
            rect.top,
            rect.right,
            rect.bottom,
            CORNER_RADIUS,
            CORNER_RADIUS,
        );

        SelectObject(hdc, old_pen);
        SelectObject(hdc, old_brush);
        let _ = DeleteObject(pen.into());
        let _ = DeleteObject(brush.into());
    };
}

unsafe fn draw_workspace_dots(hdc: HDC, rect: &RECT, state: &StatusBarState) {
    let total = state.total_workspaces.min(STATUSBAR_MAX_WORKSPACES);
    let visible = total.min(STATUSBAR_VISIBLE_DOTS);
    let (start, _) = workspace_window_range(state.active_workspace, total, visible);

    let content_width = DOT_DIAMETER + (visible as i32 - 1) * DOT_SPACING;
    let available = rect.right - rect.left;
    let mut start_x = (available - content_width) / 2;
    if start_x < PADDING {
        start_x = PADDING;
    }

    let center_y = rect.top + (rect.bottom - rect.top - DOT_DIAMETER) / 2;
    let inactive = inactive_dot_color(state.accent_color);

    for i in 0..visible {
        let workspace_id = start + i;
        let color = if workspace_id == state.active_workspace {
            state.accent_color
        } else {
            inactive
        };

        let x = start_x + (i as i32) * DOT_SPACING;

        unsafe {
            let brush: HBRUSH = CreateSolidBrush(COLORREF(color));
            let pen: HPEN = CreatePen(PS_SOLID, 1, COLORREF(color));
            let old_pen = SelectObject(hdc, pen.into());
            let old_brush = SelectObject(hdc, brush.into());

            let _ = Ellipse(hdc, x, center_y, x + DOT_DIAMETER, center_y + DOT_DIAMETER);

            SelectObject(hdc, old_pen);
            SelectObject(hdc, old_brush);
            let _ = DeleteObject(pen.into());
            let _ = DeleteObject(brush.into());
        }
    }
}

fn workspace_window_range(active: u8, total: u8, visible: u8) -> (u8, u8) {
    let total = total as i32;
    let visible = visible as i32;
    let active = active as i32;

    let start = if total <= visible || active <= 3 {
        1
    } else if active > total - 2 {
        total - visible + 1
    } else {
        active - 2
    };

    let start = start.max(1) as u8;
    (start, start + visible as u8 - 1)
}

fn subtle_background(accent_color: u32) -> u32 {
    let (r, g, b) = split_color(accent_color);
    compose_color(
        blend_channel(r, 230, 0.35),
        blend_channel(g, 230, 0.35),
        blend_channel(b, 230, 0.35),
    )
}

fn inactive_dot_color(accent_color: u32) -> u32 {
    let (r, g, b) = split_color(accent_color);
    compose_color(
        blend_channel(r, INACTIVE_GREY, 0.25),
        blend_channel(g, INACTIVE_GREY, 0.25),
        blend_channel(b, INACTIVE_GREY, 0.25),
    )
}

fn blend_channel(active: u8, target: u8, ratio: f32) -> u8 {
    ((active as f32 * ratio) + (target as f32 * (1.0 - ratio))).round() as u8
}

fn split_color(color: u32) -> (u8, u8, u8) {
    let r = (color & 0xFF) as u8;
    let g = ((color >> 8) & 0xFF) as u8;
    let b = ((color >> 16) & 0xFF) as u8;
    (r, g, b)
}

fn compose_color(r: u8, g: u8, b: u8) -> u32 {
    (b as u32) << 16 | (g as u32) << 8 | r as u32
}

unsafe fn get_state_ptr(hwnd: HWND) -> *mut StatusBarState {
    let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) };
    if ptr == 0 {
        std::ptr::null_mut()
    } else {
        ptr as *mut StatusBarState
    }
}
