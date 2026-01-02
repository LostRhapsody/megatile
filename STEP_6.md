# STEP 6: Basic Dwindle Tiling Algorithm

## Objective

Implement the Dwindle tiling algorithm (binary space partitioning) to automatically position and resize windows within the active workspace on each monitor.

## Tasks

### 6.1 Create Tiling Module

Create `src/tiling.rs`:

```rust
use crate::workspace::{Monitor, Window};
use windows::Win32::Graphics::Gdi::RECT;

#[derive(Debug, Clone, Copy)]
pub enum SplitDirection {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone)]
pub struct Tile {
    pub rect: RECT,
    pub windows: Vec<usize>, // Indices into workspace.windows
    pub split_direction: Option<SplitDirection>,
    pub children: Option<Box<(Tile, Tile)>>,
}

impl Tile {
    pub fn new(rect: RECT) -> Self {
        Tile {
            rect,
            windows: Vec::new(),
            split_direction: None,
            children: None,
        }
    }

    pub fn is_leaf(&self) -> bool {
        self.children.is_none()
    }
}

pub struct DwindleTiler {
    gap: i32,
}

impl DwindleTiler {
    pub fn new(gap: i32) -> Self {
        DwindleTiler { gap }
    }

    pub fn tile_windows(&self, monitor: &Monitor, windows: &mut Vec<Window>) {
        let active_workspace = monitor.get_active_workspace();
        let window_count = active_workspace.window_count();

        if window_count == 0 {
            return;
        }

        // Get monitor work area (usable space)
        let work_rect = self.get_work_area(monitor);

        // Create initial tile covering entire work area
        let mut root_tile = Tile::new(work_rect);

        // Distribute windows across tiles using Dwindle algorithm
        self.distribute_windows(&mut root_tile, windows);

        // Apply tile positions to windows
        self.apply_tile_positions(&root_tile, windows);
    }

    fn get_work_area(&self, monitor: &Monitor) -> RECT {
        // For now, use full monitor rect
        // TODO: Consider taskbar and other reserved areas
        let mut rect = monitor.rect;
        // Add gap padding
        rect.left += self.gap;
        rect.top += self.gap;
        rect.right -= self.gap;
        rect.bottom -= self.gap;
        rect
    }

    fn distribute_windows(&self, tile: &mut Tile, windows: &mut Vec<Window>) {
        let active_windows: Vec<&mut Window> = windows
            .iter_mut()
            .filter(|w| w.workspace > 0)
            .collect();

        if active_windows.is_empty() {
            return;
        }

        // Assign all windows to root tile initially
        let window_indices: Vec<usize> = (0..windows.len())
            .filter(|&i| windows[i].workspace > 0)
            .collect();

        tile.windows.extend(window_indices);

        // Recursively split tiles
        self.split_tile(tile, active_windows.len());
    }

    fn split_tile(&self, tile: &mut Tile, window_count: usize) {
        if window_count <= 1 {
            return;
        }

        // Determine split direction
        let tile_width = tile.rect.right - tile.rect.left;
        let tile_height = tile.rect.bottom - tile.rect.top;

        let split_direction = if tile_width > tile_height {
            SplitDirection::Vertical
        } else {
            SplitDirection::Horizontal
        };

        tile.split_direction = Some(split_direction);

        // Split windows between children
        let split_point = window_count / 2;
        let left_windows = tile.windows[..split_point].to_vec();
        let right_windows = tile.windows[split_point..].to_vec();

        // Create child tiles
        let (left_rect, right_rect) = self.split_rect(&tile.rect, split_direction);

        let mut left_tile = Tile::new(left_rect);
        left_tile.windows = left_windows;

        let mut right_tile = Tile::new(right_rect);
        right_tile.windows = right_windows;

        // Recursively split children
        if left_tile.windows.len() > 0 {
            self.split_tile(&mut left_tile, left_tile.windows.len());
        }
        if right_tile.windows.len() > 0 {
            self.split_tile(&mut right_tile, right_tile.windows.len());
        }

        tile.children = Some(Box::new((left_tile, right_tile)));
    }

    fn split_rect(&self, rect: &RECT, direction: SplitDirection) -> (RECT, RECT) {
        let gap = self.gap;
        let mid_gap = gap / 2;

        match direction {
            SplitDirection::Horizontal => {
                let height = rect.bottom - rect.top;
                let split = rect.top + height / 2 - mid_gap;

                let mut left = *rect;
                left.bottom = split;

                let mut right = *rect;
                right.top = split + mid_gap;

                (left, right)
            }
            SplitDirection::Vertical => {
                let width = rect.right - rect.left;
                let split = rect.left + width / 2 - mid_gap;

                let mut left = *rect;
                left.right = split;

                let mut right = *rect;
                right.left = split + mid_gap;

                (left, right)
            }
        }
    }

    fn apply_tile_positions(&self, tile: &Tile, windows: &mut Vec<Window>) {
        if tile.is_leaf() {
            // Apply tile rect to all windows in this tile
            for &window_idx in &tile.windows {
                if window_idx < windows.len() {
                    windows[window_idx].rect = tile.rect;
                }
            }
        } else {
            if let Some(ref children) = tile.children {
                self.apply_tile_positions(&children.0, windows);
                self.apply_tile_positions(&children.1, windows);
            }
        }
    }
}

impl Default for DwindleTiler {
    fn default() -> Self {
        Self::new(8) // Default 8px gap
    }
}
```

