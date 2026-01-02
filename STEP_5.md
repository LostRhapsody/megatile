# STEP 5: Window Hiding/Showing

## Objective

Implement window hiding and showing functionality for workspace switching. When switching away from a workspace, hide all windows from the taskbar. When switching to a workspace, show all windows again.

## Tasks

### 5.1 Add Window Hiding Functions

Update `src/windows.rs`:

```rust
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::*;

pub fn hide_window_from_taskbar(hwnd: HWND) -> Result<(), String> {
    unsafe {
        // Store original window placement
        let mut placement = WINDOWPLACEMENT {
            length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
            ..Default::default()
        };

        if GetWindowPlacement(hwnd, &mut placement).as_bool() {
            // Hide the window
            ShowWindow(hwnd, SW_HIDE).ok();

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
        ShowWindow(hwnd, SW_SHOW).ok();

        // Restore original placement
        let mut placement = WINDOWPLACEMENT {
            length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
            ..Default::default()
        };

        if GetWindowPlacement(hwnd, &mut placement).as_bool() {
            ShowWindow(hwnd, placement.showCmd).ok();
            SetWindowPlacement(hwnd, &placement).ok();
        }

        Ok(())
    }
}

pub fn is_window_hidden(hwnd: HWND) -> bool {
    unsafe {
        !IsWindowVisible(hwnd).as_bool()
    }
}
```

### 5.2 Add Workspace Switching Logic

Update `src/workspace_manager.rs`:

```rust
use crate::windows::{hide_window_from_taskbar, show_window_in_taskbar};
use windows::Win32::Foundation::HWND;

impl WorkspaceManager {
    // ... existing methods ...

    pub fn switch_workspace_with_windows(&mut self, new_workspace: u8) -> Result<(), String> {
        if new_workspace < 1 || new_workspace > 9 {
            return Err("Invalid workspace number".to_string());
        }

        let old_workspace = self.active_workspace_global;

        if old_workspace == new_workspace {
            return Ok(()); // No change needed
        }

        println!("Switching from workspace {} to {}", old_workspace, new_workspace);

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

        Ok(())
    }

    fn hide_workspace_windows(&self, workspace_num: u8) -> Result<(), String> {
        let mut monitors = self.monitors.lock().unwrap();

        for monitor in monitors.iter_mut() {
            if let Some(workspace) = monitor.get_workspace_mut(workspace_num) {
                for window in &workspace.windows {
                    if let Err(e) = hide_window_from_taskbar(window.hwnd) {
                        eprintln!("Failed to hide window {:?}: {}", window.hwnd, e);
                    }
                }
            }
        }

        Ok(())
    }

    fn show_workspace_windows(&self, workspace_num: u8) -> Result<(), String> {
        let mut monitors = self.monitors.lock().unwrap();

        for monitor in monitors.iter_mut() {
            if let Some(workspace) = monitor.get_workspace_mut(workspace_num) {
                for window in &workspace.windows {
                    if let Err(e) = show_window_in_taskbar(window.hwnd) {
                        eprintln!("Failed to show window {:?}: {}", window.hwnd, e);
                    }
                }
            }
        }

        Ok(())
    }

    pub fn move_window_to_workspace(&mut self, hwnd: HWND, new_workspace: u8) -> Result<(), String> {
        if new_workspace < 1 || new_workspace > 9 {
            return Err("Invalid workspace number".to_string());
        }

        let mut monitors = self.monitors.lock().unwrap();

        // Find and remove window from its current workspace
        let mut window_to_move = None;
        let mut current_workspace = 0;
        let mut monitor_index = 0;

        for (m_idx, monitor) in monitors.iter_mut().enumerate() {
            for w_idx in 0..9 {
                if let Some(workspace) = monitor.get_workspace_mut((w_idx + 1) as u8) {
                    if let Some(window) = workspace.remove_window(hwnd) {
                        window_to_move = Some(window);
                        current_workspace = w_idx + 1;
                        monitor_index = m_idx;
                        break;
                    }
                }
            }
            if window_to_move.is_some() {
                break;
            }
        }

        if let Some(mut window) = window_to_move {
            // Update window's workspace
            window.workspace = new_workspace;

            // Add to new workspace
            if let Some(monitor) = monitors.get_mut(monitor_index) {
                if let Some(workspace) = monitor.get_workspace_mut(new_workspace) {
                    workspace.add_window(window);
                }
            }

            // If moving from active workspace, hide it
            if current_workspace == self.active_workspace_global {
                hide_window_from_taskbar(hwnd).ok();
            }
            // If moving to active workspace, show it
            else if new_workspace == self.active_workspace_global {
                show_window_in_taskbar(hwnd).ok();
            }

            Ok(())
        } else {
            Err("Window not found".to_string())
        }
    }
}
```

