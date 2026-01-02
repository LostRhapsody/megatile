# STEP 12: Toggle Tiling Algorithm (Win + T)

## Objective

Implement tiling algorithm toggle functionality to switch between different tiling layouts. This sets up the infrastructure for multiple algorithms.

## Tasks

### 12.1 Define Tiling Algorithms

Update `src/tiling.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TilingAlgorithm {
    Dwindle,
    // Future algorithms can be added here:
    // Spiral,
    // Stack,
    // Column,
}

pub struct Tiler {
    algorithm: TilingAlgorithm,
    gap: i32,
}

impl Tiler {
    pub fn new(algorithm: TilingAlgorithm, gap: i32) -> Self {
        Tiler { algorithm, gap }
    }

    pub fn with_algorithm(algorithm: TilingAlgorithm) -> Self {
        Self::new(algorithm, 8)
    }

    pub fn set_algorithm(&mut self, algorithm: TilingAlgorithm) {
        self.algorithm = algorithm;
    }

    pub fn get_algorithm(&self) -> TilingAlgorithm {
        self.algorithm
    }

    pub fn toggle_algorithm(&mut self) -> TilingAlgorithm {
        self.algorithm = match self.algorithm {
            TilingAlgorithm::Dwindle => TilingAlgorithm::Dwindle, // Only one algorithm for now
            // _ => TilingAlgorithm::Dwindle, // When more algorithms exist
        };
        self.algorithm
    }

    pub fn tile_windows(&self, monitor: &Monitor, windows: &mut Vec<Window>) {
        match self.algorithm {
            TilingAlgorithm::Dwindle => {
                let dwindle = DwindleTiler::new(self.gap);
                dwindle.tile_windows(monitor, windows);
            }
        }
    }

    pub fn set_gap(&mut self, gap: i32) {
        self.gap = gap;
    }

    pub fn get_gap(&self) -> i32 {
        self.gap
    }
}

impl Default for Tiler {
    fn default() -> Self {
        Self::with_algorithm(TilingAlgorithm::Dwindle)
    }
}
```

### 12.2 Add Algorithm Toggle to Workspace Manager

Update `src/workspace_manager.rs`:

```rust
use crate::tiling::{Tiler, TilingAlgorithm};

pub struct WorkspaceManager {
    monitors: Arc<Mutex<Vec<Monitor>>>,
    active_workspace_global: u8,
    tiler: Arc<Mutex<Tiler>>, // Add tiler
}

impl WorkspaceManager {
    pub fn new() -> Self {
        WorkspaceManager {
            monitors: Arc::new(Mutex::new(Vec::new())),
            active_workspace_global: 1,
            tiler: Arc::new(Mutex::new(Tiler::default())),
        }
    }

    // ... existing methods ...

    pub fn toggle_tiling_algorithm(&self) -> TilingAlgorithm {
        let mut tiler = self.tiler.lock().unwrap();
        let old_algorithm = tiler.get_algorithm();
        let new_algorithm = tiler.toggle_algorithm();
        println!("Tiling algorithm: {:?} -> {:?}", old_algorithm, new_algorithm);

        // Re-tile with new algorithm
        drop(tiler);
        let mut wm = self.monitors.lock().unwrap();
        wm.tile_active_workspaces_with_tiler(self.tiler.clone());
        wm.apply_window_positions();

        new_algorithm
    }

    pub fn set_tiling_algorithm(&self, algorithm: TilingAlgorithm) {
        let mut tiler = self.tiler.lock().unwrap();
        let old_algorithm = tiler.get_algorithm();
        tiler.set_algorithm(algorithm);
        println!("Tiling algorithm: {:?} -> {:?}", old_algorithm, algorithm);

        // Re-tile with new algorithm
        drop(tiler);
        let mut wm = self.monitors.lock().unwrap();
        wm.tile_active_workspaces_with_tiler(self.tiler.clone());
        wm.apply_window_positions();
    }

    pub fn get_tiling_algorithm(&self) -> TilingAlgorithm {
        let tiler = self.tiler.lock().unwrap();
        tiler.get_algorithm()
    }

    pub fn set_window_gap(&self, gap: i32) {
        let mut tiler = self.tiler.lock().unwrap();
        tiler.set_gap(gap);

        // Re-tile with new gap
        drop(tiler);
        let mut wm = self.monitors.lock().unwrap();
        wm.tile_active_workspaces_with_tiler(self.tiler.clone());
        wm.apply_window_positions();
    }
}
```

