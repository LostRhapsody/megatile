# STEP 7: Focus Movement (Win + Arrows)

## Objective

Implement focus movement functionality using Win + Arrow keys to navigate between tiled windows.

## Tasks

### 7.1 Add Focus Management

Update `src/workspace_manager.rs`:

```rust
impl WorkspaceManager {
    // ... existing methods ...

    pub fn get_focused_window(&self) -> Option<Window> {
        use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

        unsafe {
            let hwnd = GetForegroundWindow();
            self.get_window(hwnd.0)
        }
    }

    pub fn move_focus(&self, direction: FocusDirection) -> Result<(), String> {
        use windows::Win32::UI::WindowsAndMessaging::*;

        let focused = self.get_focused_window();

        // Find all windows in active workspace on all monitors
        let mut active_windows: Vec<(Window, RECT)> = Vec::new();
        let monitors = self.monitors.lock().unwrap();

        for monitor in monitors.iter() {
            let active_workspace = monitor.get_active_workspace();
            for window in &active_workspace.windows {
                active_windows.push((window.clone(), window.rect));
            }
        }

        if active_windows.is_empty() {
            return Ok(()); // No windows to focus
        }

        let target = if let Some(focused) = focused {
            // Find window to move focus to based on direction
            self.find_next_focus(&focused, direction, &active_windows)
        } else {
            // No window focused, focus the first window
            active_windows.first().map(|(w, _)| w.clone())
        };

        if let Some(target_window) = target {
            self.set_window_focus(target_window.hwnd);
        }

        Ok(())
    }

    fn find_next_focus(
        &self,
        focused: &Window,
        direction: FocusDirection,
        windows: &[(Window, RECT)],
    ) -> Option<Window> {
        let focused_rect = focused.rect;
        let focused_center = (
            (focused_rect.left + focused_rect.right) / 2,
            (focused_rect.top + focused_rect.bottom) / 2,
        );

        let candidates: Vec<&(Window, RECT)> = windows
            .iter()
            .filter(|(w, _)| w.hwnd != focused.hwnd)
            .collect();

        if candidates.is_empty() {
            return None;
        }

        // Find the best candidate based on direction
        match direction {
            FocusDirection::Left => {
                candidates
                    .iter()
                    .filter(|(_, rect)| rect.right < focused_rect.left)
                    .min_by_key(|(_, rect)| {
                        focused_rect.left - rect.right
                    })
                    .map(|(w, _)| w.clone())
            }
            FocusDirection::Right => {
                candidates
                    .iter()
                    .filter(|(_, rect)| rect.left > focused_rect.right)
                    .min_by_key(|(_, rect)| {
                        rect.left - focused_rect.right
                    })
                    .map(|(w, _)| w.clone())
            }
            FocusDirection::Up => {
                candidates
                    .iter()
                    .filter(|(_, rect)| rect.bottom < focused_rect.top)
                    .min_by_key(|(_, rect)| {
                        focused_rect.top - rect.bottom
                    })
                    .map(|(w, _)| w.clone())
            }
            FocusDirection::Down => {
                candidates
                    .iter()
                    .filter(|(_, rect)| rect.top > focused_rect.bottom)
                    .min_by_key(|(_, rect)| {
                        rect.top - focused_rect.bottom
                    })
                    .map(|(w, _)| w.clone())
            }
        }
    }

    fn set_window_focus(&self, hwnd: isize) {
        use windows::Win32::UI::WindowsAndMessaging::*;

        unsafe {
            SetForegroundWindow(HWND(hwnd));
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum FocusDirection {
    Left,
    Right,
    Up,
    Down,
}
```

### 7.2 Update Hotkey Handler

Update `src/main.rs`:

