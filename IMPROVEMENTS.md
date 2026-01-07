# Megatile Performance Improvements Plan

This document outlines major performance issues identified in the codebase and the recommended fixes.

## Summary

| Priority | Issue | Location | Frequency | Est. Impact |
|----------|-------|----------|-----------|-------------|
| CRITICAL | `update_decorations()` excessive API calls | `workspace_manager.rs:133-181` | Every 100ms | 20-100ms |
| CRITICAL | `cleanup_invalid_windows()` full scan | `workspace_manager.rs:1543-1572` | Every 100ms | 10-50ms |
| HIGH | Monitor enumeration polling | `workspace_manager.rs:436-454` | Every 100ms | 5-15ms |
| MEDIUM | O(n*m*w) triple iteration in `apply_window_positions()` | `workspace_manager.rs:969-1003` | On tiling | 5-20ms |
| MEDIUM | Monitor clone in tiling | `workspace_manager.rs:780,959` | On tiling | 5-15ms |
| MEDIUM | Redundant HashSet creation | `workspace_manager.rs:147-148` | Every 100ms | 2-5ms |
| LOW | Repeated `get_process_name_for_window()` calls | `windows_lib.rs:202-224` | On window create | 2-10ms |
| LOW | Duplicate `IsWindowVisible` check | `windows_lib.rs:419,425` | Per validation | Minimal |

---

## Critical Fixes

### 1. Limit `update_decorations()` to Active Workspace Only

**Current Behavior:**
- Called every 100ms AND on every focus change
- Iterates ALL managed windows across ALL 9 workspaces per monitor
- Makes 2+ DWM API calls per window (`set_window_border_color`, `set_window_transparency`)
- With 20 windows, this is 40+ DWM API calls every 100ms

**Problem Code (`workspace_manager.rs:147-177`):**
```rust
let managed_hwnds = self.get_all_managed_hwnds();  // ALL windows
for hwnd_val in &managed_hwnds {
    set_window_border_color(hwnd, accent_color);
    set_window_transparency(hwnd, desired_alpha);
}
```

**Fix:**
1. Only update decorations for windows in the ACTIVE workspace on each monitor
2. Cache focus state and skip entirely if nothing changed
3. Remove the 100ms timer call - only trigger on actual focus change events

**Implementation:**
```rust
fn update_decorations(&mut self) {
    let foreground = get_foreground_window();
    
    // Skip if focus unchanged
    if self.last_focused_hwnd == Some(foreground) {
        return;
    }
    self.last_focused_hwnd = Some(foreground);
    
    // Only update active workspace windows
    for monitor in &self.monitors {
        let active_ws = &monitor.workspaces[monitor.active_workspace_index];
        for window in &active_ws.windows {
            // Update decorations...
        }
    }
}
```

**Expected Savings:** 20-100ms per cycle

---

### 2. Limit `cleanup_invalid_windows()` to Active Workspace

**Current Behavior:**
- Runs every 100ms
- Calls `is_window_still_valid()` for EVERY window in EVERY workspace
- Each validation makes 5+ Windows API calls

**Problem Code (`workspace_manager.rs:1547-1566`):**
```rust
for monitor in self.monitors.iter() {
    for workspace in &monitor.workspaces {          // ALL 9 workspaces
        for window in &workspace.windows {          // ALL windows
            if !is_window_still_valid(hwnd, ...) {  // 5+ API calls
                invalid_windows.push(hwnd);
            }
        }
    }
}
```

**Fix:**
1. Only validate windows in ACTIVE workspaces
2. Increase interval to 500ms or 1 second
3. Use lazy validation triggered by events when possible

**Implementation:**
```rust
fn cleanup_invalid_windows(&mut self) {
    let mut invalid_windows = Vec::new();
    
    // Only check active workspace per monitor
    for monitor in self.monitors.iter() {
        let active_ws = &monitor.workspaces[monitor.active_workspace_index];
        for window in &active_ws.windows {
            if !is_window_still_valid(window.hwnd, ...) {
                invalid_windows.push(window.hwnd);
            }
        }
    }
    
    // Handle invalid windows...
}
```

**Expected Savings:** 10-50ms per cycle

---

### 3. Fix Duplicate `IsWindowVisible` Call

**Current Behavior (`windows_lib.rs:419-427`):**
```rust
// Line 419
if !IsWindowVisible(hwnd).as_bool() {
    return false;
}

// Line 425-427 - DUPLICATE!
if !IsWindowVisible(hwnd).as_bool() {
    return false;
}
```

**Fix:** Remove the duplicate call.

**Expected Savings:** Minimal per call, but adds up

---

## High Priority Fixes

### 4. Event-Driven Monitor Checks

