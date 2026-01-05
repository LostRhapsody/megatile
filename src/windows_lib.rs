//! Windows API abstractions and window management utilities.
//!
//! This module provides safe wrappers around Windows API calls for:
//! - Window enumeration and filtering
//! - Window visibility and taskbar management
//! - Monitor enumeration
//! - Window decorations (borders, transparency)
//! - Window positioning and fullscreen management

use windows::Win32::Foundation::{
    COLORREF, GetLastError, HWND, LPARAM, RECT, SetLastError, TRUE, WIN32_ERROR, WPARAM,
};
use windows::Win32::Graphics::Dwm::*;
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO,
};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::BOOL;

const MONITORINFOF_PRIMARY: u32 = 1;
const DWMWA_BORDER_COLOR: DWMWINDOWATTRIBUTE = DWMWINDOWATTRIBUTE(34);
const DWMWA_COLOR_DEFAULT: u32 = 0xFFFFFFFF;
const LWA_ALPHA: LAYERED_WINDOW_ATTRIBUTES_FLAGS = LAYERED_WINDOW_ATTRIBUTES_FLAGS(2);

/// Information about a window retrieved from Windows API.
pub struct WindowInfo {
    pub hwnd: HWND,
    pub title: String,
    pub class_name: String,
    pub rect: RECT,
    #[allow(dead_code)]
    pub is_visible: bool,
    #[allow(dead_code)]
    pub is_minimized: bool,
}

/// Enumerates all top-level windows on the system.
pub fn enumerate_windows() -> Vec<WindowInfo> {
    let mut windows = Vec::new();

    unsafe {
        let lparam = LPARAM(&mut windows as *mut _ as isize);
        let _ = EnumWindows(Some(enum_windows_proc), lparam);
    }

    windows
}

unsafe extern "system" fn enum_windows_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let windows = unsafe { &mut *(lparam.0 as *mut Vec<WindowInfo>) };

    let title = get_window_title(hwnd);
    let class_name = get_window_class(hwnd);

    let mut rect = RECT::default();
    unsafe {
        let _ = GetWindowRect(hwnd, &mut rect);
    }

    let is_visible = unsafe { IsWindowVisible(hwnd).as_bool() };

    let is_minimized = unsafe { IsIconic(hwnd).as_bool() };

    windows.push(WindowInfo {
        hwnd,
        title,
        class_name,
        rect,
        is_visible,
        is_minimized,
    });

    TRUE
}

/// Gets the title text of a window.
pub fn get_window_title(hwnd: HWND) -> String {
    let mut title_buffer = [0u16; 256];
    let length = unsafe { GetWindowTextW(hwnd, &mut title_buffer) };
    String::from_utf16_lossy(&title_buffer[..length as usize])
}

/// Gets the window class name.
pub fn get_window_class(hwnd: HWND) -> String {
    let mut class_buffer = [0u16; 256];
    let class_len = unsafe { GetClassNameW(hwnd, &mut class_buffer) };
    String::from_utf16_lossy(&class_buffer[..class_len as usize])
}

/// Checks if a window handle represents a normal, manageable window.
pub fn is_normal_window_hwnd(hwnd: HWND) -> bool {
    let title = get_window_title(hwnd);
    let class_name = get_window_class(hwnd);
    let is_normal = is_normal_window(hwnd, &class_name, &title);
    println!("is normal? {}", is_normal);
    is_normal
}

