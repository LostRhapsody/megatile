# Fix Monitor Hot-Plug Handling

## Problem Statement

When monitor count changes (e.g., 3→1 or 1→3), Megatile needs to:
1. Detect the change reliably
2. Restore all windows to visible state first (safety net)
3. Migrate orphaned windows to remaining monitors
4. Rebuild tiling layout for new configuration

Currently, the `reenumerate_monitors()` function loses windows when monitors are disconnected because it only preserves workspace data when `hmonitor` handles match - but disconnected monitors have invalid handles.

## Solution Design

### Phase 1: Pre-Change Safety

Before any monitor reconfiguration, protect against data loss:

1. **Restore ALL managed windows to visible state**
   - Call `show_window_in_taskbar()` on every managed window
   - This ensures if anything crashes during reconfiguration, windows aren't left invisible
   - Similar to existing `cleanup_on_exit()` logic in `main.rs`

2. **Collect complete window inventory**
   - Extract all windows from all monitors/workspaces into a flat list
   - Each window already tracks: `hwnd`, `workspace`, `monitor`, `rect`, `original_rect`, `is_tiled`, etc.

### Phase 2: Monitor Detection & Analysis

1. **Get new monitor configuration**
   - Call `enumerate_monitors()` to get current `Vec<MonitorInfo>`
   
2. **Detect change type** by comparing old vs new:
   - `old_count > new_count`: Monitors removed - need to migrate windows
   - `old_count < new_count`: Monitors added - just rebuild, no migration needed
   - `old_count == new_count`: Arrangement changed - check hmonitor matches

3. **Identify orphaned monitors**
   - Find old monitors whose `hmonitor` doesn't exist in new config
   - These monitors' windows need migration

### Phase 3: Window Migration (Round-Robin)

When monitors are removed:

1. **Collect all orphaned windows** from monitors that no longer exist

2. **Distribute round-robin across remaining monitors**
   - Cycle through available monitors: monitor 0, 1, 2, 0, 1, 2...
   - Assign each orphaned window to next monitor in cycle
   - Preserve workspace number (window stays in same workspace 1-9)
   - Update `window.monitor` index to new target

