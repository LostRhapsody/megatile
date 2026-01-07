//! # Megatile - A Tiling Window Manager for Windows
//!
//! Megatile is a lightweight tiling window manager designed for Windows 10/11.
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
mod logging;
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

use log::{debug, error, info};

use hotkeys::HotkeyManager;
use statusbar::{
    STATUSBAR_HEIGHT, STATUSBAR_TOP_GAP, STATUSBAR_WIDTH, StatusBar, init_gdiplus, shutdown_gdiplus,
};
use tray::TrayManager;
use windows_lib::get_process_name_for_window;
use windows_lib::{
    enumerate_monitors, get_normal_windows, reset_window_decorations, show_window_in_taskbar,
};
use workspace_manager::WorkspaceManager;

use argh::FromArgs;
use logging::LogLevel;

/// Megatile - A Tiling Window Manager for Windows
#[derive(FromArgs, Debug)]
struct Args {
    /// set log level to debug (most verbose)
    #[argh(switch, short = 'd')]
    debug: bool,

    /// set log level to info
    #[argh(switch, short = 'i')]
    info: bool,

    /// set log level to warning
    #[argh(switch, short = 'w')]
    warning: bool,

    #[allow(dead_code)]
    /// set log level to error (default, least verbose)
    #[argh(switch, short = 'e')]
    error: bool,
}

/// Window class name for the hidden message window ("MegatileMessageWindow" as UTF-16).
static CLASS_NAME: [u16; 22] = [
    77, 101, 103, 97, 84, 105, 108, 101, 77, 101, 115, 115, 97, 103, 101, 87, 105, 110, 100, 111,
    119, 0,
];

/// Window title ("Megatile" as UTF-16).
static TITLE: [u16; 9] = [77, 101, 103, 97, 84, 105, 108, 101, 0];

/// Internal events processed by the main event loop.
#[derive(Debug)]
enum WindowEvent {
    Hotkey(hotkeys::HotkeyAction),
    WindowCreated(isize),
    WindowDestroyed(isize),
    WindowMinimized(isize),
    WindowRestored(isize),
    WindowMoved(isize),
    WindowHidden(isize), // New: fires when WS_VISIBLE is cleared
    FocusChanged(isize),
    DisplayChange,
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
        EVENT_OBJECT_HIDE => {
            // Fires when a window's WS_VISIBLE style is cleared
            // This catches apps like Zoom that hide windows instead of destroying them
            push_event(WindowEvent::WindowHidden(hwnd.0 as isize));
        }
        EVENT_SYSTEM_MINIMIZESTART => {
            push_event(WindowEvent::WindowMinimized(hwnd.0 as isize));
        }
        EVENT_SYSTEM_MINIMIZEEND => {
            push_event(WindowEvent::WindowRestored(hwnd.0 as isize));
        }
        EVENT_OBJECT_LOCATIONCHANGE => {
            push_event(WindowEvent::WindowMoved(hwnd.0 as isize));
        }
        _ => {}
    }
}

