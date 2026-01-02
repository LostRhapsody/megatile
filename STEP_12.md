# STEP 11: Window Closing (Win + W)

## Objective

Implement window closing functionality to gracefully close the currently focused window.

## Tasks

### 11.1 Add Window Closing Function

Update `src/windows.rs`:

```rust
use windows::Win32::UI::WindowsAndMessaging::*;

pub fn close_window(hwnd: HWND) -> Result<(), String> {
    unsafe {
        // Try to close gracefully by sending WM_CLOSE message
        PostMessageW(hwnd, WM_CLOSE, WPARAM(0), LPARAM(0)).ok();
        Ok(())
    }
}

pub fn force_close_window(hwnd: HWND) -> Result<(), String> {
    unsafe {
        // Force terminate the window
        DestroyWindow(hwnd).ok();
        Ok(())
    }
}
```

### 11.2 Add Window Removal from Workspace

Update `src/workspace_manager.rs`:

```rust
impl WorkspaceManager {
    // ... existing methods ...

    pub fn close_focused_window(&mut self) -> Result<(), String> {
        // Get currently focused window
        let focused = self.get_focused_window();

        if focused.is_none() {
            return Err("No focused window".to_string());
        }

        let focused_window = focused.unwrap();
        let hwnd = focused_window.hwnd;

        println!("Closing window {:?} ('{}')", hwnd, focused_window.title);

        // Remove window from workspace tracking
        let removed = self.remove_window(hwnd);

        if removed.is_none() {
            return Err("Window not found in workspace manager".to_string());
        }

        // Close the actual window
        crate::windows::close_window(hwnd)?;

        // Re-tile active workspace
        let mut wm = self.monitors.lock().unwrap();
        wm.tile_active_workspaces();
        wm.apply_window_positions();

        println!("Window closed and workspace re-tiled");
        Ok(())
    }

    pub fn close_window_by_hwnd(&mut self, hwnd: isize) -> Result<(), String> {
        println!("Closing window {:?} (external close event)", hwnd);

        // Remove window from workspace tracking
        let removed = self.remove_window(hwnd);

        if removed.is_some() {
            // Re-tile affected workspace
            let mut wm = self.monitors.lock().unwrap();
            wm.tile_active_workspaces();
            wm.apply_window_positions();

            println!("Window removed from workspace tracking and workspace re-tiled");
        }

        Ok(())
    }
}
```

### 11.3 Update Hotkey Handler

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
        // ... other actions ...
    }
}
```

### 11.4 Add Window Destruction Event Handling (Future Enhancement)

For automatic window removal when windows are closed externally, we'll need to hook into window events. This is a placeholder for future implementation:

```rust
// In src/windows.rs (future enhancement)
pub fn set_window_event_hook(callback: Box<dyn Fn(isize) + Send>) {
    // TODO: Implement SetWinEventHook for EVENT_OBJECT_DESTROY
    // This will automatically remove windows from tracking when they're closed
    // outside of MegaTile (e.g., user clicks X button)
}
```

### 11.5 Update Window Info

Ensure `Window` struct has title field:

```rust
// In src/workspace.rs
#[derive(Debug, Clone)]
pub struct Window {
    pub hwnd: HWND,
    pub title: String, // Add this if not already present
    pub workspace: u8,
    pub monitor: usize,
    pub rect: RECT,
    pub is_focused: bool,
    pub original_rect: RECT,
}
```

## Testing

1. Run the application
2. Open multiple applications (Notepad, Calculator, etc.)
3. Focus one window
4. Press Win + W to close it
5. Verify:
   - Window closes gracefully
   - Remaining windows are re-tiled
   - Workspace has one less window
6. Close another window
7. Verify tiling updates correctly
8. Test closing last window (should result in empty workspace)

## Success Criteria

- [ ] Win + W closes the focused window
- [ ] Window closes gracefully (sends WM_CLOSE)
- [ ] Window is removed from workspace tracking
- [ ] Remaining windows are re-tiled
- [ ] No errors in console output
- [ ] Works with various application types

## Documentation

### Window Closing Methods

**Graceful close** (`close_window`):
- Sends `WM_CLOSE` message to window
- Allows application to prompt for save, clean up, etc.
- Respects application's close behavior
- Default method for most cases

**Force close** (`force_close_window`):
- Calls `DestroyWindow` API
- Immediately destroys window without application cleanup
- May cause data loss in applications
- Use only when graceful close fails

### Close Window Flow

1. **User action**: Press Win + W
2. **Get focused window**: Find currently focused window
3. **Remove from tracking**:
   - Remove window from workspace's window list
   - Clean up any references
4. **Close actual window**:
   - Send `WM_CLOSE` message to window handle
   - Application handles close (saves data, cleans up)
5. **Re-tile workspace**:
   - Recalculate layout without closed window
   - Apply new positions to remaining windows

### External Window Closes

When windows are closed externally (user clicks X, app crashes, etc.):
- Currently not handled (TODO: implement window event hooks)
- Window remains in workspace tracking until re-enumeration
- Should implement `SetWinEventHook` for `EVENT_OBJECT_DESTROY`

### Window Close Behavior

**Supported**:
- Standard applications (Notepad, Calculator, etc.)
- Applications that handle WM_CLOSE normally
- Multi-window applications (closes focused window only)

**Edge cases**:
- Apps with unsaved changes: Will show save dialog
- Apps that minimize on close: Will be removed from tracking incorrectly
- Fullscreen apps: May not respond to WM_CLOSE
- System dialogs: Not tracked by MegaTile anyway

### Future Enhancements

1. **Window destruction hook**: Auto-remove externally closed windows
2. **Force close option**: Win + Shift + W for unresponsive windows
3. **Close all in workspace**: Win + Ctrl + W
4. **Confirm before close**: Prompt for certain apps

## Next Steps

Proceed to [STEP_12.md](STEP_12.md) to implement tiling algorithm toggle (Win + T).