/// Determines if a window is a "normal" window that should be managed.
///
/// Filters out system windows, tool windows, invisible windows, popups,
/// dialogs, and other windows that shouldn't be tiled (taskbar, shell windows, etc.).
pub fn is_normal_window(hwnd: HWND, class_name: &str, title: &str) -> bool {
    println!(
        "Checking if window, title {}, class name {}, hwnd {:?}, is 'normal'.",
        title, class_name, hwnd
    );
    unsafe {
        // Basic visibility check
        if !IsWindowVisible(hwnd).as_bool() {
            return false;
        }

        // Filter minimized windows
        if IsIconic(hwnd).as_bool() {
            return false;
        }

        // Verify the window handle is still valid
        if !IsWindow(Some(hwnd)).as_bool() {
            println!("Filtered: invalid window handle");
            return false;
        }

        // Filter specific problematic window titles
        let filtered_titles = [
            "Windows Input Experience",
            "Chrome Legacy Window",
            "OLEChannelWnd",
            "DesktopWindowXamlSource",
            "Non Client Input Sink Window",
            "Program Manager",
            "MSCTFIME UI",
            "Default IME",
            "GDI+ Window",
            "MediaContextNotificationWindow",
            "CicMarshalWnd",
        ];
        for filtered_title in &filtered_titles {
            if title == *filtered_title {
                println!("Filtered: problematic title {}", title);
                return false;
            }
        }

        // Filter empty titles (often system windows)
        if title.is_empty() {
            println!("Filtered: empty title");
            return false;
        }

        // Check for cloaked windows (hidden UWP apps, etc.)
        let mut cloaked = 0u32;
        let _ = DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            &mut cloaked as *mut _ as *mut std::ffi::c_void,
            std::mem::size_of::<u32>() as u32,
        );
        if cloaked != 0 {
            println!("Filtered: cloaked window");
            return false;
        }

        let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
        let style = GetWindowLongW(hwnd, GWL_STYLE) as u32;

        // Filter tool windows
        if ex_style & WS_EX_TOOLWINDOW.0 != 0 {
            println!("Filtered: tool window");
            return false;
        }

        // Filter non-activatable windows
        if ex_style & WS_EX_NOACTIVATE.0 != 0 {
            println!("Filtered: non-activatable");
            return false;
        }

        // Filter dialog modal frame windows (explicit dialogs)
        if ex_style & WS_EX_DLGMODALFRAME.0 != 0 {
            println!("Filtered: dialog modal frame");
            return false;
        }

        // Filter transparent layered windows with 0 alpha
        if ex_style & WS_EX_LAYERED.0 != 0 {
            let mut alpha: u8 = 255;
            let mut flags = LAYERED_WINDOW_ATTRIBUTES_FLAGS(0);
            let mut color = COLORREF(0);
            if GetLayeredWindowAttributes(
                hwnd,
                Some(&mut color),
                Some(&mut alpha),
                Some(&mut flags),
            )
            .is_ok()
                && alpha == 0
            {
                println!("Filtered: fully transparent layered window");
                return false;
            }
        }

        // Filter owned windows (typically dialogs)
        let owner = GetWindow(hwnd, GW_OWNER);
        if owner.is_ok() && !owner.unwrap().0.is_null() {
            println!("Filtered: owned window (dialog)");
            return false;
        }

        // Check window size - filter very small windows (tooltips, notifications)
        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_ok() {
            let width = rect.right - rect.left;
            let height = rect.bottom - rect.top;

            // Filter windows smaller than 100x100 (likely tooltips, popups)
            if width < 100 || height < 100 {
                println!("Filtered: too small ({}x{})", width, height);
                return false;
            }

            // Filter zero-size windows
            if width <= 0 || height <= 0 {
                println!("Filtered: zero size");
                return false;
            }

            // Filter windows positioned entirely off-screen (likely hidden)
            // This helps filter ghost windows
            if rect.right < -1000 || rect.bottom < -1000 || rect.left > 10000 || rect.top > 10000 {
                println!("Filtered: positioned off-screen");
                return false;
            }
        } else {
            println!("Filtered: couldn't get window rect");
            return false;
        }

        // Filter popup windows without resizable frame (likely dialogs)
        let is_popup = style & WS_POPUP.0 != 0;
        let has_thick_frame = style & WS_THICKFRAME.0 != 0;
        let has_caption = style & WS_CAPTION.0 != 0;

        // A popup without thick frame and without app window style is likely a dialog
        if is_popup && !has_thick_frame && (ex_style & WS_EX_APPWINDOW.0 == 0) {
            println!("Filtered: popup without thick frame");
            return false;
        }

        // System class filtering
        let system_classes = [
            "Shell_TrayWnd",
            "Shell_SecondaryTrayWnd",
            "Shell_traywnd",
            "WorkerW",
            "Progman",
            "DV2ControlHost",
            "XamlExplorerHostIslandWindow",
            "Windows.UI.Core.CoreWindow",
            "tooltips_class32",
            "IME",
            "MSCTFIME UI",
            "#32770", // Standard Windows dialog class
            "SysShadow",
            "MegaTileStatusBar", // Filter our own status bar
            "TaskListThumbnailWnd",
            "TaskSwitcherWnd",
            "TaskSwitcherOverlayWnd",
            "MultitaskingViewFrame",
            "ForegroundStaging",
            "ApplicationFrameWindow",
            "Windows.Internal.Shell.TabProxyWindow",
        ];

        for sys_class in &system_classes {
            if class_name.eq_ignore_ascii_case(sys_class) {
                println!("Filtered: system class {}", sys_class);
                return false;
            }
        }

        // Accept windows with WS_EX_APPWINDOW (explicitly meant for taskbar)
        if ex_style & WS_EX_APPWINDOW.0 != 0 {
            println!("Is normal, case 1: WS_EX_APPWINDOW");
            return true;
        }

        // Accept windows with a title that have both caption and thick frame (resizable)
        if has_caption && has_thick_frame {
            println!("Is normal, case 2: titled with caption and thick frame");
            return true;
        }

        // Accept windows with a title and overlapped style (standard app window)
        if style & WS_OVERLAPPEDWINDOW.0 != 0 {
            println!("Is normal, case 3: titled with overlapped window style");
            return true;
        }

        // Accept captioned windows
        if has_caption {
            println!("Is normal, case 4: has caption");
            return true;
        }

        println!("Filtered: doesn't match any normal window criteria");
        false
    }
}

