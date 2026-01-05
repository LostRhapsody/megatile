//! Visual workspace status bar indicator.
//!
//! Displays a floating bar showing workspace indicators with numbers,
//! and the current date/time. Uses the system accent color with a dimmed backdrop.

use std::sync::OnceLock;
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateFontW, CreatePen, CreateRoundRectRgn, CreateSolidBrush, DeleteObject,
    Ellipse, EndPaint, HBRUSH, HDC, HFONT, HPEN, InvalidateRect, PAINTSTRUCT, PS_SOLID, RoundRect,
    SelectObject, SetBkMode, SetTextColor, SetWindowRgn, TRANSPARENT, TextOutW,
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
/// Height of the status bar in pixels.
pub const STATUSBAR_HEIGHT: i32 = 34;
/// Width of the status bar in pixels.
pub const STATUSBAR_WIDTH: i32 = 360;
/// Gap above the status bar.
pub const STATUSBAR_TOP_GAP: i32 = 2;
/// Gap below the status bar.
pub const STATUSBAR_BOTTOM_GAP: i32 = 2;
/// Total vertical space reserved for the status bar area.
pub const STATUSBAR_VERTICAL_RESERVE: i32 =
    STATUSBAR_TOP_GAP + STATUSBAR_HEIGHT + STATUSBAR_BOTTOM_GAP;

const DOT_DIAMETER: i32 = 20;
const DOT_SPACING: i32 = 26;
const CORNER_RADIUS: i32 = 32;
const PADDING_LEFT: i32 = 16;
const PADDING_RIGHT: i32 = 16;
const PADDING_VERTICAL: i32 = 7;
const DEFAULT_ACCENT_COLOR: u32 = 0x007A7A7A;
const ALWAYS_SHOW_WORKSPACES: u8 = 5; // Workspaces 1-5 always shown

static STATUSBAR_CLASS: OnceLock<Result<(), String>> = OnceLock::new();
const STATUSBAR_CLASS_NAME: PCWSTR = w!("MegaTileStatusBar");

