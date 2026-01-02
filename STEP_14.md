# STEP 13: Toggle Fullscreen (Win + F)

## Objective

Implement fullscreen toggle functionality to switch the focused window between tiled and fullscreen mode.

## Tasks

### 13.1 Add Fullscreen State to Window

Update `src/workspace.rs`:

```rust
#[derive(Debug, Clone)]
pub struct Window {
    pub hwnd: HWND,
    pub title: String,
    pub workspace: u8,
    pub monitor: usize,
    pub rect: RECT,
    pub is_focused: bool,
    pub original_rect: RECT,
    pub is_fullscreen: bool,  // Add fullscreen state
}
```

### 13.2 Add Fullscreen Functions

Update `src/windows.rs`:

```rust
use windows::Win32::UI::WindowsAndMessaging::*;

pub fn set_window_fullscreen(hwnd: HWND, monitor_rect: RECT) -> Result<(), String> {
    unsafe {
        // Store original placement
        let mut placement = WINDOWPLACEMENT {
            length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
            ..Default::default()
        };

        if GetWindowPlacement(hwnd, &mut placement).as_bool() {
            // Set window to fullscreen
            SetWindowPos(
                hwnd,
                HWND_TOPMOST,
                monitor_rect.left,
                monitor_rect.top,
                monitor_rect.right - monitor_rect.left,
                monitor_rect.bottom - monitor_rect.top,
                SWP_SHOWWINDOW | SWP_NOZORDER,
            ).ok();
        }

        Ok(())
    }
}

pub fn restore_window_from_fullscreen(hwnd: HWND, original_rect: RECT) -> Result<(), String> {
    unsafe {
        // Restore original position and size
        SetWindowPos(
            hwnd,
            HWND::default(),
            original_rect.left,
            original_rect.top,
            original_rect.right - original_rect.left,
            original_rect.bottom - original_rect.top,
            SWP_SHOWWINDOW | SWP_NOZORDER | SWP_NOACTIVATE,
        ).ok();

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
```

### 13.3 Add Fullscreen Toggle to Workspace Manager

Update `src/workspace_manager.rs`:

```rust
impl WorkspaceManager {
    // ... existing methods ...

    pub fn toggle_fullscreen(&mut self) -> Result<(), String> {
        // Get currently focused window
        let focused = self.get_focused_window();

        if focused.is_none() {
            return Err("No focused window".to_string());
        }

        let focused_hwnd = focused.unwrap().hwnd;

        // Find and update the window in workspace
        let mut monitors = self.monitors.lock().unwrap();

        for monitor in monitors.iter_mut() {
            let active_workspace_idx = (monitor.active_workspace - 1) as usize;
            if let Some(workspace) = monitor.get_workspace_mut((monitor.active_workspace)) {
                if let Some(window) = workspace.get_window_mut(focused_hwnd) {
                    let monitor_rect = monitor.rect;

                    if window.is_fullscreen {
                        // Restore from fullscreen
                        println!("Restoring window {:?} from fullscreen", focused_hwnd);
                        crate::windows::restore_window_from_fullscreen(
                            focused_hwnd,
                            window.original_rect
                        )?;
                        window.is_fullscreen = false;
                    } else {
                        // Set to fullscreen
                        println!("Setting window {:?} to fullscreen", focused_hwnd);
                        window.original_rect = window.rect; // Store current position
                        crate::windows::set_window_fullscreen(focused_hwnd, monitor_rect)?;
                        window.is_fullscreen = true;
                    }

                    return Ok(());
                }
            }
        }

        Err("Window not found".to_string())
    }

    pub fn exit_fullscreen_all(&mut self) {
        let mut monitors = self.monitors.lock().unwrap();

        for monitor in monitors.iter_mut() {
            for workspace in &mut monitor.workspaces {
                for window in &mut workspace.windows {
                    if window.is_fullscreen {
                        crate::windows::restore_window_from_fullscreen(
                            window.hwnd,
                            window.original_rect
                        ).ok();
                        window.is_fullscreen = false;
                    }
                }
            }
        }
    }
}
```

### 13.4 Update Hotkey Handler

Update `src/main.rs`:

```rust
fn handle_hotkey(action: hotkeys::HotkeyAction, workspace_manager: &Arc<Mutex<WorkspaceManager>>) {
    match action {
        hotkeys::HotkeyAction::SwitchWorkspace(num) => {
            let mut wm = workspace_manager.lock().unwrap();
            match wm.switch_workspace_with_windows(num) {
                Ok(()) => {
                    let window_count = wm.get_active_workspace_window_count();
                    println!("Switched to workspace {} ({} windows)", num, window_count);
                }
                Err(e) => eprintln!("Failed to switch workspace: {}", e),
            }
        }
        hotkeys::HotkeyAction::MoveToWorkspace(num) => {
            let mut wm = workspace_manager.lock().unwrap();
            match wm.move_window_to_workspace(num) {
                Ok(()) => {
                    println!("Moved window to workspace {}", num);
                    wm.print_workspace_status();
                }
                Err(e) => eprintln!("Failed to move window: {}", e),
            }
        }
        hotkeys::HotkeyAction::CloseWindow => {
            let mut wm = workspace_manager.lock().unwrap();
            match wm.close_focused_window() {
                Ok(()) => println!("Window closed successfully"),
                Err(e) => eprintln!("Failed to close window: {}", e),
            }
        }
        hotkeys::HotkeyAction::ToggleTiling => {
            let wm = workspace_manager.lock().unwrap();
            let algorithm = wm.toggle_tiling_algorithm();
            println!("Now using {:?} tiling", algorithm);
        }
        hotkeys::HotkeyAction::ToggleFullscreen => {
            let mut wm = workspace_manager.lock().unwrap();
            match wm.toggle_fullscreen() {
                Ok(()) => println!("Fullscreen toggled"),
                Err(e) => eprintln!("Failed to toggle fullscreen: {}", e),
            }
        }
        // ... other actions ...
    }
}
```

