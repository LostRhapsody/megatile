use windows::core::BOOL;
use windows::Win32::Foundation::{HWND, LPARAM, RECT, TRUE};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO,
};
use windows::Win32::UI::WindowsAndMessaging::*;

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

    let mut title_buffer = [0u16; 256];
    let length = unsafe { GetWindowTextW(hwnd, &mut title_buffer) };
    let title = String::from_utf16_lossy(&title_buffer[..length as usize]);

    let mut class_buffer = [0u16; 256];
    let class_len = unsafe { GetClassNameW(hwnd, &mut class_buffer) };
    let class_name = String::from_utf16_lossy(&class_buffer[..class_len as usize]);

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

pub fn is_normal_window(hwnd: HWND, class_name: &str, title: &str) -> bool {
    unsafe {
        if !IsWindowVisible(hwnd).as_bool() {
            return false;
        }

        if IsIconic(hwnd).as_bool() {
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
        ];

        for sys_class in &system_classes {
            if class_name.eq_ignore_ascii_case(sys_class) {
                return false;
            }
        }

        if ex_style & WS_EX_APPWINDOW.0 != 0 {
            return true;
        }

        if !title.trim().is_empty() {
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
