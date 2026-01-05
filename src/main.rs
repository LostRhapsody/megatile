//! # MegaTile - A Tiling Window Manager for Windows
//!
//! MegaTile is a lightweight tiling window manager designed for Windows 10/11.
//! It provides automatic window tiling with a dwindle layout algorithm,
//! multi-monitor support, and workspace management.
//!
//! ## Features
//!
//! - **Automatic Tiling**: Windows are automatically arranged using a dwindle algorithm
//! - **Workspaces**: 9 virtual workspaces per monitor
//! - **Hotkey Support**: Comprehensive keyboard shortcuts for window management
//! - **Multi-Monitor**: Full support for multiple displays
//! - **System Tray**: Minimal tray icon for easy access
//! - **Status Bar**: Visual workspace indicator
//!
//! ## Architecture
//!
//! - [`windows_lib`] - Windows API abstractions and window management utilities
//! - [`workspace`] - Core data structures (Window, Workspace, Monitor)
//! - [`workspace_manager`] - High-level workspace operations and state management
//! - [`tiling`] - Tiling algorithms and layout calculations
//! - [`hotkeys`] - Hotkey registration and action mapping
//! - [`tray`] - System tray integration
//! - [`statusbar`] - Visual workspace indicator

#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod hotkeys;
mod statusbar;
mod tiling;
mod tray;
mod windows_lib;
mod workspace;
mod workspace_manager;

use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Accessibility::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::PCWSTR;

use hotkeys::HotkeyManager;
use statusbar::{
    STATUSBAR_HEIGHT, STATUSBAR_TOP_GAP, STATUSBAR_WIDTH, StatusBar, init_gdiplus, shutdown_gdiplus,
};
use tray::TrayManager;
use windows_lib::{
    enumerate_monitors, get_normal_windows, reset_window_decorations, show_window_in_taskbar,
};
use workspace_manager::WorkspaceManager;

/// Window class name for the hidden message window ("MegaTileMessageWindow" as UTF-16).
static CLASS_NAME: [u16; 22] = [
    77, 101, 103, 97, 84, 105, 108, 101, 77, 101, 115, 115, 97, 103, 101, 87, 105, 110, 100, 111,
    119, 0,
];

/// Window title ("MegaTile" as UTF-16).
static TITLE: [u16; 9] = [77, 101, 103, 97, 84, 105, 108, 101, 0];

/// Internal events processed by the main event loop.
#[derive(Debug)]
enum WindowEvent {
    Hotkey(hotkeys::HotkeyAction),
    WindowCreated(isize),
    WindowDestroyed(isize),
    WindowMinimized(isize),
    WindowMoved(isize),
    FocusChanged(isize),
    DisplayChange,
    PeriodicCheck,
    TrayExit,
}

/// Global event queue for inter-thread communication.
static EVENT_QUEUE: OnceLock<Mutex<VecDeque<WindowEvent>>> = OnceLock::new();

/// Pushes an event to the global event queue for processing in the main loop.
fn push_event(event: WindowEvent) {
    if let Some(queue) = EVENT_QUEUE.get()
        && let Ok(mut q) = queue.lock()
    {
        q.push_back(event);
    }
}

/// Windows accessibility event callback for tracking window changes.
///
/// This callback receives notifications about window creation, destruction,
/// movement, and focus changes from the Windows accessibility API.
unsafe extern "system" fn win_event_proc(
    _hwin_event_hook: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    id_object: i32,
    id_child: i32,
    _id_event_thread: u32,
    _dwms_event_time: u32,
) {
    if id_object != OBJID_WINDOW.0 || id_child != CHILDID_SELF as i32 || hwnd.0.is_null() {
        return;
    }

    match event {
        EVENT_SYSTEM_FOREGROUND => {
            push_event(WindowEvent::FocusChanged(hwnd.0 as isize));
        }
        EVENT_OBJECT_CREATE | EVENT_OBJECT_SHOW => {
            push_event(WindowEvent::WindowCreated(hwnd.0 as isize));
        }
        EVENT_OBJECT_DESTROY => {
            push_event(WindowEvent::WindowDestroyed(hwnd.0 as isize));
        }
        EVENT_SYSTEM_MINIMIZESTART => {
            push_event(WindowEvent::WindowMinimized(hwnd.0 as isize));
        }
        EVENT_OBJECT_LOCATIONCHANGE => {
            push_event(WindowEvent::WindowMoved(hwnd.0 as isize));
        }
        _ => {}
    }
}

