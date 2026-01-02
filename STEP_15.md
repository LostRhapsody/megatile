# STEP 14: Multi-Monitor Support

## Objective

Enhance multi-monitor support to handle monitor hot-plug events and ensure independent tiling on each monitor within the same workspace.

## Tasks

### 14.1 Add Monitor Hot-Plug Detection

Update `src/windows.rs`:

```rust
use windows::Win32::UI::WindowsAndMessaging::*;

pub fn register_display_change() {
    unsafe {
        // Register for display change notifications
        // This will be used to re-enumerate monitors when configuration changes
    }
}

pub fn check_monitor_change() -> bool {
    // Compare current monitor configuration with previous
    // Return true if monitors changed
    false // Placeholder
}
```

### 14.2 Add Monitor Re-enumeration

Update `src/workspace_manager.rs`:

```rust
impl WorkspaceManager {
    // ... existing methods ...

    pub fn reenumerate_monitors(&mut self) -> Result<(), String> {
        println!("Re-enumerating monitors...");

        // Get current monitor info
        let monitor_infos = windows::enumerate_monitors();
        println!("Found {} monitor(s)", monitor_infos.len());

        // Update monitors
        let mut new_monitors: Vec<Monitor> = Vec::new();

        for (i, info) in monitor_infos.iter().enumerate() {
            println!("  Monitor {}: {:?}", i, info.rect);

            // Try to preserve workspace data from existing monitor
            let existing_workspace_data = if let Some(old_monitor) = self.get_monitors().get(i) {
                old_monitor.workspaces.clone()
            } else {
                [
                    Workspace::new(),
                    Workspace::new(),
                    Workspace::new(),
                    Workspace::new(),
                    Workspace::new(),
                    Workspace::new(),
                    Workspace::new(),
                    Workspace::new(),
                    Workspace::new(),
                ]
            };

            let mut monitor = Monitor::new(info.hmonitor, info.rect);
            monitor.workspaces = existing_workspace_data;
            monitor.active_workspace = self.active_workspace_global;
            new_monitors.push(monitor);
        }

        // Update monitors
        self.set_monitors(new_monitors);

        // Re-tile active workspace on all monitors
        let mut wm = self.monitors.lock().unwrap();
        wm.tile_active_workspaces();
        wm.apply_window_positions();

        println!("Monitor re-enumeration complete");
        Ok(())
    }

    pub fn get_monitor_for_window(&self, hwnd: isize) -> Option<usize> {
        let monitors = self.monitors.lock().unwrap();

        for (i, monitor) in monitors.iter().enumerate() {
            if let Some(workspace) = monitor.get_workspace(monitor.active_workspace) {
                for window in &workspace.windows {
                    if window.hwnd == hwnd {
                        return Some(i);
                    }
                }
            }
        }

        None
    }
}
```

### 14.3 Improve Window-Monitor Assignment

Update `src/main.rs` to properly assign windows to monitors:

```rust
use windows::Win32::Graphics::Gdi::MonitorFromWindow;

fn assign_window_to_monitor(hwnd: isize) -> usize {
    unsafe {
        let hmonitor = MonitorFromWindow(HWND(hwnd), MONITOR_DEFAULTTONEAREST);

        let monitors = windows::enumerate_monitors();
        for (i, monitor_info) in monitors.iter().enumerate() {
            if monitor_info.hmonitor == hmonitor.0 {
                return i;
            }
        }

        // Default to monitor 0 if not found
        0
    }
}

// In main(), when enumerating windows
for window_info in normal_windows {
    let monitor_idx = assign_window_to_monitor(window_info.hwnd.0);
    let mut window = workspace::Window::new(
        window_info.hwnd,
        1, // Workspace 1
        monitor_idx,
        window_info.rect,
    );

    // Set focus state
    let focused_hwnd = unsafe { GetForegroundWindow() };
    window.is_focused = window.hwnd == focused_hwnd;

    wm.add_window(window);
}
```

### 14.4 Add Periodic Monitor Check

Update `src/main.rs`:

```rust
fn main() {
    // ... existing setup ...

    // Last monitor check time
    let mut last_monitor_check = std::time::Instant::now();
    const MONITOR_CHECK_INTERVAL: Duration = Duration::from_secs(5);

    // Main event loop
    loop {
        if tray.should_exit() {
            println!("Exiting MegaTile...");
            hotkey_manager.unregister_all(hwnd);
            break;
        }

        // Check for monitor changes periodically
        if last_monitor_check.elapsed() >= MONITOR_CHECK_INTERVAL {
            // TODO: Implement proper monitor change detection
            // For now, just print that we're checking
            last_monitor_check = std::time::Instant::now();
        }

        // Process window messages
        let mut msg = MSG::default();
        while unsafe { PeekMessageW(&mut msg, HWND::default(), 0, 0, PM_REMOVE) }.as_bool() {
            unsafe {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);

                if msg.message == WM_HOTKEY {
                    let action = hotkey_manager.get_action(msg.wParam.0 as i32);
                    if let Some(action) = action {
                        handle_hotkey(action, &workspace_manager);
                    }
                } else if msg.message == WM_DISPLAYCHANGE {
                    // Display configuration changed
                    println!("Display configuration changed");
                    let mut wm = workspace_manager.lock().unwrap();
                    let _ = wm.reenumerate_monitors();
                } else if msg.message == WM_DESTROY {
                    PostQuitMessage(0);
                }
            }
        }

        std::thread::sleep(Duration::from_millis(10));
    }
}
```

### 14.5 Add Cross-Monitor Focus Movement

Update `src/workspace_manager.rs`:

