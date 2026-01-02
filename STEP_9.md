# STEP 8: Window Movement (Win + Shift + Arrows)

## Objective

Implement window swapping functionality using Win + Shift + Arrow keys to move windows between tiles.

## Tasks

### 8.1 Add Window Movement Functions

Update `src/workspace_manager.rs`:

```rust
impl WorkspaceManager {
    // ... existing methods ...

    pub fn move_window(&self, direction: FocusDirection) -> Result<(), String> {
        let focused = self.get_focused_window();

        if focused.is_none() {
            return Err("No focused window".to_string());
        }

        let focused = focused.unwrap();

        // Find all windows in active workspace
        let mut active_windows: Vec<Window> = Vec::new();
        let mut monitors = self.monitors.lock().unwrap();

        for monitor in monitors.iter_mut() {
            let active_workspace_idx = (monitor.active_workspace - 1) as usize;
            for window in &mut monitor.workspaces[active_workspace_idx].windows {
                active_windows.push(window.clone());
            }
        }

        // Find target window to swap with
        let target = self.find_swap_target(&focused, direction, &active_windows);

        if let Some(target_window) = target {
            // Swap window positions
            self.swap_window_positions(&mut monitors, focused.hwnd, target_window.hwnd);

            // Re-tile and apply positions
            drop(monitors);
            let mut wm = self.monitors.lock().unwrap();
            wm.tile_active_workspaces();
            wm.apply_window_positions();

            println!("Swapped windows: {:?} <-> {:?}", focused.hwnd, target_window.hwnd);
        }

        Ok(())
    }

    fn find_swap_target(
        &self,
        window: &Window,
        direction: FocusDirection,
        windows: &[Window],
    ) -> Option<Window> {
        // Similar to focus finding but for swapping
        let window_rect = window.rect;

        let candidates: Vec<&Window> = windows
            .iter()
            .filter(|w| w.hwnd != window.hwnd)
            .collect();

        if candidates.is_empty() {
            return None;
        }

        match direction {
            FocusDirection::Left => {
                candidates
                    .iter()
                    .filter(|w| w.rect.right < window_rect.left)
                    .min_by_key(|w| window_rect.left - w.rect.right)
                    .map(|w| (*w).clone())
            }
            FocusDirection::Right => {
                candidates
                    .iter()
                    .filter(|w| w.rect.left > window_rect.right)
                    .min_by_key(|w| w.rect.left - window_rect.right)
                    .map(|w| (*w).clone())
            }
            FocusDirection::Up => {
                candidates
                    .iter()
                    .filter(|w| w.rect.bottom < window_rect.top)
                    .min_by_key(|w| window_rect.top - w.rect.bottom)
                    .map(|w| (*w).clone())
            }
            FocusDirection::Down => {
                candidates
                    .iter()
                    .filter(|w| w.rect.top > window_rect.bottom)
                    .min_by_key(|w| w.rect.top - window_rect.bottom)
                    .map(|w| (*w).clone())
            }
        }
    }

    fn swap_window_positions(&self, monitors: &mut Vec<Monitor>, hwnd1: isize, hwnd2: isize) {
        let mut window1: Option<Window> = None;
        let mut window2: Option<Window> = None;
        let mut workspace1_idx = 0;
        let mut workspace2_idx = 0;
        let mut monitor1_idx = 0;
        let mut monitor2_idx = 0;

        // Find both windows
        for (m_idx, monitor) in monitors.iter_mut().enumerate() {
            for w_idx in 0..9 {
                if let Some(workspace) = monitor.get_workspace_mut((w_idx + 1) as u8) {
                    for window in &workspace.windows {
                        if window.hwnd == hwnd1 && window1.is_none() {
                            window1 = Some(window.clone());
                            workspace1_idx = w_idx;
                            monitor1_idx = m_idx;
                        }
                        if window.hwnd == hwnd2 && window2.is_none() {
                            window2 = Some(window.clone());
                            workspace2_idx = w_idx;
                            monitor2_idx = m_idx;
                        }
                    }
                }
            }
        }

        // Swap rect and monitor between windows
        if let (Some(mut w1), Some(mut w2)) = (window1, window2) {
            let temp_rect = w1.rect;
            w1.rect = w2.rect;
            w2.rect = temp_rect;

            let temp_monitor = w1.monitor;
            w1.monitor = w2.monitor;
            w2.monitor = temp_monitor;

            // Update windows in their respective workspaces
            for (m_idx, monitor) in monitors.iter_mut().enumerate() {
                if m_idx == monitor1_idx {
                    if let Some(workspace) = monitor.get_workspace_mut((workspace1_idx + 1) as u8) {
                        if let Some(window) = workspace.get_window_mut(hwnd1) {
                            window.rect = w1.rect;
                            window.monitor = w1.monitor;
                        }
                    }
                }
                if m_idx == monitor2_idx {
                    if let Some(workspace) = monitor.get_workspace_mut((workspace2_idx + 1) as u8) {
                        if let Some(window) = workspace.get_window_mut(hwnd2) {
                            window.rect = w2.rect;
                            window.monitor = w2.monitor;
                        }
                    }
                }
            }
        }
    }
}
```