/// Internal state for status bar rendering.
#[derive(Debug)]
struct StatusBarState {
    active_workspace: u8,
    total_workspaces: u8,
    accent_color: u32,
    /// Cached time string for display.
    time_string: [u16; 16],
    time_string_len: usize,
    /// Bitmask for workspaces 6-9 that have windows (bit 0 = ws6, bit 1 = ws7, etc)
    occupied_workspaces_6_9: u8,
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
        let mut state = Box::new(StatusBarState {
            active_workspace: 1,
            total_workspaces: STATUSBAR_MAX_WORKSPACES,
            accent_color,
            time_string: [0u16; 16],
            time_string_len: 0,
            occupied_workspaces_6_9: 0,
        });
        update_time_string(&mut state);

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
    ///
    /// # Arguments
    /// * `active_workspace` - Currently active workspace (1-9)
    /// * `total_workspaces` - Total number of workspaces (1-9)
    /// * `occupied_6_9` - Bitmask for workspaces 6-9 occupancy (bit 0=ws6, bit 1=ws7, bit 2=ws8, bit 3=ws9)
    pub fn update_indicator(
        &mut self,
        active_workspace: u8,
        total_workspaces: u8,
        occupied_6_9: u8,
    ) {
        self.state.active_workspace = active_workspace.clamp(1, STATUSBAR_MAX_WORKSPACES);
        self.state.total_workspaces = total_workspaces.clamp(1, STATUSBAR_MAX_WORKSPACES);
        self.state.occupied_workspaces_6_9 = occupied_6_9;
        if let Ok(color) = get_accent_color() {
            self.state.accent_color = color;
        }
        update_time_string(&mut self.state);
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

/// Updates the time string in the state with current time.
fn update_time_string(state: &mut StatusBarState) {
    use std::time::{SystemTime, UNIX_EPOCH};

    // Get current time
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Convert to local time components (simple UTC offset approximation)
    // In a real implementation, you'd use proper timezone handling
    let secs_per_day = 86400u64;
    let secs_per_hour = 3600u64;
    let secs_per_min = 60u64;

    // Days since Unix epoch
    let days = now / secs_per_day;
    let time_of_day = now % secs_per_day;

    let hours = (time_of_day / secs_per_hour) % 24;
    let minutes = (time_of_day % secs_per_hour) / secs_per_min;

    // Calculate date (simplified - days since 1970-01-01)
    // This is a simplified calculation that doesn't account for leap years perfectly
    let mut remaining_days = days as i64;
    let mut year = 1970i32;

    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        year += 1;
    }

    let days_in_months: [i64; 12] = if is_leap_year(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut month = 1u32;
    for days_in_month in days_in_months.iter() {
        if remaining_days < *days_in_month {
            break;
        }
        remaining_days -= days_in_month;
        month += 1;
    }
    let day = remaining_days + 1;

    // Format: "HH:MM DD/MM"
    let time_str = format!("{:02}:{:02} {:02}/{:02}", hours, minutes, day, month);

    // Convert to UTF-16
    let mut i = 0;
    for ch in time_str.encode_utf16() {
        if i < state.time_string.len() {
            state.time_string[i] = ch;
            i += 1;
        }
    }
    state.time_string_len = i;
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
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
        draw_time(hdc, &rect, state);

        let _ = EndPaint(hwnd, &ps);
    };
}

unsafe fn draw_background(hdc: HDC, rect: &RECT, accent_color: u32) {
    let bg_color = dimmed_desaturated_background(accent_color);
    unsafe {
        let brush = CreateSolidBrush(COLORREF(bg_color));
        // Use a slightly darker border
        let border_color = darken_color(bg_color, 0.85);
        let pen = CreatePen(PS_SOLID, 2, COLORREF(border_color));

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
    // Determine which workspaces to display
    let mut workspaces_to_show = Vec::with_capacity(9);

    // Always show workspaces 1-5
    for i in 1..=ALWAYS_SHOW_WORKSPACES {
        workspaces_to_show.push(i);
    }

    // Conditionally show workspaces 6-9 if they have windows
    if state.occupied_workspaces_6_9 & 0x01 != 0 {
        workspaces_to_show.push(6);
    }
    if state.occupied_workspaces_6_9 & 0x02 != 0 {
        workspaces_to_show.push(7);
    }
    if state.occupied_workspaces_6_9 & 0x04 != 0 {
        workspaces_to_show.push(8);
    }
    if state.occupied_workspaces_6_9 & 0x08 != 0 {
        workspaces_to_show.push(9);
    }

    // Start at left with padding
    let start_x = rect.left + PADDING_LEFT;
    let center_y = rect.top + PADDING_VERTICAL;

    // Create font for workspace numbers
    let font = unsafe { create_small_font() };
    let old_font = unsafe { SelectObject(hdc, font.into()) };
    let _ = unsafe { SetBkMode(hdc, TRANSPARENT) };

    for (index, workspace_id) in workspaces_to_show.iter().enumerate() {
        let x = start_x + (index as i32) * DOT_SPACING;
        let is_active = *workspace_id == state.active_workspace;

        // Draw the dot
        let (dot_color, text_color) = if is_active {
            // Active: opaque accent color dot, no text
            (state.accent_color, state.accent_color)
        } else {
            // Inactive: semi-transparent dot, muted gray text
            (
                semi_transparent_dot_color(state.accent_color),
                0x00888888_u32,
            )
        };

        unsafe {
            let brush: HBRUSH = CreateSolidBrush(COLORREF(dot_color));
            let pen: HPEN = CreatePen(PS_SOLID, 1, COLORREF(dot_color));
            let old_pen = SelectObject(hdc, pen.into());
            let old_brush = SelectObject(hdc, brush.into());

            let _ = Ellipse(hdc, x, center_y, x + DOT_DIAMETER, center_y + DOT_DIAMETER);

            SelectObject(hdc, old_pen);
            SelectObject(hdc, old_brush);
            let _ = DeleteObject(pen.into());
            let _ = DeleteObject(brush.into());

            // Draw the workspace number inside the dot (centered)
            let _ = SetTextColor(hdc, COLORREF(text_color));
            let num_char = [b'0' as u16 + *workspace_id as u16];
            // Center the number in the dot
            let text_x = x + (DOT_DIAMETER - 5) / 2;
            let text_y = center_y + (DOT_DIAMETER - 14) / 2;
            let _ = TextOutW(hdc, text_x, text_y, &num_char);
        }
    }

    unsafe { SelectObject(hdc, old_font) };
    let _ = unsafe { DeleteObject(font.into()) };
}

unsafe fn draw_time(hdc: HDC, rect: &RECT, state: &StatusBarState) {
    if state.time_string_len == 0 {
        return;
    }

    // Create font for time display
    let font = unsafe { create_med_font() };
    let old_font = unsafe { SelectObject(hdc, font.into()) };
    let _ = unsafe { SetBkMode(hdc, TRANSPARENT) };
    // Use a muted color for the time text (not bright white)
    let _ = unsafe { SetTextColor(hdc, COLORREF(0x00AAAAAA)) };

    // Position time at far right
    let time_width = (state.time_string_len as i32) * 7; // Approximate character width
    let x = rect.right - PADDING_RIGHT - time_width;
    let y = rect.top + PADDING_VERTICAL + (DOT_DIAMETER - 22) / 2;

    unsafe {
        let _ = TextOutW(hdc, x, y, &state.time_string[..state.time_string_len]);
    }

    unsafe { SelectObject(hdc, old_font) };
    let _ = unsafe { DeleteObject(font.into()) };
}

unsafe fn create_small_font() -> HFONT {
    unsafe {
        CreateFontW(
            14,                                                      // Height (adjusted for better rendering)
            0,                                                       // Width (0 = auto)
            0,                                                       // Escapement
            0,                                                       // Orientation
            400,                                                     // Weight (400 = normal)
            0,                                                       // Italic
            0,                                                       // Underline
            0,                                                       // StrikeOut
            windows::Win32::Graphics::Gdi::FONT_CHARSET(0),          // CharSet
            windows::Win32::Graphics::Gdi::FONT_OUTPUT_PRECISION(3), // OutPrecision (3 = TT_ONLY for smoother rendering)
            windows::Win32::Graphics::Gdi::FONT_CLIP_PRECISION(0),   // ClipPrecision
            windows::Win32::Graphics::Gdi::FONT_QUALITY(5), // Quality (5 = CLEARTYPE_QUALITY for smoother rendering)
            0,                                              // PitchAndFamily
            w!("Segoe UI"),                                 // Face name
        )
    }
}

unsafe fn create_med_font() -> HFONT {
    unsafe {
        CreateFontW(
            20,                                                      // Height (adjusted for better rendering)
            0,                                                       // Width (0 = auto)
            0,                                                       // Escapement
            0,                                                       // Orientation
            400,                                                     // Weight (400 = normal)
            0,                                                       // Italic
            0,                                                       // Underline
            0,                                                       // StrikeOut
            windows::Win32::Graphics::Gdi::FONT_CHARSET(0),          // CharSet
            windows::Win32::Graphics::Gdi::FONT_OUTPUT_PRECISION(3), // OutPrecision (3 = TT_ONLY for smoother rendering)
            windows::Win32::Graphics::Gdi::FONT_CLIP_PRECISION(0),   // ClipPrecision
            windows::Win32::Graphics::Gdi::FONT_QUALITY(5), // Quality (5 = CLEARTYPE_QUALITY for smoother rendering)
            0,                                              // PitchAndFamily
            w!("Segoe UI"),                                 // Face name
        )
    }
}

/// Creates a dimmed and desaturated version of the accent color for the background.
fn dimmed_desaturated_background(accent_color: u32) -> u32 {
    let (r, g, b) = split_color(accent_color);

    // Convert to grayscale-ish by averaging with gray
    let gray = ((r as u32 + g as u32 + b as u32) / 3) as u8;

    // Blend towards gray (desaturate) and darken
    let desaturate_factor = 0.6_f32; // More desaturation
    let darken_factor = 0.35_f32; // Slightly darker

    let dr = blend_channel(r, gray, desaturate_factor);
    let dg = blend_channel(g, gray, desaturate_factor);
    let db = blend_channel(b, gray, desaturate_factor);

    // Then darken
    let fr = (dr as f32 * darken_factor) as u8;
    let fg = (dg as f32 * darken_factor) as u8;
    let fb = (db as f32 * darken_factor) as u8;

    compose_color(fr, fg, fb)
}

/// Creates a semi-transparent looking dot color for inactive workspaces.
fn semi_transparent_dot_color(accent_color: u32) -> u32 {
    let (r, g, b) = split_color(accent_color);

    // Blend with a lighter gray to simulate transparency
    let target = 190u8;
    compose_color(
        blend_channel(r, target, 0.25),
        blend_channel(g, target, 0.25),
        blend_channel(b, target, 0.25),
    )
}

/// Darkens a color by a factor (0.0 = black, 1.0 = unchanged).
fn darken_color(color: u32, factor: f32) -> u32 {
    let (r, g, b) = split_color(color);
    compose_color(
        (r as f32 * factor) as u8,
        (g as f32 * factor) as u8,
        (b as f32 * factor) as u8,
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