/// Returns all windows that are suitable for tiling management.
pub fn get_normal_windows() -> Vec<WindowInfo> {
    enumerate_windows()
        .into_iter()
        .filter(|w| is_normal_window(w.hwnd, &w.class_name, &w.title))
        .collect()
}

/// Hides a window and removes it from the taskbar.
///
/// Used when switching away from a workspace to hide its windows.
pub fn hide_window_from_taskbar(hwnd: HWND) -> Result<(), String> {
    unsafe {
        // Store original window placement
        let mut placement = WINDOWPLACEMENT {
            length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
            ..Default::default()
        };

        if GetWindowPlacement(hwnd, &mut placement).is_ok() {
            // Hide the window
            let _ = ShowWindow(hwnd, SW_HIDE);

            // Remove from taskbar by temporarily removing WS_EX_APPWINDOW
            let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
            SetWindowLongW(hwnd, GWL_EXSTYLE, (ex_style & !WS_EX_APPWINDOW.0) as i32);

            Ok(())
        } else {
            Err("Failed to get window placement".to_string())
        }
    }
}

/// Shows a window and restores it to the taskbar.
///
/// Used when switching to a workspace to show its windows.
pub fn show_window_in_taskbar(hwnd: HWND) -> Result<(), String> {
    unsafe {
        // Restore WS_EX_APPWINDOW to show in taskbar
        let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
        SetWindowLongW(hwnd, GWL_EXSTYLE, (ex_style | WS_EX_APPWINDOW.0) as i32);

        // Show the window
        let _ = ShowWindow(hwnd, SW_SHOW);

        // Restore original placement
        let mut placement = WINDOWPLACEMENT {
            length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
            ..Default::default()
        };

        if GetWindowPlacement(hwnd, &mut placement).is_ok() {
            let _ = ShowWindow(hwnd, SHOW_WINDOW_CMD(placement.showCmd as i32));
            let _ = SetWindowPlacement(hwnd, &placement);
        }

        Ok(())
    }
}

