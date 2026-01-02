#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod hotkeys;
mod tray;
mod windows_lib;

use hotkeys::HotkeyManager;
use std::time::Duration;
use tray::TrayManager;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::PCWSTR;

static CLASS_NAME: [u16; 22] = [
    77, 101, 103, 97, 84, 105, 108, 101, 77, 101, 115, 115, 97, 103, 101, 87, 105, 110, 100, 111,
    119, 0,
];
static TITLE: [u16; 9] = [77, 101, 103, 97, 84, 105, 108, 101, 0];

fn main() {
    println!("MegaTile - Window Manager");

    // Initialize tray icon
    let tray = TrayManager::new().expect("Failed to create tray icon");

    // Create hidden window for hotkey messages
    let hwnd = create_message_window().expect("Failed to create message window");

    // Register hotkeys
    let mut hotkey_manager = HotkeyManager::new();
    hotkey_manager
        .register_hotkeys(hwnd)
        .expect("Failed to register hotkeys");

    println!("MegaTile is running. Use the tray icon to exit.");

    // Main event loop
    loop {
        if tray.should_exit() {
            println!("Exiting MegaTile...");
            hotkey_manager.unregister_all(hwnd);
            break;
        }

        // Process window messages
        let mut msg = MSG::default();
        while unsafe { PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE) }.as_bool() {
            unsafe {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);

                if msg.message == WM_HOTKEY {
                    let action = hotkey_manager.get_action(msg.wParam.0 as i32);
                    if let Some(action) = action {
                        println!("Hotkey pressed: {:?}", action);
                        // TODO: Handle hotkey action
                    }
                } else if msg.message == WM_DESTROY {
                    PostQuitMessage(0);
                }
            }
        }

        std::thread::sleep(Duration::from_millis(10));
    }
}

extern "system" fn window_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        if msg == WM_DESTROY {
            PostQuitMessage(0);
        }
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}

fn create_message_window() -> Result<HWND, String> {
    unsafe {
        let class_name = PCWSTR(CLASS_NAME.as_ptr());

        let wc = WNDCLASSW {
            hInstance: GetModuleHandleW(None).unwrap().into(),
            lpfnWndProc: Some(window_proc),
            lpszClassName: class_name,
            ..Default::default()
        };

        if RegisterClassW(&wc) == 0 {
            return Err("Failed to register window class".to_string());
        }

        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class_name,
            PCWSTR(TITLE.as_ptr()),
            WINDOW_STYLE::default(),
            0,
            0,
            0,
            0,
            None,
            None,
            Some(GetModuleHandleW(None).unwrap().into()),
            None,
        );

        let hwnd = match hwnd {
            Ok(h) => h,
            Err(_) => return Err("Failed to create window".to_string()),
        };

        Ok(hwnd)
    }
}
