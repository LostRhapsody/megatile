//! Visual workspace status bar indicator.
//!
//! Displays a floating bar showing workspace indicators with numbers,
//! and the current date/time. Uses the system accent color with a dimmed backdrop.
//! Renders using GDI+ for anti-aliased edges and smooth rendering.

use std::sync::OnceLock;

use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, RECT, SYSTEMTIME, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateRoundRectRgn, EndPaint, InvalidateRect, PAINTSTRUCT, SetWindowRgn,
};
use windows::Win32::Graphics::GdiPlus::{
    FillMode, GdipAddPathArc, GdipAddPathLine, GdipClosePathFigure, GdipCreateFont,
    GdipCreateFontFamilyFromName, GdipCreateFromHDC, GdipCreatePath, GdipCreatePen1,
    GdipCreateSolidFill, GdipCreateStringFormat, GdipDeleteBrush, GdipDeleteFont,
    GdipDeleteFontFamily, GdipDeleteGraphics, GdipDeletePath, GdipDeletePen,
    GdipDeleteStringFormat, GdipDrawPath, GdipDrawString, GdipFillEllipse, GdipFillPath,
    GdipSetSmoothingMode, GdipSetStringFormatAlign, GdipSetStringFormatLineAlign,
    GdipSetTextRenderingHint, GdipStartPathFigure, GdiplusShutdown, GdiplusStartup,
    GdiplusStartupInput, GpBrush, GpFontFamily, GpGraphics, GpPath, GpPen, GpSolidFill,
    GpStringFormat, SmoothingModeHighQuality, StringAlignmentCenter,
    TextRenderingHintClearTypeGridFit, Unit,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::SystemInformation::GetLocalTime;
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

/// GDI+ token for initialization/shutdown.
static mut GDIPLUS_TOKEN: usize = 0;

/// Internal state for status bar rendering.
#[derive(Debug)]
struct StatusBarState {
    active_workspace: u8,
    total_workspaces: u8,
    accent_color: u32,
    /// Cached time string for display.
    time_string: String,
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

/// Initializes GDI+. Must be called before creating any StatusBar.
pub fn init_gdiplus() -> Result<(), String> {
    unsafe {
        let input = GdiplusStartupInput {
            GdiplusVersion: 1,
            DebugEventCallback: 0,
            SuppressBackgroundThread: BOOL(0),
            SuppressExternalCodecs: BOOL(0),
        };
        let mut token: usize = 0;
        let status = GdiplusStartup(&mut token, &input, std::ptr::null_mut());
        if status.0 != 0 {
            return Err(format!("GdiplusStartup failed with status: {}", status.0));
        }
        GDIPLUS_TOKEN = token;
        Ok(())
    }
}

/// Shuts down GDI+. Call when application exits.
pub fn shutdown_gdiplus() {
    unsafe {
        if GDIPLUS_TOKEN != 0 {
            GdiplusShutdown(GDIPLUS_TOKEN);
            GDIPLUS_TOKEN = 0;
        }
    }
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
            time_string: String::new(),
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

/// Updates the time string in the state with current local time.
fn update_time_string(state: &mut StatusBarState) {
    let st: SYSTEMTIME = unsafe { GetLocalTime() };

    // Format: "HH:MM DD/MM"
    state.time_string = format!(
        "{:02}:{:02} {:02}/{:02}",
        st.wHour, st.wMinute, st.wDay, st.wMonth
    );
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

        // Create GDI+ Graphics object from HDC
        let mut graphics: *mut GpGraphics = std::ptr::null_mut();
        if GdipCreateFromHDC(hdc, &mut graphics).0 != 0 || graphics.is_null() {
            let _ = EndPaint(hwnd, &ps);
            return;
        }

        // Enable anti-aliasing
        let _ = GdipSetSmoothingMode(graphics, SmoothingModeHighQuality);
        let _ = GdipSetTextRenderingHint(graphics, TextRenderingHintClearTypeGridFit);

        draw_background_gdiplus(graphics, &rect, state.accent_color);
        draw_workspace_dots_gdiplus(graphics, &rect, state);
        draw_time_gdiplus(graphics, &rect, state);

        // Cleanup
        GdipDeleteGraphics(graphics);
        let _ = EndPaint(hwnd, &ps);
    }
}

unsafe fn draw_background_gdiplus(graphics: *mut GpGraphics, rect: &RECT, accent_color: u32) {
    unsafe {
        let bg_color = dimmed_desaturated_background(accent_color);
        let (r, g, b) = split_color(bg_color);

        // Create fill brush for background
        let mut brush: *mut GpSolidFill = std::ptr::null_mut();
        let argb_bg = make_argb(255, r, g, b);
        if GdipCreateSolidFill(argb_bg, &mut brush).0 != 0 {
            return;
        }

        // Create rounded rectangle path for fill
        let x = rect.left as f32;
        let y = rect.top as f32;
        let width = (rect.right - rect.left) as f32;
        let height = (rect.bottom - rect.top) as f32;
        let radius = CORNER_RADIUS as f32 / 2.0;

        let fill_path = create_rounded_rect_path(x, y, width, height, radius);
        if !fill_path.is_null() {
            let _ = GdipFillPath(graphics, brush as *mut GpBrush, fill_path);
            GdipDeletePath(fill_path);
        }

        GdipDeleteBrush(brush as *mut GpBrush);

        // Draw border with rounded rectangle path
        let border_color = darken_color(bg_color, 0.85);
        let (br, bg_c, bb) = split_color(border_color);
        let mut pen: *mut GpPen = std::ptr::null_mut();
        let argb_border = make_argb(255, br, bg_c, bb);
        if GdipCreatePen1(argb_border, 2.0, Unit(0), &mut pen).0 == 0 {
            // Inset the border slightly
            let border_path =
                create_rounded_rect_path(x + 1.0, y + 1.0, width - 2.0, height - 2.0, radius);
            if !border_path.is_null() {
                let _ = GdipDrawPath(graphics, pen, border_path);
                GdipDeletePath(border_path);
            }
            GdipDeletePen(pen);
        }
    }
}

/// Creates a GDI+ path for a rounded rectangle.
unsafe fn create_rounded_rect_path(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    radius: f32,
) -> *mut GpPath {
    unsafe {
        let mut path: *mut GpPath = std::ptr::null_mut();
        // FillModeAlternate = 0
        if GdipCreatePath(FillMode(0), &mut path).0 != 0 || path.is_null() {
            return std::ptr::null_mut();
        }

        let diameter = radius * 2.0;

        // Start the path figure
        let _ = GdipStartPathFigure(path);

        // Top-left arc
        let _ = GdipAddPathArc(path, x, y, diameter, diameter, 180.0, 90.0);

        // Top edge
        let _ = GdipAddPathLine(path, x + radius, y, x + width - radius, y);

        // Top-right arc
        let _ = GdipAddPathArc(
            path,
            x + width - diameter,
            y,
            diameter,
            diameter,
            270.0,
            90.0,
        );

        // Right edge
        let _ = GdipAddPathLine(path, x + width, y + radius, x + width, y + height - radius);

        // Bottom-right arc
        let _ = GdipAddPathArc(
            path,
            x + width - diameter,
            y + height - diameter,
            diameter,
            diameter,
            0.0,
            90.0,
        );

        // Bottom edge
        let _ = GdipAddPathLine(path, x + width - radius, y + height, x + radius, y + height);

        // Bottom-left arc
        let _ = GdipAddPathArc(
            path,
            x,
            y + height - diameter,
            diameter,
            diameter,
            90.0,
            90.0,
        );

        // Left edge (back to start)
        let _ = GdipAddPathLine(path, x, y + height - radius, x, y + radius);

        // Close the figure
        let _ = GdipClosePathFigure(path);

        path
    }
}

unsafe fn draw_workspace_dots_gdiplus(
    graphics: *mut GpGraphics,
    rect: &RECT,
    state: &StatusBarState,
) {
    unsafe {
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
        let font_family = create_font_family();
        let font = create_font(font_family, 10.0);
        let string_format = create_centered_string_format();

        for (index, workspace_id) in workspaces_to_show.iter().enumerate() {
            let x = start_x + (index as i32) * DOT_SPACING;
            let is_active = *workspace_id == state.active_workspace;

            // Get dot color and text color
            let (dot_color, text_color) = if is_active {
                (state.accent_color, state.accent_color) // Active: accent color, text same (hidden)
            } else {
                (
                    semi_transparent_dot_color(state.accent_color),
                    0x00888888_u32,
                )
            };

            // Draw the ellipse (dot)
            let (dr, dg, db) = split_color(dot_color);
            let mut dot_brush: *mut GpSolidFill = std::ptr::null_mut();
            if GdipCreateSolidFill(make_argb(255, dr, dg, db), &mut dot_brush).0 == 0 {
                let _ = GdipFillEllipse(
                    graphics,
                    dot_brush as *mut GpBrush,
                    x as f32,
                    center_y as f32,
                    DOT_DIAMETER as f32,
                    DOT_DIAMETER as f32,
                );
                GdipDeleteBrush(dot_brush as *mut GpBrush);
            }

            // Draw the workspace number inside the dot
            if !font.is_null() && !string_format.is_null() {
                let (tr, tg, tb) = split_color(text_color);
                let mut text_brush: *mut GpSolidFill = std::ptr::null_mut();
                if GdipCreateSolidFill(make_argb(255, tr, tg, tb), &mut text_brush).0 == 0 {
                    let num_str: Vec<u16> = format!("{}", workspace_id)
                        .encode_utf16()
                        .chain(std::iter::once(0))
                        .collect();

                    // Create a rect for the text centered in the dot
                    let text_rect = windows::Win32::Graphics::GdiPlus::RectF {
                        X: x as f32,
                        Y: center_y as f32,
                        Width: DOT_DIAMETER as f32,
                        Height: DOT_DIAMETER as f32,
                    };

                    let _ = GdipDrawString(
                        graphics,
                        PCWSTR::from_raw(num_str.as_ptr()),
                        -1,
                        font,
                        &text_rect,
                        string_format,
                        text_brush as *mut GpBrush,
                    );
                    GdipDeleteBrush(text_brush as *mut GpBrush);
                }
            }
        }

        // Cleanup
        if !string_format.is_null() {
            GdipDeleteStringFormat(string_format);
        }
        if !font.is_null() {
            GdipDeleteFont(font);
        }
        if !font_family.is_null() {
            GdipDeleteFontFamily(font_family);
        }
    }
}

unsafe fn draw_time_gdiplus(graphics: *mut GpGraphics, rect: &RECT, state: &StatusBarState) {
    unsafe {
        if state.time_string.is_empty() {
            return;
        }

        // Create font for time display
        let font_family = create_font_family();
        let font = create_font(font_family, 12.0);
        let string_format = create_right_aligned_string_format();

        if font.is_null() || string_format.is_null() {
            if !string_format.is_null() {
                GdipDeleteStringFormat(string_format);
            }
            if !font.is_null() {
                GdipDeleteFont(font);
            }
            if !font_family.is_null() {
                GdipDeleteFontFamily(font_family);
            }
            return;
        }

        // Use a muted color for the time text
        let mut text_brush: *mut GpSolidFill = std::ptr::null_mut();
        if GdipCreateSolidFill(make_argb(255, 0xAA, 0xAA, 0xAA), &mut text_brush).0 != 0 {
            GdipDeleteStringFormat(string_format);
            GdipDeleteFont(font);
            GdipDeleteFontFamily(font_family);
            return;
        }

        let time_str: Vec<u16> = state
            .time_string
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        // Position time at far right
        let text_rect = windows::Win32::Graphics::GdiPlus::RectF {
            X: (rect.right - PADDING_RIGHT - 100) as f32,
            Y: (rect.top + PADDING_VERTICAL) as f32,
            Width: 100.0,
            Height: DOT_DIAMETER as f32,
        };

        let _ = GdipDrawString(
            graphics,
            PCWSTR::from_raw(time_str.as_ptr()),
            -1,
            font,
            &text_rect,
            string_format,
            text_brush as *mut GpBrush,
        );

        // Cleanup
        GdipDeleteBrush(text_brush as *mut GpBrush);
        GdipDeleteStringFormat(string_format);
        GdipDeleteFont(font);
        GdipDeleteFontFamily(font_family);
    }
}

unsafe fn create_font_family() -> *mut GpFontFamily {
    unsafe {
        let mut font_family: *mut GpFontFamily = std::ptr::null_mut();
        let family_name: Vec<u16> = "Segoe UI"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let _ = GdipCreateFontFamilyFromName(
            PCWSTR::from_raw(family_name.as_ptr()),
            std::ptr::null_mut(),
            &mut font_family,
        );
        font_family
    }
}

unsafe fn create_font(
    font_family: *mut GpFontFamily,
    size: f32,
) -> *mut windows::Win32::Graphics::GdiPlus::GpFont {
    unsafe {
        if font_family.is_null() {
            return std::ptr::null_mut();
        }
        let mut font: *mut windows::Win32::Graphics::GdiPlus::GpFont = std::ptr::null_mut();
        // FontStyleRegular = 0, UnitPoint = 3
        let _ = GdipCreateFont(font_family, size, 0, Unit(3), &mut font);
        font
    }
}

unsafe fn create_centered_string_format() -> *mut GpStringFormat {
    unsafe {
        let mut format: *mut GpStringFormat = std::ptr::null_mut();
        if GdipCreateStringFormat(0, 0, &mut format).0 != 0 {
            return std::ptr::null_mut();
        }
        let _ = GdipSetStringFormatAlign(format, StringAlignmentCenter);
        let _ = GdipSetStringFormatLineAlign(format, StringAlignmentCenter);
        format
    }
}

unsafe fn create_right_aligned_string_format() -> *mut GpStringFormat {
    unsafe {
        let mut format: *mut GpStringFormat = std::ptr::null_mut();
        if GdipCreateStringFormat(0, 0, &mut format).0 != 0 {
            return std::ptr::null_mut();
        }
        // StringAlignmentFar = 2 for right alignment
        let _ = GdipSetStringFormatAlign(
            format,
            windows::Win32::Graphics::GdiPlus::StringAlignment(2),
        );
        let _ = GdipSetStringFormatLineAlign(format, StringAlignmentCenter);
        format
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

/// Creates an ARGB color value for GDI+.
fn make_argb(a: u8, r: u8, g: u8, b: u8) -> u32 {
    ((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

unsafe fn get_state_ptr(hwnd: HWND) -> *mut StatusBarState {
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
        if ptr == 0 {
            std::ptr::null_mut()
        } else {
            ptr as *mut StatusBarState
        }
    }
}
