#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod hotkeys;
mod tiling;
mod tray;
mod windows_lib;
mod workspace;
mod workspace_manager;

use hotkeys::HotkeyManager;
use std::collections::HashSet;

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tray::TrayManager;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, HWND_MESSAGE};
use windows_lib::{enumerate_monitors, get_normal_windows, show_window_in_taskbar};
use workspace_manager::WorkspaceManager;

static CLASS_NAME: [u16; 22] = [
    77, 101, 103, 97, 84, 105, 108, 101, 77, 101, 115, 115, 97, 103, 101, 87, 105, 110, 100, 111,
    119, 0,
];
static TITLE: [u16; 9] = [77, 101, 103, 97, 84, 105, 108, 101, 0];

fn start_window_monitoring(workspace_manager: Arc<Mutex<WorkspaceManager>>) {
    thread::spawn(move || loop {
        let new_windows = detect_new_windows(&workspace_manager);
        if !new_windows.is_empty() {
            manage_new_windows(&workspace_manager, new_windows);
        }
        thread::sleep(Duration::from_millis(500));
    });
}

fn detect_new_windows(
    workspace_manager: &Arc<Mutex<WorkspaceManager>>,
) -> Vec<windows_lib::WindowInfo> {
    let existing_hwnds: HashSet<isize> = {
        let wm = workspace_manager.lock().unwrap();
        wm.get_all_managed_hwnds().into_iter().collect()
    };

    let all_windows = windows_lib::get_normal_windows();
    all_windows
        .into_iter()
        .filter(|w| !existing_hwnds.contains(&(w.hwnd.0 as isize)))
        .collect()
}

fn manage_new_windows(
    workspace_manager: &Arc<Mutex<WorkspaceManager>>,
    new_windows: Vec<windows_lib::WindowInfo>,
) {
    let mut wm = workspace_manager.lock().unwrap();
    let active_workspace = wm.get_active_workspace();

    for window_info in &new_windows {
        let monitor_index = wm.get_monitor_for_window(window_info.hwnd).unwrap_or(0);

        let window = workspace::Window::new(
            window_info.hwnd.0 as isize,
            active_workspace,
            monitor_index,
            window_info.rect,
        );

        if active_workspace == wm.get_active_workspace() {
            let _ = show_window_in_taskbar(HWND(window.hwnd as *mut std::ffi::c_void));
        } else {
            let _ =
                windows_lib::hide_window_from_taskbar(HWND(window.hwnd as *mut std::ffi::c_void));
        }

        wm.add_window(window);
    }

    if !new_windows.is_empty() {
        wm.tile_active_workspaces();
        wm.apply_window_positions();
    }
}

fn main() {
    println!("MegaTile - Window Manager");

    // Initialize workspace manager
    let workspace_manager = Arc::new(Mutex::new(WorkspaceManager::new()));

    // Enumerate monitors and create monitor structs
    let monitor_infos = enumerate_monitors();
    println!("Found {} monitor(s):", monitor_infos.len());

    let monitors: Vec<workspace::Monitor> = monitor_infos
        .iter()
        .enumerate()
        .map(|(i, info)| {
            println!("  Monitor {}: {:?}", i + 1, info.rect);
            workspace::Monitor::new(info.hmonitor, info.rect)
        })
        .collect();

    workspace_manager.lock().unwrap().set_monitors(monitors);

    // Enumerate windows and assign to workspace 1
    let normal_windows = get_normal_windows();
    println!("Found {} normal windows", normal_windows.len());

    {
        let focused_hwnd = unsafe { GetForegroundWindow() };
        let wm = workspace_manager.lock().unwrap();
        for window_info in normal_windows {
            let is_focused = window_info.hwnd == focused_hwnd;
            let monitor_index = wm.get_monitor_for_window(window_info.hwnd).unwrap_or(0);
            let mut window = workspace::Window::new(
                window_info.hwnd.0 as isize,
                1, // Assign to workspace 1
                monitor_index,
                window_info.rect,
            );
            window.is_focused = is_focused;
            // Since workspace 1 is active, show in taskbar
            let _ = show_window_in_taskbar(window_info.hwnd);
            wm.add_window(window);
        }
    }

    println!("Assigned all windows to workspace 1");

    // Apply initial tiling
    {
        let wm = workspace_manager.lock().unwrap();
        wm.tile_active_workspaces();
        wm.apply_window_positions();
    }
    println!("Applied initial tiling to workspace 1");

    // Start monitoring for new windows
    start_window_monitoring(Arc::clone(&workspace_manager));

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
            cleanup_on_exit(&workspace_manager);
            hotkey_manager.unregister_all(hwnd);
            break;
        }

        // Process window messages
        let mut msg = MSG::default();
        while unsafe { PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE) }.as_bool() {
            // Check for WM_HOTKEY BEFORE dispatching
            if msg.message == WM_HOTKEY {
                let action = hotkey_manager.get_action(msg.wParam.0 as i32);
                if let Some(action) = action {
                    handle_hotkey(action, &workspace_manager);
                }
            } else {
                // Only dispatch non-hotkey messages
                unsafe {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
        }

        std::thread::sleep(Duration::from_millis(10));
    }
}