/// Gets the bounding rectangle of a window.
pub fn get_window_rect(hwnd: HWND) -> Result<RECT, String> {
    let mut rect = RECT::default();
    unsafe {
        GetWindowRect(hwnd, &mut rect).map_err(|e| e.to_string())?;
    }
    Ok(rect)
}

/// Information about a display monitor.
pub struct MonitorInfo {
    /// Windows HMONITOR handle as isize.
    pub hmonitor: isize,
    /// Monitor screen bounds.
    pub rect: RECT,
    /// Whether this is the primary monitor.
    pub is_primary: bool,
}

/// Enumerates all connected display monitors.
pub fn enumerate_monitors() -> Vec<MonitorInfo> {
    let mut monitors = Vec::new();

    unsafe extern "system" fn enum_monitors_proc(
        hmonitor: HMONITOR,
        _hdc: HDC,
        _lprect: *mut RECT,
        lparam: LPARAM,
    ) -> BOOL {
        unsafe {
            let monitors = &mut *(lparam.0 as *mut Vec<MonitorInfo>);

            let mut info = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };

            if GetMonitorInfoW(hmonitor, &mut info).as_bool() {
                monitors.push(MonitorInfo {
                    hmonitor: hmonitor.0 as isize,
                    rect: info.rcMonitor,
                    is_primary: info.dwFlags & MONITORINFOF_PRIMARY != 0,
                });
            }

            TRUE
        }
    }

    unsafe {
        let _ = EnumDisplayMonitors(
            Some(HDC::default()),
            None,
            Some(enum_monitors_proc),
            LPARAM(&mut monitors as *mut _ as isize),
        );
    }

    monitors
}

/// Closes a window gracefully by sending WM_CLOSE.
pub fn close_window(hwnd: HWND) -> Result<(), String> {
    unsafe {
        // Try to close gracefully by sending WM_CLOSE message
        PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0))
            .map_err(|e| format!("Failed to send WM_CLOSE: {}", e))?;
        Ok(())
    }
}

/// Sets a window to fullscreen mode covering the specified monitor.
pub fn set_window_fullscreen(hwnd: HWND, monitor_rect: RECT) -> Result<(), String> {
    unsafe {
        // Set window to fullscreen
        SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            monitor_rect.left,
            monitor_rect.top,
            monitor_rect.right - monitor_rect.left,
            monitor_rect.bottom - monitor_rect.top,
            SWP_SHOWWINDOW,
        )
        .map_err(|e| format!("Failed to set window fullscreen: {}", e))?;

        Ok(())
    }
}

/// Restores a window from fullscreen to its original position.
pub fn restore_window_from_fullscreen(hwnd: HWND, original_rect: RECT) -> Result<(), String> {
    unsafe {
        // Restore original position and size
        SetWindowPos(
            hwnd,
            Some(HWND_NOTOPMOST),
            original_rect.left,
            original_rect.top,
            original_rect.right - original_rect.left,
            original_rect.bottom - original_rect.top,
            SWP_SHOWWINDOW | SWP_NOACTIVATE,
        )
        .map_err(|e| format!("Failed to restore window from fullscreen: {}", e))?;

        Ok(())
    }
}

/// Gets the Windows accent color and converts it to COLORREF format (0x00BBGGRR).
pub fn get_accent_color() -> Result<u32, String> {
    let mut color = 0u32;
    let mut pfopaque = BOOL(0);
    unsafe {
        DwmGetColorizationColor(&mut color, &mut pfopaque)
            .map_err(|e| format!("Failed to get accent color: {}", e))?;
    }
    // color is 0xAARRGGBB. Convert to 0x00BBGGRR (COLORREF format)
    let r = (color >> 16) & 0xFF;
    let g = (color >> 8) & 0xFF;
    let b = color & 0xFF;
    Ok((b << 16) | (g << 8) | r)
}