/// Restores all managed windows to their visible state before exit.
///
/// This ensures windows are not left hidden in the taskbar when Megatile exits.
fn cleanup_on_exit(wm: &mut WorkspaceManager) {
    info!("Restoring all hidden windows...");

    // Get all managed windows from all workspaces
    let all_hwnds = wm.get_all_managed_hwnds();
    debug!("Found {} managed windows to restore", all_hwnds.len());

    let normal_windows = get_normal_windows();
    debug!("Found {} normal windows to restore", normal_windows.len());
    for window_info in normal_windows {
        debug!(
            "Window: {} (Class: {})",
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
                debug!("Restored window {:?}", hwnd);
            }
            Err(e) => {
                failed_count += 1;
                error!("Failed to restore window {:?}: {}", hwnd, e);
            }
        }
        if let Err(e) = reset_window_decorations(hwnd_handle) {
            error!("Failed to reset window decorations for {:?}: {}", hwnd, e);
        }
    }

    info!(
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
                    info!("Switched to workspace {}", num);
                    // Clean up any invalid/zombie windows before tiling
                    wm.cleanup_invalid_windows();
                    wm.tile_active_workspaces();
                    wm.apply_window_positions();
                }
                Err(e) => error!("Failed to switch workspace: {}", e),
            }
        }
        hotkeys::HotkeyAction::MoveLeft => {
            if let Err(e) = wm.move_window(workspace_manager::FocusDirection::Left) {
                error!("Failed to move window: {}", e);
            }
        }
        hotkeys::HotkeyAction::MoveRight => {
            if let Err(e) = wm.move_window(workspace_manager::FocusDirection::Right) {
                error!("Failed to move window: {}", e);
            }
        }
        hotkeys::HotkeyAction::FocusLeft => {
            if let Err(e) = wm.move_focus(workspace_manager::FocusDirection::Left) {
                error!("Failed to move focus: {}", e);
            }
        }
        hotkeys::HotkeyAction::FocusRight => {
            if let Err(e) = wm.move_focus(workspace_manager::FocusDirection::Right) {
                error!("Failed to move focus: {}", e);
            }
        }
        hotkeys::HotkeyAction::FocusUp => {
            if let Err(e) = wm.move_focus(workspace_manager::FocusDirection::Up) {
                error!("Failed to move focus: {}", e);
            }
        }
        hotkeys::HotkeyAction::FocusDown => {
            if let Err(e) = wm.move_focus(workspace_manager::FocusDirection::Down) {
                error!("Failed to move focus: {}", e);
            }
        }
        hotkeys::HotkeyAction::MoveUp => {
            if let Err(e) = wm.move_window(workspace_manager::FocusDirection::Up) {
                error!("Failed to move window: {}", e);
            }
        }
        hotkeys::HotkeyAction::MoveDown => {
            if let Err(e) = wm.move_window(workspace_manager::FocusDirection::Down) {
                error!("Failed to move window: {}", e);
            }
        }
        hotkeys::HotkeyAction::MoveToWorkspace(num) => match wm.move_window_to_workspace(num) {
            Ok(()) => {
                info!("Moved window to workspace {}", num);
                wm.print_workspace_status();
            }
            Err(e) => error!("Failed to move window: {}", e),
        },
        hotkeys::HotkeyAction::ToggleTiling => {
            if let Some(focused) = wm.get_focused_window()
                && let Err(e) = wm.toggle_window_tiling(HWND(focused.hwnd as _))
            {
                error!("Failed to toggle tiling: {}", e);
            }
        }
        hotkeys::HotkeyAction::ToggleFullscreen => match wm.toggle_fullscreen() {
            Ok(()) => info!("Fullscreen toggled"),
            Err(e) => error!("Failed to toggle fullscreen: {}", e),
        },
        hotkeys::HotkeyAction::ResizeHorizontalIncrease => {
            if let Err(e) =
                wm.resize_focused_window(workspace_manager::ResizeDirection::Horizontal, 0.05)
            {
                error!("Failed to resize window: {}", e);
            }
        }
        hotkeys::HotkeyAction::ResizeHorizontalDecrease => {
            if let Err(e) =
                wm.resize_focused_window(workspace_manager::ResizeDirection::Horizontal, -0.05)
            {
                error!("Failed to resize window: {}", e);
            }
        }
        hotkeys::HotkeyAction::ResizeVerticalIncrease => {
            if let Err(e) =
                wm.resize_focused_window(workspace_manager::ResizeDirection::Vertical, 0.05)
            {
                error!("Failed to resize window: {}", e);
            }
        }
        hotkeys::HotkeyAction::ResizeVerticalDecrease => {
            if let Err(e) =
                wm.resize_focused_window(workspace_manager::ResizeDirection::Vertical, -0.05)
            {
                error!("Failed to resize window: {}", e);
            }
        }
        hotkeys::HotkeyAction::FlipRegion => {
            if let Err(e) = wm.flip_focused_region() {
                error!("Failed to flip region: {}", e);
            }
        }
        hotkeys::HotkeyAction::CloseWindow => match wm.close_focused_window() {
            Ok(()) => info!("Window closed successfully"),
            Err(e) => error!("Failed to close window: {}", e),
        },
        hotkeys::HotkeyAction::ToggleStatusBar => {
            wm.invert_statusbar_visibility();
        }
        hotkeys::HotkeyAction::MoveToMonitorLeft => {
            if let Err(e) = wm.move_window_to_monitor(workspace_manager::FocusDirection::Left) {
                error!("Failed to move window to monitor: {}", e);
            }
        }
        hotkeys::HotkeyAction::MoveToMonitorRight => {
            if let Err(e) = wm.move_window_to_monitor(workspace_manager::FocusDirection::Right) {
                error!("Failed to move window to monitor: {}", e);
            }
        }
        hotkeys::HotkeyAction::MoveToMonitorUp => {
            if let Err(e) = wm.move_window_to_monitor(workspace_manager::FocusDirection::Up) {
                error!("Failed to move window to monitor: {}", e);
            }
        }
        hotkeys::HotkeyAction::MoveToMonitorDown => {
            if let Err(e) = wm.move_window_to_monitor(workspace_manager::FocusDirection::Down) {
                error!("Failed to move window to monitor: {}", e);
            }
        }
    }
}