```rust
fn handle_hotkey(action: hotkeys::HotkeyAction, workspace_manager: &Arc<Mutex<WorkspaceManager>>) {
    match action {
        hotkeys::HotkeyAction::SwitchWorkspace(num) => {
            let mut wm = workspace_manager.lock().unwrap();
            match wm.switch_workspace_with_windows(num) {
                Ok(()) => {
                    println!("Switched to workspace {}", num);
                    wm.tile_active_workspaces();
                    wm.apply_window_positions();
                }
                Err(e) => eprintln!("Failed to switch workspace: {}", e),
            }
        }
        hotkeys::HotkeyAction::FocusLeft => {
            let wm = workspace_manager.lock().unwrap();
            if let Err(e) = wm.move_focus(workspace_manager::FocusDirection::Left) {
                eprintln!("Failed to move focus: {}", e);
            }
        }
        hotkeys::HotkeyAction::FocusRight => {
            let wm = workspace_manager.lock().unwrap();
            if let Err(e) = wm.move_focus(workspace_manager::FocusDirection::Right) {
                eprintln!("Failed to move focus: {}", e);
            }
        }
        hotkeys::HotkeyAction::FocusUp => {
            let wm = workspace_manager.lock().unwrap();
            if let Err(e) = wm.move_focus(workspace_manager::FocusDirection::Up) {
                eprintln!("Failed to move focus: {}", e);
            }
        }
        hotkeys::HotkeyAction::FocusDown => {
            let wm = workspace_manager.lock().unwrap();
            if let Err(e) = wm.move_focus(workspace_manager::FocusDirection::Down) {
                eprintln!("Failed to move focus: {}", e);
            }
        }
        hotkeys::HotkeyAction::MoveToWorkspace(num) => {
            println!("Move to workspace {} (not yet implemented)", num);
        }
        hotkeys::HotkeyAction::MoveLeft |
        hotkeys::HotkeyAction::MoveRight |
        hotkeys::HotkeyAction::MoveUp |
        hotkeys::HotkeyAction::MoveDown => {
            println!("Window movement (not yet implemented)");
        }
        hotkeys::HotkeyAction::CloseWindow => {
            println!("Close window (not yet implemented)");
        }
        hotkeys::HotkeyAction::ToggleTiling => {
            println!("Toggle tiling (not yet implemented)");
        }
        hotkeys::HotkeyAction::ToggleFullscreen => {
            println!("Toggle fullscreen (not yet implemented)");
        }
    }
}
```

### 7.3 Update Focus State

Update window enumeration to track focus state:

```rust
// In src/main.rs, when enumerating windows
use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

let focused_hwnd = unsafe { GetForegroundWindow() };

for window_info in normal_windows {
    let is_focused = window_info.hwnd == focused_hwnd;
    let mut window = workspace::Window::new(
        window_info.hwnd,
        1,
        0,
        window_info.rect,
    );
    window.is_focused = is_focused;
    wm.add_window(window);
}
```

## Testing

1. Run the application
2. Open multiple applications
3. Press Win + Right Arrow to move focus to the next window
4. Press Win + Left Arrow to move focus back
5. Press Win + Up/Down Arrow to move focus vertically
6. Verify that focus moves between windows correctly

## Success Criteria

- [ ] Win + Arrow keys move focus between windows
- [ ] Focus moves in the correct direction (left/right/up/down)
- [ ] Focus wraps around at edges (or stops - test behavior)
- [ ] Console logs focus changes
- [ ] Focus state is tracked correctly

## Documentation

### Focus Movement Algorithm

**Direction-based selection**:
1. Get currently focused window
2. Filter candidate windows based on direction
3. For Left: windows to the left (rect.right < focused.rect.left)
4. For Right: windows to the right (rect.left > focused.rect.right)
5. For Up: windows above (rect.bottom < focused.rect.top)
6. For Down: windows below (rect.top > focused.rect.bottom)

**Distance calculation**:
- Left/Right: Horizontal distance between edges
- Up/Down: Vertical distance between edges
- Select the window with minimum distance

**Edge cases**:
- No window focused: Focus first window
- No candidate in direction: No focus change
- Multiple candidates: Select closest by distance

### Focus Management

**Getting focused window**:
- Uses `GetForegroundWindow()` to get current focus
- Matches HWND to tracked windows

**Setting focus**:
- Uses `SetForegroundWindow()` to set focus
- Automatically brings window to front

### Limitations

- Focus movement is based on window positions, not tiling order
- Diagonal movement requires two arrow key presses
- May not work well with overlapping windows (not expected in tiling)

## Next Steps

Proceed to [STEP_8.md](STEP_8.md) to implement window movement with Win + Shift + Arrow keys.