```rust
impl WorkspaceManager {
    // ... existing methods ...

    pub fn move_focus_cross_monitor(&self, direction: FocusDirection) -> Result<(), String> {
        use windows::Win32::UI::WindowsAndMessaging::*;

        let focused = self.get_focused_window();

        if focused.is_none() {
            return Ok(()); // No window focused
        }

        let focused_window = focused.unwrap();
        let current_monitor_idx = focused_window.monitor;

        let monitors = self.monitors.lock().unwrap();
        let current_monitor = monitors.get(current_monitor_idx);

        if current_monitor.is_none() {
            return Err("Current monitor not found".to_string());
        }

        let current_monitor_rect = current_monitor.unwrap().rect;

        // Try to find window in current monitor first
        let mut active_windows: Vec<Window> = Vec::new();
        for monitor in monitors.iter() {
            let active_workspace = monitor.get_active_workspace();
            for window in &active_workspace.windows {
                active_windows.push(window.clone());
            }
        }

        // Find candidate windows in direction
        let target = self.find_next_focus(&focused_window, direction, &active_windows);

        if let Some(target_window) = target {
            // Check if target is on different monitor
            if target_window.monitor != current_monitor_idx {
                println!("Moving focus to different monitor: {} -> {}",
                         current_monitor_idx, target_window.monitor);
            }

            self.set_window_focus(target_window.hwnd);
        }

        Ok(())
    }
}
```

### 14.6 Update Focus Movement to Use Cross-Monitor

Update hotkey handler in `src/main.rs`:

```rust
hotkeys::HotkeyAction::FocusLeft => {
    let wm = workspace_manager.lock().unwrap();
    let _ = wm.move_focus_cross_monitor(workspace_manager::FocusDirection::Left);
}
hotkeys::HotkeyAction::FocusRight => {
    let wm = workspace_manager.lock().unwrap();
    let _ = wm.move_focus_cross_monitor(workspace_manager::FocusDirection::Right);
}
hotkeys::HotkeyAction::FocusUp => {
    let wm = workspace_manager.lock().unwrap();
    let _ = wm.move_focus_cross_monitor(workspace_manager::FocusDirection::Up);
}
hotkeys::HotkeyAction::FocusDown => {
    let wm = workspace_manager.lock().unwrap();
    let _ = wm.move_focus_cross_monitor(workspace_manager::FocusDirection::Down);
}
```

## Testing

1. Run the application with multiple monitors
2. Open applications on different monitors
3. Verify windows are assigned to correct monitors
4. Press Win + Right Arrow to move focus to right monitor
5. Verify focus moves correctly between monitors
6. Disconnect a monitor
7. Verify application handles monitor change (WM_DISPLAYCHANGE)
8. Reconnect monitor
9. Verify windows are re-tiled correctly

## Success Criteria

- [ ] Windows are correctly assigned to their monitors
- [ ] Focus movement works across monitors
- [ ] Monitor hot-plug events are detected
- [ ] Workspace data is preserved on monitor change
- [ ] Windows re-tile correctly on monitor configuration change
- [ ] Each monitor has independent tiling within same workspace

## Documentation

### Multi-Monitor Architecture

**Shared workspace model**:
- All monitors share same active workspace number
- Each monitor maintains independent window lists for each workspace
- Workspace 1 on Monitor A ≠ Workspace 1 on Monitor B
- Switching workspace affects all monitors simultaneously

**Independent tiling**:
- Each monitor tiles its own windows independently
- Different monitors can have different numbers of windows
- Window gaps apply per-monitor
- Tiling algorithm is global but applied per-monitor

### Monitor Assignment

**Window to monitor mapping**:
- Uses `MonitorFromWindow()` API to find window's monitor
- Updated when windows are added or moved
- Tracked in `Window.monitor` field

**Monitor enumeration**:
- Uses `EnumDisplayMonitors()` API
- Captures monitor handle and rectangle
- Detects primary monitor flag

### Monitor Hot-Plug Handling

**Display change detection**:
- `WM_DISPLAYCHANGE` message in window message loop
- Triggers monitor re-enumeration
- Preserves existing workspace data

**Re-enumeration process**:
1. Get new monitor list
2. Match with existing monitors by index
3. Preserve workspace data where possible
4. Create new monitor structs for new monitors
5. Re-tile all active workspaces

**Edge cases**:
- Monitor disconnect: Windows on disconnected monitor need reassignment
- Monitor addition: New monitor is empty
- Resolution change: Windows may need repositioning
- DPI change: Not currently handled

### Cross-Monitor Focus Movement

**Algorithm**:
1. Get currently focused window and its monitor
2. Search for windows in specified direction across ALL monitors
3. Select closest window in that direction
4. Move focus to target window

**Direction selection**:
- Left/Right: Uses horizontal position
- Up/Down: Uses vertical position
- Considers all active workspace windows on all monitors

### Workspace Synchronization

**All monitors, one workspace**:
- Win + 1 shows workspace 1 on ALL monitors
- Each monitor shows its own windows in workspace 1
- Switching workspace updates all monitors simultaneously

**Per-monitor window lists**:
- Monitor 0: Workspace 1 has [Window A, Window B]
- Monitor 1: Workspace 1 has [Window C, Window D, Window E]
- Both are workspace 1 but have different content

### Limitations

- No per-monitor workspace selection
- Monitor DPI changes not handled
- Window reassignment on disconnect may fail
- Monitor ordering may change on reconnection

### Future Enhancements

1. **Per-monitor workspaces**: Independent workspace per monitor
2. **Monitor-specific gaps**: Different gaps per monitor
3. **DPI awareness**: Handle DPI scaling changes
4. **Window reassignment**: Smart reassignment on monitor disconnect
5. **Monitor profiles**: Save/restore monitor configurations

## Next Steps

Proceed to [STEP_15.md](STEP_15.md) to implement auto-start configuration.