**Current Behavior:**
- `check_monitor_changes()` called every 100ms
- Calls `EnumDisplayMonitors()` + `GetMonitorInfoW()` for each monitor
- Monitors rarely change during normal operation

**Fix:**
1. Only check monitors after receiving `WM_DISPLAYCHANGE` event
2. Set a dirty flag when display change event is received
3. Check on next cycle only if flag is set

**Implementation:**
```rust
// In event handling
Event::DisplayChange => {
    self.monitors_dirty = true;
}

// In main loop (instead of unconditional check)
if self.monitors_dirty {
    self.check_monitor_changes();
    self.monitors_dirty = false;
}
```

**Expected Savings:** 5-15ms per 100ms cycle

---

## Medium Priority Fixes

### 5. Eliminate O(n*m*w) Loop in `apply_window_positions()`

**Current Behavior (`workspace_manager.rs:969-1003`):**
```rust
// First pass: collect windows
for monitor in self.monitors.iter() {
    for window in &active_workspace.windows {
        windows_to_position.push((window.hwnd, window.rect));
    }
}

// Second pass: O(n * 9 * w) nested loops to update rects
for (hwnd_val, target_rect) in &windows_to_position {
    for monitor in self.monitors.iter_mut() {
        for workspace in &mut monitor.workspaces {
            if let Some(window) = workspace.get_window_mut(...) {
                window.rect = *target_rect;
            }
        }
    }
}

// Third pass: actually position
for (hwnd, rect) in windows_to_position {
    self.set_window_position(...);
}
```

**Fix Options:**
1. Add a `HashMap<isize, (usize, usize, usize)>` mapping hwnd -> (monitor_idx, workspace_idx, window_idx) for O(1) lookup
2. Or combine passes and update rect directly when collecting

**Implementation (Option 1 - HashMap):**
```rust
// Add to WorkspaceManager struct
window_locations: HashMap<isize, (usize, usize, usize)>,

// O(1) lookup instead of nested iteration
fn get_window_location(&self, hwnd: isize) -> Option<(usize, usize, usize)> {
    self.window_locations.get(&hwnd).copied()
}
```

**Expected Savings:** 5-20ms with many windows

---

### 6. Avoid Cloning Monitor in Tiling Operations

**Current Behavior (`workspace_manager.rs:780, 959`):**
```rust
let monitor_copy = monitor.clone();  // Clones 9 workspaces + all windows + tile trees
```

**Fix:**
Refactor `tile_windows()` to only take the data it actually needs (the monitor rect), not the entire Monitor struct.

**Implementation:**
```rust
// Change from:
fn tile_windows(monitor: &Monitor, ...) -> Option<Tile>

// To:
fn tile_windows(monitor_rect: Rect, windows: &[Window], ...) -> Option<Tile>
```

**Expected Savings:** 5-15ms per tiling operation

---

### 7. Return HashSet Directly from `get_all_managed_hwnds()`

**Current Behavior (`workspace_manager.rs:147-148`):**
```rust
let managed_hwnds = self.get_all_managed_hwnds();  // Returns Vec, O(n)
let managed_set: HashSet<isize> = managed_hwnds.iter().copied().collect();  // O(n) again
```

**Fix:**
Change `get_all_managed_hwnds()` to return `HashSet<isize>` directly, or maintain a persistent set.

**Expected Savings:** 2-5ms with many windows

---

## Low Priority Fixes

### 8. Cache Process Names

**Current Behavior:**
`get_process_name_for_window()` is called on every window creation and during enumeration. Each call:
- `GetWindowThreadProcessId()`
- `OpenProcess()`
- `QueryFullProcessImageNameW()`

**Fix:**
Add a cache mapping `hwnd -> process_name` or `process_id -> process_name`.

**Expected Savings:** 2-10ms per new window

---

## Implementation Order

1. **Phase 1 - Critical (Immediate):**
   - [ ] Limit `update_decorations()` to active workspace
   - [ ] Limit `cleanup_invalid_windows()` to active workspace
   - [ ] Remove duplicate `IsWindowVisible` call

2. **Phase 2 - High (Short-term):**
   - [ ] Make monitor checks event-driven

3. **Phase 3 - Medium (When convenient):**
   - [ ] Add HashMap for O(1) window lookup
   - [ ] Refactor `tile_windows()` to avoid Monitor clone
   - [ ] Return HashSet from `get_all_managed_hwnds()`

4. **Phase 4 - Low (Future):**
   - [ ] Cache process names

---

## Testing

After each fix:
1. Run `cargo clippy` and `cargo fmt`
2. Build with `cargo build --release`
3. Test with 10+ windows across multiple workspaces
4. Verify no visible latency when switching workspaces or focusing windows
5. Monitor CPU usage in Task Manager during normal operation