fn cleanup_on_exit(workspace_manager: &Arc<Mutex<WorkspaceManager>>) {
    println!("Restoring all hidden windows...");
    let wm = workspace_manager.lock().unwrap();
    let monitors = wm.get_monitors();

    for monitor in monitors {
        for workspace_num in 1..=9 {
            if let Some(workspace) = monitor.get_workspace(workspace_num) {
                for window in &workspace.windows {
                    // Show all windows regardless of current workspace
                    if let Err(e) =
                        show_window_in_taskbar(HWND(window.hwnd as *mut std::ffi::c_void))
                    {
                        eprintln!("Failed to restore window {:?}: {}", window.hwnd, e);
                    }
                }
            }
        }
    }
    println!("Window restoration complete.");
}

fn handle_hotkey(action: hotkeys::HotkeyAction, workspace_manager: &Arc<Mutex<WorkspaceManager>>) {
    match action {
        hotkeys::HotkeyAction::SwitchWorkspace(num) => {
            let mut wm = workspace_manager.lock().unwrap();
            match wm.switch_workspace_with_windows(num) {
                Ok(()) => {
                    println!("Switched to workspace {}", num);
                    // Tile and apply positions for new workspace
                    wm.tile_active_workspaces();
                    wm.apply_window_positions();
                }
                Err(e) => eprintln!("Failed to switch workspace: {}", e),
            }
        }
        hotkeys::HotkeyAction::MoveLeft => {
            println!("Moving left");
            let wm = workspace_manager.lock().unwrap();
            if let Err(e) = wm.move_window(workspace_manager::FocusDirection::Left) {
                eprintln!("Failed to move window: {}", e);
            } else {
                println!("Moved window left");
            }
        }
        hotkeys::HotkeyAction::MoveRight => {
            let wm = workspace_manager.lock().unwrap();
            if let Err(e) = wm.move_window(workspace_manager::FocusDirection::Right) {
                eprintln!("Failed to move window: {}", e);
            } else {
                println!("Moved window right");
            }
        }
        hotkeys::HotkeyAction::FocusLeft => {
            let wm = workspace_manager.lock().unwrap();
            if let Err(e) = wm.move_focus(workspace_manager::FocusDirection::Left) {
                eprintln!("Failed to move focus: {}", e);
            } else {
                println!("Moved focus left");
            }
        }
        hotkeys::HotkeyAction::FocusRight => {
            let wm = workspace_manager.lock().unwrap();
            if let Err(e) = wm.move_focus(workspace_manager::FocusDirection::Right) {
                eprintln!("Failed to move focus: {}", e);
            } else {
                println!("Moved focus right");
            }
        }
        hotkeys::HotkeyAction::FocusUp => {
            let wm = workspace_manager.lock().unwrap();
            if let Err(e) = wm.move_focus(workspace_manager::FocusDirection::Up) {
                eprintln!("Failed to move focus: {}", e);
            } else {
                println!("Moved focus up");
            }
        }
        hotkeys::HotkeyAction::FocusDown => {
            let wm = workspace_manager.lock().unwrap();
            if let Err(e) = wm.move_focus(workspace_manager::FocusDirection::Down) {
                eprintln!("Failed to move focus: {}", e);
            } else {
                println!("Moved focus down");
            }
        }
        hotkeys::HotkeyAction::MoveUp => {
            let wm = workspace_manager.lock().unwrap();
            if let Err(e) = wm.move_window(workspace_manager::FocusDirection::Up) {
                eprintln!("Failed to move window: {}", e);
            } else {
                println!("Moved window up");
            }
        }
        hotkeys::HotkeyAction::MoveDown => {
            let wm = workspace_manager.lock().unwrap();
            if let Err(e) = wm.move_window(workspace_manager::FocusDirection::Down) {
                eprintln!("Failed to move window: {}", e);
            } else {
                println!("Moved window down");
            }
        }
        hotkeys::HotkeyAction::MoveToWorkspace(num) => {
            // TODO: Get currently focused window
            println!("Move to workspace {} (not yet implemented)", num);
        }
        _ => {
            println!("Hotkey action: {:?}", action);
            // TODO: Implement other actions in future steps
        }
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
            Some(HWND_MESSAGE),
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
