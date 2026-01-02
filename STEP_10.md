# STEP 9: Workspace Switching (Win + Numbers)

## Objective

Implement complete workspace switching functionality with proper window hiding/showing. This step integrates the hiding/showing logic from STEP 5 with the workspace management from STEP 4.

## Tasks

### 9.1 Review and Integrate

The workspace switching logic was already implemented in STEP 5. This step ensures it's fully integrated and working correctly.

**Key components already in place**:
- `switch_workspace_with_windows()` in `src/workspace_manager.rs`
- Hiding/showing functions in `src/windows.rs`
- Hotkey handlers in `src/main.rs`

### 9.2 Enhance Workspace Switching

Update `src/workspace_manager.rs` to improve the experience:

```rust
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

        // Tile and position windows in new workspace
        drop(monitors);
        let mut wm = self.monitors.lock().unwrap();
        wm.tile_active_workspaces();
        wm.apply_window_positions();

        Ok(())
    }

    pub fn get_workspace_window_count(&self, workspace_num: u8) -> usize {
        if workspace_num < 1 || workspace_num > 9 {
            return 0;
        }

        let monitors = self.monitors.lock().unwrap();
        let mut count = 0;

        for monitor in monitors.iter() {
            if let Some(workspace) = monitor.get_workspace(workspace_num) {
                count += workspace.window_count();
            }
        }

        count
    }

    pub fn get_active_workspace_window_count(&self) -> usize {
        self.get_workspace_window_count(self.active_workspace_global)
    }
}
```

### 9.3 Update Hotkey Handler

Ensure the hotkey handler is properly set up:

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
        // ... other actions ...
    }
}
```

### 9.4 Add Workspace Status Display

Create a simple status display function:

```rust
// In src/workspace_manager.rs
impl WorkspaceManager {
    pub fn print_workspace_status(&self) {
        println!("\n=== Workspace Status ===");
        println!("Active workspace: {}", self.active_workspace_global);

        let monitors = self.monitors.lock().unwrap();
        for (i, monitor) in monitors.iter().enumerate() {
            println!("\nMonitor {}:", i);
            for w_idx in 1..=9 {
                if let Some(workspace) = monitor.get_workspace(w_idx) {
                    let count = workspace.window_count();
                    if count > 0 {
                        let active = if w_idx == monitor.active_workspace {
                            " [ACTIVE]"
                        } else {
                            ""
                        };
                        println!("  Workspace {}{}: {} windows", w_idx, active, count);
                    }
                }
            }
        }
        println!("========================\n");
    }
}
```

Add hotkey to print status (for debugging):

```rust
// Add to hotkeys.rs
pub enum HotkeyAction {
    // ... existing actions ...
    PrintStatus, // For debugging
}

// Register Win + S for status
(MOD_WIN, 0x53, 31, HotkeyAction::PrintStatus), // S
```

```rust
// In main.rs hotkey handler
hotkeys::HotkeyAction::PrintStatus => {
    let wm = workspace_manager.lock().unwrap();
    wm.print_workspace_status();
}
```

## Testing

1. Run the application
2. Open multiple applications (6-10 windows)
3. Press Win + 2 to switch to workspace 2
4. Verify:
   - All windows disappear from taskbar
   - Workspace is now empty
5. Press Win + 1 to return to workspace 1
6. Verify:
   - All windows reappear in taskbar
   - Windows are tiled correctly
7. Test switching between various workspaces (1-9)
8. Press Win + S to see workspace status
9. Open windows in different workspaces (once MoveToWorkspace is implemented)

## Success Criteria

- [ ] Workspace switching hides windows correctly
- [ ] Workspace switching shows windows correctly
- [ ] Windows are re-tiled when switching to workspace
- [ ] Taskbar reflects current workspace windows
- [ ] Console logs workspace changes
- [ ] Win + S shows workspace status

## Documentation

### Workspace Switching Flow

1. **User action**: Press Win + N (workspace number)
2. **Hotkey handler**: Calls `switch_workspace_with_windows(new_workspace)`
3. **Hide current workspace**:
   - Iterate through all monitors
   - For each monitor, get current workspace windows
   - Hide each window from taskbar (`hide_window_from_taskbar`)
4. **Show new workspace**:
   - Iterate through all monitors
   - For each monitor, get target workspace windows
   - Show each window in taskbar (`show_window_in_taskbar`)
5. **Update state**:
   - Set `active_workspace_global` to new workspace
   - Update all monitors' `active_workspace`
6. **Re-tile**:
   - Call `tile_active_workspaces()` to recalculate layout
   - Call `apply_window_positions()` to position windows

### Window Hiding Mechanics

**Hiding**:
- `ShowWindow(hwnd, SW_HIDE)` - Makes window invisible
- Remove `WS_EX_APPWINDOW` style - Removes from taskbar
- Store original placement for restoration

**Showing**:
- Restore `WS_EX_APPWINDOW` style - Shows in taskbar
- `ShowWindow(hwnd, SW_SHOW)` - Makes window visible
- Restore original placement and state

### Workspace Isolation

Each workspace maintains independent window lists:
- Windows in inactive workspaces are completely hidden
- No visibility in taskbar
- No keyboard focus
- Effectively "don't exist" from user perspective
- Switching back restores them completely

### Multi-Monitor Workspace

All monitors share the same workspace number:
- Switching to workspace 3 shows workspace 3 on ALL monitors
- Each monitor maintains independent window lists for each workspace
- Workspace 3 on Monitor 1 ≠ Workspace 3 on Monitor 2
- They share the same number but have different content

## Known Issues

1. **Application state**: Some applications may react poorly to being hidden (video players, games)
2. **Fullscreen apps**: Fullscreen applications may not hide properly
3. **Taskbar focus**: Taskbar may briefly show all windows during switch
4. **Performance**: Hiding/showing many windows may cause brief lag

## Next Steps

Proceed to [STEP_10.md](STEP_10.md) to implement moving windows between workspaces (Win + Shift + Numbers).