### 12.3 Update Monitor Tiling Methods

Update `src/workspace_manager.rs` to use tiler:

```rust
impl WorkspaceManager {
    // ... existing methods ...

    pub fn tile_active_workspaces(&self) {
        let tiler = self.tiler.clone();
        let mut monitors = self.monitors.lock().unwrap();
        monitors.tile_active_workspaces_with_tiler(tiler);
    }

    pub fn tile_active_workspaces_with_tiler(&self, tiler: Arc<Mutex<Tiler>>) {
        let mut monitors = self.monitors.lock().unwrap();

        for monitor in monitors.iter_mut() {
            let workspace_idx = (monitor.active_workspace - 1) as usize;
            let windows = &mut monitor.workspaces[workspace_idx].windows;

            if !windows.is_empty() {
                let t = tiler.lock().unwrap();
                t.tile_windows(monitor, windows);
            }
        }
    }
}
```

### 12.4 Update Hotkey Handler

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
        // ... other actions ...
    }
}
```

### 12.5 Add Algorithm Status to Workspace Display

Update `src/workspace_manager.rs`:

```rust
impl WorkspaceManager {
    pub fn print_workspace_status(&self) {
        println!("\n=== Workspace Status ===");
        println!("Active workspace: {}", self.active_workspace_global);
        println!("Tiling algorithm: {:?}", self.get_tiling_algorithm());

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

## Testing

1. Run the application
2. Open multiple applications
3. Press Win + T to toggle tiling algorithm
4. Verify:
   - Console shows algorithm change
   - Windows re-tile (though no visual change with only Dwindle)
5. Press Win + S to view status and see current algorithm
6. Verify status shows correct tiling algorithm

## Success Criteria

- [ ] Win + T toggles between tiling algorithms
- [ ] Console logs algorithm changes
- [ ] Windows are re-tiled after algorithm change
- [ ] Workspace status shows current algorithm
- [ ] Tiler is properly integrated with workspace manager

## Documentation

### Tiling Algorithm Infrastructure

**Current state**: Only Dwindle algorithm is implemented
**Purpose**: Sets up infrastructure for future algorithms

**Future algorithms to add**:
- **Spiral**: Windows spiral inward from edges
- **Stack**: Stack windows vertically on one side
- **Column**: Column-based layout (like VS Code)
- **Grid**: Simple equal grid layout
- **BSP**: More advanced binary space partitioning

### Algorithm Toggle Flow

1. **User action**: Press Win + T
2. **Get current algorithm**: Check current tiling algorithm
3. **Toggle to next**: Cycle to next algorithm (currently just Dwindle)
4. **Re-tile**: Apply new algorithm to all active workspaces
5. **Apply positions**: Update window positions
6. **Log change**: Console output for feedback

### Tiler Configuration

**Gap size**: Controls spacing between windows
- Default: 8px
- Can be adjusted programmatically
- Affects all tiling algorithms

**Algorithm selection**:
- Global setting (applies to all monitors)
- Changed via `toggle_tiling_algorithm()` or `set_tiling_algorithm()`
- Saved in Tiler struct

### When Multiple Algorithms Exist

Toggle order (example):
1. Dwindle → Spiral
2. Spiral → Stack
3. Stack → Column
4. Column → Dwindle (cycle)

Each algorithm will:
- Use the same gap size
- Apply to all monitors
- Work with existing window lists

### Limitations

Currently:
- Only Dwindle algorithm implemented
- Toggle has no visual effect
- Gap size not adjustable by user

Future:
- More algorithms needed
- User-configurable algorithms
- Per-monitor algorithm selection
- Adjustable gap size via hotkeys

## Next Steps

Proceed to [STEP_13.md](STEP_13.md) to implement fullscreen toggle (Win + F).