3. **Handle edge case**: If ALL monitors are removed (shouldn't happen but be safe), use monitor 0

### Phase 4: Rebuild & Re-tile

1. **Create new monitor list** from `enumerate_monitors()` results

2. **Populate monitors with windows**:
   - For monitors that still exist (hmonitor matches): preserve their workspace data
   - Inject migrated windows into their target monitors/workspaces

3. **Reset all layout trees** - Force fresh tiling calculation

4. **Re-tile all active workspaces** via `tile_active_workspaces()`

5. **Apply positions** via `apply_window_positions()`

6. **Re-hide non-active workspace windows** - Restore proper visibility state

7. **Rebuild window_locations cache** for O(1) lookups

8. **Recenter statusbar** on primary monitor

## Implementation Plan

### Step 1: Fix File Corruption

Clean up stray closing braces in `workspace_manager.rs`:
- Lines 70-73: Extra `}` after `new()`
- Lines 249-251: Extra `}` after `rebuild_window_locations()`

### Step 2: Add Helper Functions

Add to `WorkspaceManager`:

```rust
/// Restores all managed windows to visible state.
/// Call before any risky operation to prevent window loss.
fn restore_all_windows_visible(&self) {
    for hwnd in self.get_all_managed_hwnds() {
        let _ = show_window_in_taskbar(hwnd_from_isize(hwnd));
    }
}

/// Collects all windows from all monitors/workspaces into a flat list.
fn collect_all_windows(&self) -> Vec<Window> {
    let mut windows = Vec::new();
    for monitor in &self.monitors {
        for workspace in &monitor.workspaces {
            windows.extend(workspace.windows.iter().cloned());
        }
    }
    windows
}

/// Finds monitors that no longer exist in the new configuration.
/// Returns indices of old monitors that are orphaned.
fn find_orphaned_monitor_indices(&self, new_infos: &[MonitorInfo]) -> Vec<usize> {
    self.monitors
        .iter()
        .enumerate()
        .filter(|(_, m)| !new_infos.iter().any(|info| info.hmonitor == m.hmonitor))
        .map(|(i, _)| i)
        .collect()
}
```

### Step 3: Rewrite `reenumerate_monitors()`

New implementation:

```rust
pub fn reenumerate_monitors(&mut self) -> Result<(), String> {
    // Debounce rapid calls
    if self.last_reenumerate.elapsed() < Duration::from_millis(500) {
        return Ok(());
    }
    self.last_reenumerate = Instant::now();

    info!("Re-enumerating monitors...");

    // PHASE 1: Safety - restore all windows to visible
    self.restore_all_windows_visible();

    // Get new monitor configuration
    let new_infos = crate::windows_lib::enumerate_monitors();
    info!("Found {} monitor(s) (was {})", new_infos.len(), self.monitors.len());

    if new_infos.is_empty() {
        warn!("No monitors detected, skipping reconfiguration");
        return Ok(());
    }

    // PHASE 2: Detect orphaned monitors
    let orphaned_indices = self.find_orphaned_monitor_indices(&new_infos);
    
    // PHASE 3: Collect orphaned windows and determine migration targets
    let mut orphaned_windows: Vec<Window> = Vec::new();
    for &idx in &orphaned_indices {
        if let Some(monitor) = self.monitors.get(idx) {
            for workspace in &monitor.workspaces {
                orphaned_windows.extend(workspace.windows.iter().cloned());
            }
        }
    }
    info!("Found {} orphaned windows to migrate", orphaned_windows.len());

    // PHASE 4: Build new monitor list
    let mut new_monitors: Vec<Monitor> = Vec::new();
    for info in &new_infos {
        // Try to preserve workspace data from matching monitor
        let workspaces = if let Some(old) = self.monitors.iter().find(|m| m.hmonitor == info.hmonitor) {
            old.workspaces.clone()
        } else {
            std::array::from_fn(|_| Workspace::new())
        };

        let mut monitor = Monitor::new(info.hmonitor, info.rect);
        monitor.workspaces = workspaces;
        monitor.active_workspace = self.active_workspace_global;
        new_monitors.push(monitor);
    }

    // PHASE 5: Distribute orphaned windows round-robin
    if !orphaned_windows.is_empty() && !new_monitors.is_empty() {
        for (i, mut window) in orphaned_windows.into_iter().enumerate() {
            let target_monitor_idx = i % new_monitors.len();
            window.monitor = target_monitor_idx;
            // Reset tiling state for fresh layout
            window.is_tiled = true;
            
            if let Some(workspace) = new_monitors[target_monitor_idx]
                .get_workspace_mut(window.workspace)
            {
                workspace.layout_tree = None; // Force layout recalculation
                workspace.windows.push(window);
            }
        }
    }

    // PHASE 6: Finalize
    self.monitors = new_monitors;

    // Clear all layout trees to force fresh calculation
    for monitor in &mut self.monitors {
        for workspace in &mut monitor.workspaces {
            workspace.layout_tree = None;
        }
    }

    // Re-tile and apply positions
    self.tile_active_workspaces();
    self.apply_window_positions();

    // Re-hide windows on non-active workspaces
    self.rehide_inactive_workspace_windows();

    // Rebuild lookup cache
    self.rebuild_window_locations();

    info!("Monitor re-enumeration complete");
    Ok(())
}

/// Re-hides windows that should be hidden due to workspace switching.
fn rehide_inactive_workspace_windows(&self) {
    let active = self.active_workspace_global;
    for monitor in &self.monitors {
        for (ws_idx, workspace) in monitor.workspaces.iter().enumerate() {
            let ws_num = (ws_idx + 1) as u8;
            for window in &workspace.windows {
                let hwnd = hwnd_from_isize(window.hwnd);
                if ws_num == active {
                    let _ = show_window_in_taskbar(hwnd);
                } else {
                    let _ = hide_window_from_taskbar(hwnd);
                }
            }
        }
    }
}
```

### Step 4: Testing

Manual testing scenarios:

1. **3→1 monitors**: Disconnect 2 monitors, verify all windows migrate to remaining monitor
2. **1→3 monitors**: Connect 2 monitors, verify windows stay on original monitor, new monitors ready
3. **Rapid disconnect/reconnect**: Simulate sleep/wake cycle, verify no windows lost
4. **Workspace preservation**: Windows should stay in same workspace number after migration
5. **Tiling reset**: After migration, windows should be freshly tiled on new monitor

## Files to Modify

1. `src/workspace_manager.rs`:
   - Fix file corruption (extra braces)
   - Add `restore_all_windows_visible()`
   - Add `collect_all_windows()` (if needed for debugging)
   - Add `find_orphaned_monitor_indices()`
   - Add `rehide_inactive_workspace_windows()`
   - Rewrite `reenumerate_monitors()`

## Risk Mitigation

- **Safety first**: Always restore windows visible before any changes
- **Graceful degradation**: If no monitors detected, skip reconfiguration
- **Preserve workspace numbers**: Users' mental model of workspace organization is maintained
- **Round-robin distribution**: Evenly spreads load across remaining monitors
- **Layout reset**: Fresh tiling avoids weird sizing issues from old monitor dimensions