### 13.5 Handle Fullscreen on Workspace Switch

Update `src/workspace_manager.rs`:

```rust
impl WorkspaceManager {
    pub fn switch_workspace_with_windows(&mut self, new_workspace: u8) -> Result<(), String> {
        if new_workspace < 1 || new_workspace > 9 {
            return Err("Invalid workspace number".to_string());
        }

        let old_workspace = self.active_workspace_global;

        if old_workspace == new_workspace {
            return Ok(()); // No change needed
        }

        println!("Switching from workspace {} to {}", old_workspace, new_workspace);

        // Exit fullscreen on all windows in old workspace
        self.exit_fullscreen_workspace(old_workspace);

        // Hide windows from old workspace
        self.hide_workspace_windows(old_workspace)?;

        // Show windows from new workspace
        self.show_workspace_windows(new_workspace)?;

        // Update active workspace
        self.active_workspace_global = new_workspace;

        // Update all monitors
        let mut monitors = self.monitors.lock().unwrap();
        for monitor in monitors.iter_mut() {
            monitor.set_active_workspace(new_workspace);
        }

        // Tile and position windows in new workspace
        drop(monitors);
        let mut wm = self.monitors.lock().unwrap();
        wm.tile_active_workspaces();
        wm.apply_window_positions();

        Ok(())
    }

    fn exit_fullscreen_workspace(&self, workspace_num: u8) {
        let mut monitors = self.monitors.lock().unwrap();

        for monitor in monitors.iter_mut() {
            if let Some(workspace) = monitor.get_workspace_mut(workspace_num) {
                for window in &mut workspace.windows {
                    if window.is_fullscreen {
                        crate::windows::restore_window_from_fullscreen(
                            window.hwnd,
                            window.original_rect
                        ).ok();
                        window.is_fullscreen = false;
                    }
                }
            }
        }
    }
}
```

## Testing

1. Run the application
2. Open multiple applications
3. Focus one window
4. Press Win + F to toggle fullscreen
5. Verify:
   - Window expands to fill entire monitor
   - Console logs fullscreen activation
6. Press Win + F again
7. Verify:
   - Window returns to tiled position
   - Window is properly tiled with other windows
8. Switch workspaces while window is fullscreen
9. Verify:
   - Window exits fullscreen before hiding
   - Window appears in tiled state when switching back

## Success Criteria

- [ ] Win + F toggles fullscreen on focused window
- [ ] Window expands to fill entire monitor
- [ ] Window restores to tiled position
- [ ] Tiling is maintained after restoring from fullscreen
- [ ] Fullscreen windows exit fullscreen on workspace switch
- [ ] Console logs fullscreen state changes

## Documentation

### Fullscreen Toggle Flow

**Enter fullscreen**:
1. Get focused window
2. Store current position in `original_rect`
3. Get monitor rectangle
4. Set window to monitor rectangle size
5. Set `is_fullscreen = true`

**Exit fullscreen**:
1. Get focused window
2. Restore `original_rect` position
3. Set `is_fullscreen = false`

**Switch workspace**:
1. Exit fullscreen on all windows in old workspace
2. Hide/show windows as normal
3. Windows appear in tiled state in new workspace

### Fullscreen Implementation

**API used**:
- `SetWindowPos` with monitor rect
- `HWND_TOPMOST` for z-order (can be adjusted)
- `SWP_SHOWWINDOW` flag

**Positioning**:
- Fullscreen: `monitor_rect` (entire monitor)
- Tiled: `original_rect` (saved tiled position)

### Edge Cases

**Multiple fullscreen windows**:
- Only one window can be truly fullscreen at a time
- Other fullscreen windows should be restored when switching focus

**Fullscreen on workspace switch**:
- All fullscreen windows in old workspace are restored
- Prevents windows from being hidden while fullscreen
- Ensures clean state when returning

**Monitor configuration changes**:
- Fullscreen windows may need to be repositioned
- Monitor disconnection not currently handled

### Window Behavior

**Supported**:
- Most standard applications
- Applications that respond to `SetWindowPos`

**Potential issues**:
- Apps with own fullscreen mode (games, video players)
- Apps that resist window positioning
- Apps with complex window hierarchies

### Future Enhancements

1. **Per-window fullscreen**: Store fullscreen state per window
2. **Fullscreen with decorations**: Preserve title bar, etc.
3. **Fullscreen on specific monitor**: Support multi-monitor fullscreen
4. **Auto-exit fullscreen**: Detect external fullscreen events

## Next Steps

Proceed to [STEP_14.md](STEP_14.md) to implement multi-monitor support enhancements.