/// Restores all managed windows to their visible state before exit.
///
/// This ensures windows are not left hidden in the taskbar when MegaTile exits.
fn cleanup_on_exit(wm: &mut WorkspaceManager) {
    println!("Restoring all hidden windows...");

    // Get all managed windows from all workspaces
    let all_hwnds = wm.get_all_managed_hwnds();
    println!("Found {} managed windows to restore", all_hwnds.len());

    let normal_windows = get_normal_windows();
    println!("Found {} normal windows to restore", normal_windows.len());
    for window_info in normal_windows {
        println!(
            "  - {} (Class: {})",
            window_info.title, window_info.class_name
        );
    }

    let mut restored_count = 0;
    let mut failed_count = 0;

    for hwnd in all_hwnds {
        let hwnd_handle = HWND(hwnd as *mut std::ffi::c_void);

        // Try to restore each window
        match show_window_in_taskbar(hwnd_handle) {
            Ok(()) => {
                restored_count += 1;
                println!("  ✓ Restored window {:?}", hwnd);
            }
            Err(e) => {
                failed_count += 1;
                eprintln!("  ✗ Failed to restore window {:?}: {}", hwnd, e);
            }
        }
        if let Err(e) = reset_window_decorations(hwnd_handle) {
            eprintln!(
                "  ✗ Failed to reset window decorations for {:?}: {}",
                hwnd, e
            );
        }
    }

    println!(
        "Window restoration complete: {} restored, {} failed",
        restored_count, failed_count
    );
}

/// Dispatches a hotkey action to the workspace manager.
fn handle_action(action: hotkeys::HotkeyAction, wm: &mut WorkspaceManager) {
    match action {
        hotkeys::HotkeyAction::SwitchWorkspace(num) => {
            match wm.switch_workspace_with_windows(num) {
                Ok(()) => {
                    println!("Switched to workspace {}", num);
                    wm.tile_active_workspaces();
                    wm.apply_window_positions();
                }
                Err(e) => eprintln!("Failed to switch workspace: {}", e),
            }
        }
        hotkeys::HotkeyAction::MoveLeft => {
            if let Err(e) = wm.move_window(workspace_manager::FocusDirection::Left) {
                eprintln!("Failed to move window: {}", e);
            }
        }
        hotkeys::HotkeyAction::MoveRight => {
            if let Err(e) = wm.move_window(workspace_manager::FocusDirection::Right) {
                eprintln!("Failed to move window: {}", e);
            }
        }
        hotkeys::HotkeyAction::FocusLeft => {
            if let Err(e) = wm.move_focus(workspace_manager::FocusDirection::Left) {
                eprintln!("Failed to move focus: {}", e);
            }
        }
        hotkeys::HotkeyAction::FocusRight => {
            if let Err(e) = wm.move_focus(workspace_manager::FocusDirection::Right) {
                eprintln!("Failed to move focus: {}", e);
            }
        }
        hotkeys::HotkeyAction::FocusUp => {
            if let Err(e) = wm.move_focus(workspace_manager::FocusDirection::Up) {
                eprintln!("Failed to move focus: {}", e);
            }
        }
        hotkeys::HotkeyAction::FocusDown => {
            if let Err(e) = wm.move_focus(workspace_manager::FocusDirection::Down) {
                eprintln!("Failed to move focus: {}", e);
            }
        }
        hotkeys::HotkeyAction::MoveUp => {
            if let Err(e) = wm.move_window(workspace_manager::FocusDirection::Up) {
                eprintln!("Failed to move window: {}", e);
            }
        }
        hotkeys::HotkeyAction::MoveDown => {
            if let Err(e) = wm.move_window(workspace_manager::FocusDirection::Down) {
                eprintln!("Failed to move window: {}", e);
            }
        }
        hotkeys::HotkeyAction::MoveToWorkspace(num) => match wm.move_window_to_workspace(num) {
            Ok(()) => {
                println!("Moved window to workspace {}", num);
                wm.print_workspace_status();
            }
            Err(e) => eprintln!("Failed to move window: {}", e),
        },
        hotkeys::HotkeyAction::ToggleTiling => {
            if let Some(focused) = wm.get_focused_window()
                && let Err(e) = wm.toggle_window_tiling(HWND(focused.hwnd as _))
            {
                eprintln!("Failed to toggle tiling: {}", e);
            }
        }
        hotkeys::HotkeyAction::ToggleFullscreen => match wm.toggle_fullscreen() {
            Ok(()) => println!("Fullscreen toggled"),
            Err(e) => eprintln!("Failed to toggle fullscreen: {}", e),
        },
        hotkeys::HotkeyAction::ResizeHorizontalIncrease => {
            if let Err(e) =
                wm.resize_focused_window(workspace_manager::ResizeDirection::Horizontal, 0.05)
            {
                eprintln!("Failed to resize window: {}", e);
            }
        }
        hotkeys::HotkeyAction::ResizeHorizontalDecrease => {
            if let Err(e) =
                wm.resize_focused_window(workspace_manager::ResizeDirection::Horizontal, -0.05)
            {
                eprintln!("Failed to resize window: {}", e);
            }
        }
        hotkeys::HotkeyAction::ResizeVerticalIncrease => {
            if let Err(e) =
                wm.resize_focused_window(workspace_manager::ResizeDirection::Vertical, 0.05)
            {
                eprintln!("Failed to resize window: {}", e);
            }
        }
        hotkeys::HotkeyAction::ResizeVerticalDecrease => {
            if let Err(e) =
                wm.resize_focused_window(workspace_manager::ResizeDirection::Vertical, -0.05)
            {
                eprintln!("Failed to resize window: {}", e);
            }
        }
        hotkeys::HotkeyAction::FlipRegion => {
            if let Err(e) = wm.flip_focused_region() {
                eprintln!("Failed to flip region: {}", e);
            }
        }
        hotkeys::HotkeyAction::CloseWindow => match wm.close_focused_window() {
            Ok(()) => println!("Window closed successfully"),
            Err(e) => eprintln!("Failed to close window: {}", e),
        },
        hotkeys::HotkeyAction::ToggleStatusBar => {
            wm.invert_statusbar_visibility();
        }
    }
}