### 8.2 Update Hotkey Handler

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
            let _ = wm.move_focus(workspace_manager::FocusDirection::Left);
        }
        hotkeys::HotkeyAction::FocusRight => {
            let wm = workspace_manager.lock().unwrap();
            let _ = wm.move_focus(workspace_manager::FocusDirection::Right);
        }
        hotkeys::HotkeyAction::FocusUp => {
            let wm = workspace_manager.lock().unwrap();
            let _ = wm.move_focus(workspace_manager::FocusDirection::Up);
        }
        hotkeys::HotkeyAction::FocusDown => {
            let wm = workspace_manager.lock().unwrap();
            let _ = wm.move_focus(workspace_manager::FocusDirection::Down);
        }
        hotkeys::HotkeyAction::MoveLeft => {
            let wm = workspace_manager.lock().unwrap();
            let _ = wm.move_window(workspace_manager::FocusDirection::Left);
        }
        hotkeys::HotkeyAction::MoveRight => {
            let wm = workspace_manager.lock().unwrap();
            let _ = wm.move_window(workspace_manager::FocusDirection::Right);
        }
        hotkeys::HotkeyAction::MoveUp => {
            let wm = workspace_manager.lock().unwrap();
            let _ = wm.move_window(workspace_manager::FocusDirection::Up);
        }
        hotkeys::HotkeyAction::MoveDown => {
            let wm = workspace_manager.lock().unwrap();
            let _ = wm.move_window(workspace_manager::FocusDirection::Down);
        }
        hotkeys::HotkeyAction::MoveToWorkspace(num) => {
            println!("Move to workspace {} (not yet implemented)", num);
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

## Testing

1. Run the application
2. Open multiple applications
3. Focus a window in the middle
4. Press Win + Shift + Right Arrow to move it right
5. Observe that windows swap positions
6. Test other directions (Left, Up, Down)
7. Verify that windows stay tiled correctly

## Success Criteria

- [ ] Win + Shift + Arrow keys swap window positions
- [ ] Windows move in the correct direction
- [ ] Tiling is maintained after window swap
- [ ] Window monitor assignments are updated correctly
- [ ] No errors in console output

## Documentation

### Window Swapping Algorithm

**Finding swap target**:
1. Get currently focused window
2. Find candidate windows in specified direction
3. Select the closest window in that direction
4. Swap window positions (rectangles and monitor assignments)

**Position swapping**:
- Swap `rect` between windows
- Swap `monitor` index to maintain correct monitor tracking
- Update window objects in their respective workspaces

**Re-tiling**:
- After swapping, call `tile_active_workspaces()` to recalculate layout
- Apply new positions to all windows
- Ensures consistent tiling state

### Edge Cases

- No window focused: Error message, no action
- No target in direction: No action
- Windows on different monitors: Still swap positions
- Single window: No action possible

### Differences from Focus Movement

- **Focus**: Just changes which window has keyboard focus
- **Move**: Swaps window positions in the tiling layout
- Both use similar direction-based selection logic

## Next Steps

Proceed to [STEP_9.md](STEP_9.md) to implement workspace switching with proper window hiding/showing.
