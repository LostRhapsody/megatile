use windows::Win32::Foundation::{HWND, LPARAM, RECT, TRUE, WPARAM};
use windows::Win32::Graphics::Dwm::*;
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITOR_DEFAULTTONEAREST, MONITORINFO,
    MonitorFromWindow,
};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::BOOL;

const MONITORINFOF_PRIMARY: u32 = 1;

pub struct WindowInfo {
    pub hwnd: HWND,
    pub title: String,
    pub class_name: String,
    pub rect: RECT,
    pub is_visible: bool,
    pub is_minimized: bool,
}

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

pub fn get_window_title(hwnd: HWND) -> String {
    let mut title_buffer = [0u16; 256];
    let length = unsafe { GetWindowTextW(hwnd, &mut title_buffer) };
    String::from_utf16_lossy(&title_buffer[..length as usize])
}

pub fn get_window_class(hwnd: HWND) -> String {
    let mut class_buffer = [0u16; 256];
    let class_len = unsafe { GetClassNameW(hwnd, &mut class_buffer) };
    String::from_utf16_lossy(&class_buffer[..class_len as usize])
}

pub fn is_normal_window_hwnd(hwnd: HWND) -> bool {
    let title = get_window_title(hwnd);
    let class_name = get_window_class(hwnd);
    let is_normal = is_normal_window(hwnd, &class_name, &title);
    println!("is normal? {}", is_normal);
    is_normal
}

pub fn is_normal_window(hwnd: HWND, class_name: &str, title: &str) -> bool {
    println!("Checking if window, title {}, class name {}, hwnd {:?}, is 'normal'.", title, class_name, hwnd);
    unsafe {
        if !IsWindowVisible(hwnd).as_bool() {
            return false;
        }

        if IsIconic(hwnd).as_bool() {
            return false;
        }

        if title == "Windows Input Experience" ||
        title == "Chrome Legacy Window" ||
        title == "OLEChannelWnd" ||
        title == "DesktopWindowXamlSource" ||
        title == "Non Client Input Sink Window" {
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
            return false;
        }

        let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;

        if ex_style & WS_EX_TOOLWINDOW.0 != 0 {
            return false;
        }

        if ex_style & WS_EX_NOACTIVATE.0 != 0 {
            return false;
        }

        let system_classes = [
            "Shell_TrayWnd",
            "Shell_SecondaryTrayWnd",
            "Shell_traywnd",
            "WorkerW",
            "Progman",
            "DV2ControlHost",
            "XamlExplorerHostIslandWindow",
            "Windows.UI.Core.CoreWindow",
        ];

        for sys_class in &system_classes {
            if class_name.eq_ignore_ascii_case(sys_class) {
                return false;
            }
        }

        if ex_style & WS_EX_APPWINDOW.0 != 0 {
            println!("Is normal, case 1");
            return true;
        }

        if !title.trim().is_empty() {
            println!("Is normal, case 2");
            return true;
        }

        false
    }
}

pub fn get_normal_windows() -> Vec<WindowInfo> {
    enumerate_windows()
        .into_iter()
        .filter(|w| is_normal_window(w.hwnd, &w.class_name, &w.title))
        .collect()
}

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

pub fn get_window_rect(hwnd: HWND) -> Result<RECT, String> {
    let mut rect = RECT::default();
    unsafe {
        GetWindowRect(hwnd, &mut rect).map_err(|e| e.to_string())?;
    }
    Ok(rect)
}

pub fn is_window_hidden(hwnd: HWND) -> bool {
    unsafe { !IsWindowVisible(hwnd).as_bool() }
}

pub struct MonitorInfo {
    pub hmonitor: isize,
    pub rect: RECT,
    pub is_primary: bool,
}

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

pub fn close_window(hwnd: HWND) -> Result<(), String> {
    unsafe {
        // Try to close gracefully by sending WM_CLOSE message
        PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0))
            .map_err(|e| format!("Failed to send WM_CLOSE: {}", e))?;
        Ok(())
    }
}

pub fn force_close_window(hwnd: HWND) -> Result<(), String> {
    unsafe {
        // Force terminate the window
        DestroyWindow(hwnd).map_err(|e| format!("Failed to destroy window: {}", e))?;
        Ok(())
    }
}

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

pub fn get_monitor_rect(hwnd: HWND) -> Option<RECT> {
    unsafe {
        let mut monitor_info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };

        let hmonitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);

        if GetMonitorInfoW(hmonitor, &mut monitor_info).as_bool() {
            Some(monitor_info.rcMonitor)
        } else {
            None
        }
    }
}