fn main() {
    // Parse CLI arguments
    let args: Args = argh::from_env();

    // Determine log level from CLI flags (default to Error if none specified)
    let log_level = if args.debug {
        LogLevel::Debug
    } else if args.info {
        LogLevel::Info
    } else if args.warning {
        LogLevel::Warning
    } else {
        LogLevel::Error
    };

    // Initialize logging (must be done before any log macros)
    let _logger_handle = logging::init_logging(log_level).expect("Failed to initialize logging");

    log::info!("Megatile - Window Manager");

    // Initialize event queue
    EVENT_QUEUE.set(Mutex::new(VecDeque::new())).unwrap();

    // Initialize workspace manager
    let mut wm = WorkspaceManager::new();

    // Setup Ctrl+C handler for cleanup
    ctrlc::set_handler(move || {
        info!("\nReceived Ctrl+C signal, pushing exit event...");
        push_event(WindowEvent::TrayExit);
    })
    .expect("Error setting Ctrl+C handler");

    // Enumerate monitors and create monitor structs
    let monitor_infos = enumerate_monitors();
    info!("Found {} monitor(s)", monitor_infos.len());

    let monitors: Vec<workspace::Monitor> = monitor_infos
        .iter()
        .enumerate()
        .map(|(i, info)| {
            debug!("Monitor {}: {:?}", i + 1, info.rect);
            workspace::Monitor::new(info.hmonitor, info.rect)
        })
        .collect();

    wm.set_monitors(monitors);

    // Enumerate windows and assign to workspace 1
    let normal_windows = get_normal_windows();
    info!("Found {} normal windows", normal_windows.len());

    let focused_hwnd = unsafe { GetForegroundWindow() };
    for window_info in normal_windows {
        debug!(
            "Window: {} (Class: {})",
            window_info.title, window_info.class_name
        );
        let is_focused = window_info.hwnd == focused_hwnd;
        let monitor_index = wm.get_monitor_for_window(window_info.hwnd).unwrap_or(0);
        let process_name = get_process_name_for_window(window_info.hwnd);
        let mut window = workspace::Window::new(
            window_info.hwnd.0 as isize,
            1, // Assign to workspace 1
            monitor_index,
            window_info.rect,
            process_name,
        );
        window.is_focused = is_focused;
        // Since workspace 1 is active, show in taskbar
        let _ = show_window_in_taskbar(window_info.hwnd);
        wm.add_window(window);
    }

    info!("Assigned all windows to workspace 1");

    // Apply initial tiling
    wm.tile_active_workspaces();
    wm.apply_window_positions();
    info!("Applied initial tiling to workspace 1");

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

    info!("Megatile is running. Use the tray icon to exit.");

    let mut last_monitor_check = Instant::now();
    let monitor_check_interval = Duration::from_millis(100);
    let mut last_clock_update = Instant::now();
    let clock_update_interval = Duration::from_secs(1);

    // Main event loop
    loop {
        // 1. Check monitor configuration first (every 100ms)
        if last_monitor_check.elapsed() >= monitor_check_interval {
            if wm.check_monitor_changes() {
                info!("Monitor change detected in main loop");
                if let Err(e) = wm.reenumerate_monitors() {
                    error!("Failed to reenumerate monitors: {}", e);
                } else {
                    // Recenter status bar on primary monitor after monitor changes
                    wm.recenter_statusbar();
                }
            }
            // Periodic maintenance tasks
            wm.update_decorations();
            wm.cleanup_minimized_windows();
            last_monitor_check = Instant::now();
        }

        // 2. Update status bar clock (every second)
        if last_clock_update.elapsed() >= clock_update_interval {
            wm.update_statusbar_clock();
            last_clock_update = Instant::now();
        }

        // 3. Check for tray exit
        if tray.should_exit() {
            push_event(WindowEvent::TrayExit);
        }

        // 4. Process window messages
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

        // 5. Process all events from the queue per iteration
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
                            info!("Event: Window Registered {:?}", hwnd);
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
                            let process_name = get_process_name_for_window(hwnd);
                            let window = workspace::Window::new(
                                hwnd_val,
                                active_workspace,
                                monitor_index,
                                info.rect,
                                process_name,
                            );
                            let _ = show_window_in_taskbar(hwnd);
                            wm.add_window(window);
                            wm.tile_active_workspaces();
                            wm.apply_window_positions();
                        }
                    }
                    WindowEvent::WindowDestroyed(hwnd_val) => {
                        let hwnd = HWND(hwnd_val as *mut std::ffi::c_void);
                        info!("Event: Window Destroyed {:?}", hwnd);
                        wm.remove_window_with_tiling(hwnd);
                    }
                    WindowEvent::WindowMinimized(hwnd_val) => {
                        let hwnd = HWND(hwnd_val as *mut std::ffi::c_void);
                        info!("Event: Window Minimized {:?}", hwnd);
                        wm.handle_window_minimized(hwnd);
                    }
                    WindowEvent::WindowHidden(hwnd_val) => {
                        let hwnd = HWND(hwnd_val as *mut std::ffi::c_void);

                        // Only treat as zombie if window is in active workspace
                        // Windows in inactive workspaces are supposed to be hidden (workspace switching)
                        if wm.is_window_in_active_workspace(hwnd) {
                            info!(
                                "Event: Window Hidden {:?} in active workspace (zombie)",
                                hwnd
                            );
                            // This is unexpected - window in active workspace shouldn't be hidden
                            // Likely a zombie window (app hid it without destroying)
                            wm.remove_window_with_tiling(hwnd);
                        } else {
                            debug!(
                                "Event: Window Hidden {:?} in inactive workspace (expected)",
                                hwnd
                            );
                            // This is expected - workspace switching hides windows
                            // Don't remove it
                        }
                    }
                    WindowEvent::WindowRestored(hwnd_val) => {
                        let hwnd = HWND(hwnd_val as *mut std::ffi::c_void);
                        info!("Event: Window Restored {:?}", hwnd);
                        wm.handle_window_restored(hwnd);
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
                        info!("Event: Display Change");
                        if let Err(e) = wm.reenumerate_monitors() {
                            error!("Failed to reenumerate monitors: {}", e);
                        } else {
                            // Recenter status bar on primary monitor after display change
                            wm.recenter_statusbar();
                        }
                    }
                    WindowEvent::TrayExit => {
                        info!("Exiting Megatile...");
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
