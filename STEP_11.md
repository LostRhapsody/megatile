# STEP 10: Move Windows to Workspaces (Win + Shift + Numbers)

## Objective

Implement functionality to move the focused window to a different workspace using Win + Shift + Number keys.

## Tasks

### 10.1 Enhance Window Movement to Workspace

Update `src/workspace_manager.rs`:

```rust
impl WorkspaceManager {
    // ... existing methods ...

    pub fn move_window_to_workspace(&mut self, new_workspace: u8) -> Result<(), String> {
        if new_workspace < 1 || new_workspace > 9 {
            return Err("Invalid workspace number".to_string());
        }

        // Get currently focused window
        let focused = self.get_focused_window();

        if focused.is_none() {
            return Err("No focused window".to_string());
        }

        let focused_window = focused.unwrap();
        let hwnd = focused_window.hwnd;

        let old_workspace = focused_window.workspace;
        let active_workspace = self.active_workspace_global;

        if old_workspace == new_workspace {
            return Ok(()); // Already in target workspace
        }

        println!("Moving window {:?} from workspace {} to workspace {}",
                 hwnd, old_workspace, new_workspace);

        // Remove window from current workspace
        let mut window_to_move = None;
        let mut source_monitor_idx = 0;

        let mut monitors = self.monitors.lock().unwrap();
        for (m_idx, monitor) in monitors.iter_mut().enumerate() {
            if let Some(workspace) = monitor.get_workspace_mut(old_workspace) {
                if let Some(window) = workspace.remove_window(hwnd) {
                    window_to_move = Some(window);
                    source_monitor_idx = m_idx;
                    break;
                }
            }
        }

        if let Some(mut window) = window_to_move {
            // Update window's workspace
            let old_ws = window.workspace;
            window.workspace = new_workspace;

            // Keep window on same monitor (find target workspace on same monitor)
            if let Some(monitor) = monitors.get_mut(source_monitor_idx) {
                if let Some(workspace) = monitor.get_workspace_mut(new_workspace) {
                    workspace.add_window(window.clone());
                }
            }

            // Handle visibility
            if old_ws == active_workspace {
                // Moving from active workspace: hide the window
                crate::windows::hide_window_from_taskbar(hwnd).ok();
            }

            if new_workspace == active_workspace {
                // Moving to active workspace: show and tile the window
                crate::windows::show_window_in_taskbar(hwnd).ok();

                // Re-tile active workspace
                drop(monitors);
                let mut wm = self.monitors.lock().unwrap();
                wm.tile_active_workspaces();
                wm.apply_window_positions();
            }

            println!("Successfully moved window to workspace {}", new_workspace);
            Ok(())
        } else {
            Err("Window not found".to_string())
        }
    }
}
```

### 10.2 Update Hotkey Handler

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
                    // Print updated status
                    wm.print_workspace_status();
                }
                Err(e) => eprintln!("Failed to move window: {}", e),
            }
        }
        // ... other actions ...
    }
}
```

### 10.3 Add Follow Window Option (Optional Enhancement)

Add ability to follow window to target workspace:

```rust
// In src/workspace_manager.rs
impl WorkspaceManager {
    pub fn move_window_to_workspace_follow(&mut self, new_workspace: u8, follow: bool)
        -> Result<(), String>
    {
        // First move the window
        self.move_window_to_workspace(new_workspace)?;

        // If follow is true, switch to the target workspace
        if follow {
            self.switch_workspace_with_windows(new_workspace)?;
        }

        Ok(())
    }
}
```

Update hotkey to follow (Win + Ctrl + Shift + Number):

```rust
// In src/hotkeys.rs, add new hotkeys for follow
(MOD_WIN | MOD_SHIFT | MOD_CONTROL, VK_1, 40, HotkeyAction::MoveToWorkspaceFollow(1)),
// ... and so on for 2-9
```

## Testing

1. Run the application
2. Open multiple applications (5-6 windows)
3. Focus a window
4. Press Win + Shift + 2 to move it to workspace 2
5. Verify:
   - Window disappears from current workspace
   - Window is no longer in taskbar
6. Press Win + 2 to switch to workspace 2
7. Verify:
   - Window appears in workspace 2
   - Window is tiled correctly
8. Move another window to workspace 3
9. Switch between workspaces 1, 2, 3
10. Verify each workspace has the correct windows

## Success Criteria

- [ ] Win + Shift + Number moves focused window to target workspace
- [ ] Window is hidden when moving away from active workspace
- [ ] Window is shown when moving to active workspace
- [ ] Window maintains correct monitor assignment
- [ ] Workspace window counts are updated correctly
- [ ] Tiling is recalculated for affected workspaces

## Documentation

### Move to Workspace Flow

1. **User action**: Press Win + Shift + N (workspace number)
2. **Get focused window**: Find currently focused window
3. **Remove from current workspace**:
   - Locate window in its workspace
   - Remove it from workspace's window list
4. **Add to target workspace**:
   - Update window's workspace number
   - Add to target workspace on same monitor
5. **Handle visibility**:
   - If moving FROM active workspace: hide window
   - If moving TO active workspace: show and tile window
6. **Re-tile if needed**:
   - If moving to/from active workspace, re-tile active workspace

### Monitor Assignment

Windows stay on the same monitor when moved between workspaces:
- Window on Monitor 1, Workspace 1 → Monitor 1, Workspace 2
- Per-monitor workspace lists are maintained independently
- Each monitor has 9 workspace slots

### Visibility Handling

**Moving FROM active workspace**:
- Window is immediately hidden
- Removed from taskbar
- No longer visible or focusable

**Moving TO active workspace**:
- Window is shown
- Appears in taskbar
- Tiled with other active workspace windows

**Moving between inactive workspaces**:
- No visibility change
- Just updates internal tracking
- Window remains hidden

### Use Cases

1. **Organize windows**: Group related apps in different workspaces
2. **Reduce clutter**: Move background apps to separate workspace
3. **Context switching**: Have different setups for different tasks
4. **Multi-project**: Workspace per project

### Limitations

- Cannot move multiple windows at once
- Window must be focused to move it
- Moving window may briefly cause layout flicker
- Some applications may not handle workspace changes gracefully

## Next Steps

Proceed to [STEP_11.md](STEP_11.md) to implement window closing (Win + W).