### 6.2 Update Workspace Manager

Update `src/workspace_manager.rs` to integrate tiling:

```rust
use crate::tiling::DwindleTiler;

impl WorkspaceManager {
    // ... existing methods ...

    pub fn tile_active_workspaces(&self) {
        let tiler = DwindleTiler::default();
        let mut monitors = self.monitors.lock().unwrap();

        for monitor in monitors.iter_mut() {
            let workspace_idx = (monitor.active_workspace - 1) as usize;
            let windows = &mut monitor.workspaces[workspace_idx].windows;

            if !windows.is_empty() {
                tiler.tile_windows(monitor, windows);
            }
        }
    }

    pub fn apply_window_positions(&self) {
        let monitors = self.monitors.lock().unwrap();

        for monitor in monitors.iter() {
            let active_workspace = monitor.get_active_workspace();

            for window in &active_workspace.windows {
                self.set_window_position(window.hwnd, &window.rect);
            }
        }
    }

    fn set_window_position(&self, hwnd: isize, rect: &RECT) {
        use windows::Win32::UI::WindowsAndMessaging::*;

        unsafe {
            SetWindowPos(
                HWND(hwnd),
                HWND::default(),
                rect.left,
                rect.top,
                rect.right - rect.left,
                rect.bottom - rect.top,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }
}
```

### 6.3 Update Main Entry Point

Update `src/main.rs` to apply tiling:

```rust
fn handle_hotkey(action: hotkeys::HotkeyAction, workspace_manager: &Arc<Mutex<WorkspaceManager>>) {
    match action {
        hotkeys::HotkeyAction::SwitchWorkspace(num) => {
            let mut wm = workspace_manager.lock().unwrap();
            match wm.switch_workspace_with_windows(num) {
                Ok(()) => {
                    println!("Switched to workspace {}", num);
                    // Tile and apply positions for new workspace
                    wm.tile_active_workspaces();
                    wm.apply_window_positions();
                }
                Err(e) => eprintln!("Failed to switch workspace: {}", e),
            }
        }
        hotkeys::HotkeyAction::MoveToWorkspace(num) => {
            println!("Move to workspace {} (not yet implemented)", num);
        }
        _ => {
            println!("Hotkey action: {:?}", action);
        }
    }
}

fn main() {
    // ... existing setup code ...

    // After enumerating windows and assigning to workspace 1
    let mut wm = workspace_manager.lock().unwrap();
    wm.tile_active_workspaces();
    wm.apply_window_positions();
    drop(wm);

    // ... rest of main function ...
}
```

## Testing

1. Run the application
2. Open multiple applications (3-5 windows)
3. Observe that windows are tiled in a grid pattern
4. Press Win + 2 to switch to workspace 2, then Win + 1 to return
5. Verify that windows are re-tiled correctly
6. Open more windows and observe tiling behavior

## Success Criteria

- [ ] Windows are tiled using Dwindle algorithm
- [ ] Windows are resized and positioned correctly
- [ ] Gaps are maintained between windows
- [ ] Tiling updates on workspace switch
- [ ] Multiple windows are distributed evenly

## Documentation

### Dwindle Tiling Algorithm

The Dwindle algorithm uses binary space partitioning:

1. Start with a single tile covering the entire work area
2. Split the tile either horizontally or vertically based on aspect ratio
3. Distribute windows between the two child tiles
4. Recursively split child tiles until each tile has at most one window
5. Apply tile positions to windows

**Split direction**:
- Vertical split for wide rectangles
- Horizontal split for tall rectangles

**Window distribution**:
- Windows are divided evenly between child tiles
- Each recursive split divides the window count by 2

### Tile Structure

```rust
Tile {
    rect: RECT,              // Position and size
    windows: Vec<usize>,     // Window indices
    split_direction: Option<Direction>, // Split if any
    children: Option<(Tile, Tile)>, // Child tiles if split
}
```

### Gap Handling

A configurable gap (default 8px) is added:
- Padding around work area
- Gap between split tiles
- Helps visually separate windows

### Limitations

- Does not consider window minimum sizes
- May not handle very small monitors well
- All windows have equal priority (no master window)
- Does not handle window resizing constraints

## Next Steps

Proceed to [STEP_7.md](STEP_7.md) to implement focus movement with Win + Arrow keys.