### 5.3 Update Hotkey Handler

Update `src/main.rs`:

```rust
fn handle_hotkey(action: hotkeys::HotkeyAction, workspace_manager: &Arc<Mutex<WorkspaceManager>>) {
    match action {
        hotkeys::HotkeyAction::SwitchWorkspace(num) => {
            let mut wm = workspace_manager.lock().unwrap();
            match wm.switch_workspace_with_windows(num) {
                Ok(()) => println!("Switched to workspace {}", num),
                Err(e) => eprintln!("Failed to switch workspace: {}", e),
            }
        }
        hotkeys::HotkeyAction::MoveToWorkspace(num) => {
            // TODO: Get currently focused window
            println!("Move to workspace {} (not yet implemented)", num);
        }
        _ => {
            println!("Hotkey action: {:?}", action);
        }
    }
}
```

### 5.4 Add Test Window Assignment

Update `src/main.rs` to assign initial windows to workspace 1:

```rust
fn main() {
    // ... existing setup code ...

    // Enumerate windows and assign to workspace 1
    let normal_windows = get_normal_windows();
    println!("Found {} normal windows", normal_windows.len());

    let mut wm = workspace_manager.lock().unwrap();
    for window_info in normal_windows {
        let window = workspace::Window::new(
            window_info.hwnd,
            1, // Assign to workspace 1
            0, // TODO: Determine which monitor
            window_info.rect,
        );
        wm.add_window(window);
    }
    drop(wm);

    // ... rest of main function ...
}
```

## Testing

1. Run the application
2. Open multiple applications (Notepad, Calculator, File Explorer, etc.)
3. Press Win + 2 to switch to workspace 2
4. Verify that all windows disappear from the taskbar
5. Press Win + 1 to switch back to workspace 1
6. Verify that all windows reappear in the taskbar
7. Test workspace switching between 1-9

## Success Criteria

- [ ] Windows hide from taskbar when switching away from workspace
- [ ] Windows show in taskbar when switching to workspace
- [ ] Workspace switching works smoothly
- [ ] No errors in console output
- [ ] Windows are restored to correct positions

## Documentation

### Window Hiding Approach

**Hiding windows**:
1. Store current window placement
2. Call `ShowWindow(hwnd, SW_HIDE)` to hide the window
3. Remove `WS_EX_APPWINDOW` style to hide from taskbar

**Showing windows**:
1. Restore `WS_EX_APPWINDOW` style
2. Call `ShowWindow(hwnd, SW_SHOW)` to make visible
3. Restore original window placement

**Limitations**:
- Windows are hidden but not minimized
- Some applications may react unexpectedly to being hidden
- Fullscreen applications may not hide properly

### Workspace Switching Flow

1. User presses Win + N (workspace number)
2. Hotkey handler calls `switch_workspace_with_windows`
3. Hide all windows in current workspace
4. Show all windows in target workspace
5. Update global active workspace state
6. Update all monitors' active workspace

### Error Handling

Window hiding/showing errors are logged but don't prevent workspace switching. Some windows (like system windows) may fail to hide/show and are simply skipped.

## Next Steps

Proceed to [STEP_6.md](STEP_6.md) to implement the Dwindle tiling algorithm.