/// Sets the window border color.
///
/// # Arguments
/// * `color` - Color in COLORREF format (0x00BBGGRR)
pub fn set_window_border_color(hwnd: HWND, color: u32) -> Result<(), String> {
    unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_BORDER_COLOR,
            &color as *const _ as *const std::ffi::c_void,
            std::mem::size_of::<u32>() as u32,
        )
        .map_err(|e| format!("Failed to set window border color: {}", e))?;
    }
    Ok(())
}

/// Sets the window transparency level.
///
/// # Arguments
/// * `alpha` - Transparency level (0 = fully transparent, 255 = fully opaque)
pub fn set_window_transparency(hwnd: HWND, alpha: u8) -> Result<(), String> {
    unsafe {
        SetLastError(WIN32_ERROR(0));
        let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE);
        if alpha == 255 {
            let result = SetWindowLongW(
                hwnd,
                GWL_EXSTYLE,
                (ex_style as u32 & !WS_EX_LAYERED.0) as i32,
            );
            if result == 0 && GetLastError() != WIN32_ERROR(0) {
                return Err(format!(
                    "Failed to clear layered style: {}",
                    windows::core::Error::from_thread()
                ));
            }
        } else {
            let result = SetWindowLongW(
                hwnd,
                GWL_EXSTYLE,
                (ex_style as u32 | WS_EX_LAYERED.0) as i32,
            );
            if result == 0 && GetLastError() != WIN32_ERROR(0) {
                return Err(format!(
                    "Failed to set layered style: {}",
                    windows::core::Error::from_thread()
                ));
            }
            // COLORREF(0) is unused when LWA_ALPHA flag is set
            SetLayeredWindowAttributes(hwnd, COLORREF(0), alpha, LWA_ALPHA)
                .map_err(|e| format!("Failed to set layered window attributes: {}", e))?;
        }
        // Force frame update
        SetWindowPos(
            hwnd,
            None,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED | SWP_NOACTIVATE,
        )
        .map_err(|e| format!("Failed to update window frame: {}", e))?;
    }
    Ok(())
}

/// Resets window decorations to default (removes custom border color and transparency).
pub fn reset_window_decorations(hwnd: HWND) -> Result<(), String> {
    set_window_border_color(hwnd, DWMWA_COLOR_DEFAULT)?;
    set_window_transparency(hwnd, 255)?;
    Ok(())
}

/// Gets the DWM extended frame bounds for a window.
/// Returns the actual visible bounds (excluding invisible DWM borders).
pub fn get_dwm_frame_bounds(hwnd: HWND) -> Result<RECT, String> {
    let mut frame_rect = RECT::default();
    unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut frame_rect as *mut _ as *mut std::ffi::c_void,
            std::mem::size_of::<RECT>() as u32,
        )
        .map_err(|e| format!("Failed to get DWM frame bounds: {}", e))?;
    }
    Ok(frame_rect)
}

/// Gets the invisible border sizes for a window (difference between window rect and DWM frame).
/// Returns (left, top, right, bottom) border sizes.
pub fn get_invisible_borders(hwnd: HWND) -> (i32, i32, i32, i32) {
    let window_rect = match get_window_rect(hwnd) {
        Ok(r) => r,
        Err(_) => return (0, 0, 0, 0),
    };

    let frame_rect = match get_dwm_frame_bounds(hwnd) {
        Ok(r) => r,
        Err(_) => return (0, 0, 0, 0),
    };

    // Calculate the invisible borders
    let left = frame_rect.left - window_rect.left;
    let top = frame_rect.top - window_rect.top;
    let right = window_rect.right - frame_rect.right;
    let bottom = window_rect.bottom - frame_rect.bottom;

    (left, top, right, bottom)
}

/// Adjusts a target rect to compensate for DWM invisible borders.
/// Returns a rect that, when set, will result in the visible area matching the target.
pub fn adjust_rect_for_dwm_borders(hwnd: HWND, target: &RECT) -> RECT {
    let (left_border, top_border, right_border, bottom_border) = get_invisible_borders(hwnd);

    RECT {
        left: target.left - left_border,
        top: target.top - top_border,
        right: target.right + right_border,
        bottom: target.bottom + bottom_border,
    }
}