fn main() {
    println!("MegaTile - Window Manager");

    // Initialize event queue
    EVENT_QUEUE.set(Mutex::new(VecDeque::new())).unwrap();

    // Initialize workspace manager
    let mut wm = WorkspaceManager::new();

    // Setup Ctrl+C handler for cleanup
    ctrlc::set_handler(move || {
        println!("\nReceived Ctrl+C signal, pushing exit event...");
        push_event(WindowEvent::TrayExit);
    })
    .expect("Error setting Ctrl+C handler");

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

    wm.set_monitors(monitors);

    // Enumerate windows and assign to workspace 1
    let normal_windows = get_normal_windows();
    println!("Found {} normal windows:", normal_windows.len());

    let focused_hwnd = unsafe { GetForegroundWindow() };
    for window_info in normal_windows {
        println!(
            "  - {} (Class: {})",
            window_info.title, window_info.class_name
        );
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

    println!("Assigned all windows to workspace 1");

    // Apply initial tiling
    wm.tile_active_workspaces();
    wm.apply_window_positions();
    println!("Applied initial tiling to workspace 1");

    // Setup window event hooks
    let _event_hook = unsafe {
        SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_OBJECT_LOCATIONCHANGE,
            None,
            Some(win_event_proc),
            0,
            0,
            WINEVENT_OUTOFCONTEXT,
        )
    };

    // Setup minimize event hook
    let _minimize_hook = unsafe {
        SetWinEventHook(
            EVENT_SYSTEM_MINIMIZESTART,
            EVENT_SYSTEM_MINIMIZEEND,
            None,
            Some(win_event_proc),
            0,
            0,
            WINEVENT_OUTOFCONTEXT,
        )
    };

    // Initialize tray icon
    let tray = TrayManager::new().expect("Failed to create tray icon");

    // Create hidden window for hotkey messages
    let hwnd = create_message_window().expect("Failed to create message window");

    // Register hotkeys
    let mut hotkey_manager = HotkeyManager::new();
    hotkey_manager
        .register_hotkeys(hwnd)
        .expect("Failed to register hotkeys");

    // Initialize GDI+ for anti-aliased rendering
    init_gdiplus().expect("Failed to initialize GDI+");

    // Initialize status bar
    let statusbar = StatusBar::new(hwnd).expect("Failed to create status bar");

    // Set status bar position and size (top center of primary monitor)
    let monitor_infos = windows_lib::enumerate_monitors();
    if let Some(primary_monitor) = monitor_infos.iter().find(|m| m.is_primary) {
        let rect = primary_monitor.rect;
        let statusbar_width = STATUSBAR_WIDTH;
        let statusbar_height = STATUSBAR_HEIGHT;
        let x = rect.left + (rect.right - rect.left - statusbar_width) / 2;
        let y = rect.top + STATUSBAR_TOP_GAP;

        statusbar.set_position(x, y, statusbar_width, statusbar_height);
        statusbar.show(); // Show the status bar on startup
    }

    wm.set_statusbar(statusbar);
    wm.update_statusbar();
    wm.update_decorations();

    println!("MegaTile is running. Use the tray icon to exit.");

    let mut last_periodic_check = Instant::now();
    let periodic_check_interval = Duration::from_millis(100);

    // Main event loop
    loop {
        if tray.should_exit() {
            push_event(WindowEvent::TrayExit);
        }

        // Periodic check event
        if last_periodic_check.elapsed() >= periodic_check_interval {
            push_event(WindowEvent::PeriodicCheck);
            last_periodic_check = Instant::now();
        }

        // Process window messages
        let mut msg = MSG::default();
        while unsafe { PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE) }.as_bool() {
            if msg.message == WM_QUIT {
                push_event(WindowEvent::TrayExit);
            } else if msg.message == WM_HOTKEY {
                let action = hotkey_manager.get_action(msg.wParam.0 as i32);
                if let Some(action) = action {
                    push_event(WindowEvent::Hotkey(action));
                }
            } else if msg.message == WM_DISPLAYCHANGE {
                push_event(WindowEvent::DisplayChange);
            } else {
                unsafe {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
        }

        // Process all events from the queue per iteration
        loop {
            let event = if let Some(queue) = EVENT_QUEUE.get() {
                if let Ok(mut q) = queue.lock() {
                    q.pop_front()
                } else {
                    None
                }
            } else {
                None
            };

            if let Some(event) = event {
                match event {
                    WindowEvent::Hotkey(action) => {
                        handle_action(action, &mut wm);
                    }
                    WindowEvent::WindowCreated(hwnd_val) => {
                        let hwnd = HWND(hwnd_val as *mut std::ffi::c_void);

                        // Check if we already manage this window
                        if wm.get_window(hwnd).is_some() {
                            continue;
                        }

                        // Use is_normal_window_hwnd which is more efficient
                        if windows_lib::is_normal_window_hwnd(hwnd) {
                            println!("Event: Window Registered {:?}", hwnd);
                            let info = windows_lib::WindowInfo {
                                hwnd,
                                title: windows_lib::get_window_title(hwnd),
                                class_name: windows_lib::get_window_class(hwnd),
                                rect: windows_lib::get_window_rect(hwnd).unwrap_or_default(),
                                is_visible: true,
                                is_minimized: false,
                            };

                            let active_workspace = wm.get_active_workspace();
                            let monitor_index = wm.get_monitor_for_window(hwnd).unwrap_or(0);
                            let window = workspace::Window::new(
                                hwnd_val,
                                active_workspace,
                                monitor_index,
                                info.rect,
                            );
                            let _ = show_window_in_taskbar(hwnd);
                            wm.add_window(window);
                            wm.tile_active_workspaces();
                            wm.apply_window_positions();
                        }
                    }
                    WindowEvent::WindowDestroyed(hwnd_val) => {
                        let hwnd = HWND(hwnd_val as *mut std::ffi::c_void);
                        println!("Event: Window Destroyed {:?}", hwnd);
                        wm.remove_window_with_tiling(hwnd);
                    }
                    WindowEvent::WindowMinimized(hwnd_val) => {
                        let hwnd = HWND(hwnd_val as *mut std::ffi::c_void);
                        println!("Event: Window Minimized {:?}", hwnd);
                        wm.handle_window_minimized(hwnd);
                    }
                    WindowEvent::WindowMoved(hwnd_val) => {
                        let hwnd = HWND(hwnd_val as *mut std::ffi::c_void);
                        // Only process move events if not from our own positioning
                        if !wm.is_positioning_window(hwnd) {
                            wm.update_window_positions();
                        }
                    }
                    WindowEvent::FocusChanged(_hwnd_val) => {
                        wm.update_decorations();
                    }
                    WindowEvent::DisplayChange => {
                        println!("Event: Display Change");
                        if let Err(e) = wm.reenumerate_monitors() {
                            eprintln!("Failed to reenumerate monitors: {}", e);
                        }
                    }
                    WindowEvent::PeriodicCheck => {
                        // Don't call update_window_positions here - it causes feedback loops
                        // WindowMoved events will handle position updates
                        wm.update_decorations();
                        wm.cleanup_minimized_windows();
                        if wm.check_monitor_changes() {
                            println!("Monitor change detected by periodic check");
                            if let Err(e) = wm.reenumerate_monitors() {
                                eprintln!("Failed to reenumerate monitors: {}", e);
                            }
                        }
                    }
                    WindowEvent::TrayExit => {
                        println!("Exiting MegaTile...");
                        cleanup_on_exit(&mut wm);
                        hotkey_manager.unregister_all(hwnd);
                        shutdown_gdiplus();
                        return;
                    }
                }
            } else {
                break;
            }
        }

        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Window procedure for the hidden message window.
extern "system" fn window_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        if msg == WM_DESTROY {
            PostQuitMessage(0);
        }
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}

/// Creates a hidden window for receiving hotkey and system messages.
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
