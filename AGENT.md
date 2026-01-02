# MegaTile Agent Handoff Guide

## Project Status

**Phase**: Implementation in Progress
**Next Action**: Complete STEP_6: Dwindle tiling algorithm

## Project Notes

**Cargo.toml**: It's DONE! Don't change the Rust edition, windows dependencies, or features.

## Project Overview

MegaTile is a fast, lightweight, opinionated tiling window manager for Windows built in Rust. It uses the Windows API directly for maximum performance and minimal dependencies.

## Current State

- ✅ Documentation created (README.md, PLAN.md, STEP_1 through STEP_16)
- ✅ Architecture planned
- ✅ Dependencies identified
- ✅ Implementation started (STEP_6)

## File Structure

```
megatile/
├── README.md           # Project overview and quick start
├── PLAN.md            # Long-term plan and architecture
├── AGENT.md           # This file - agent handoff guide
├── STEP_1.md          # Project scaffolding and window enumeration
├── STEP_2.md          # System tray integration
├── STEP_3.md          # Global hotkey registration
├── STEP_4.md          # Workspace data structures
├── STEP_5.md          # Window hiding/showing
├── STEP_6.md          # Dwindle tiling algorithm
├── STEP_7.md          # Focus movement
├── STEP_8.md          # Window movement
├── STEP_9.md          # Workspace switching
├── STEP_10.md         # Move windows to workspaces
├── STEP_11.md         # Window closing
├── STEP_12.md         # Toggle tiling algorithm
├── STEP_13.md         # Toggle fullscreen
├── STEP_14.md         # Multi-monitor support
├── STEP_15.md         # Auto-start configuration
├── STEP_16.md         # Status bar (low priority)
└── src/               # Source code (to be created)
```

## Implementation Roadmap

### Phase 1: Foundation (Steps 1-5)
- STEP_1: Project scaffolding + window enumeration
- STEP_2: System tray integration
- STEP_3: Global hotkey registration
- STEP_4: Workspace data structures
- STEP_5: Window hiding/showing

### Phase 2: Core Tiling (Steps 6-8)
- STEP_6: Dwindle tiling algorithm
- STEP_7: Focus movement
- STEP_8: Window movement

### Phase 3: Workspace Management (Steps 9-10)
- STEP_9: Workspace switching
- STEP_10: Move windows to workspaces

### Phase 4: Window Operations (Steps 11-13)
- STEP_11: Window closing
- STEP_12: Toggle tiling algorithm
- STEP_13: Toggle fullscreen

### Phase 5: Polish (Steps 14-16)
- STEP_14: Multi-monitor support
- STEP_15: Auto-start configuration
- STEP_16: Status bar

## Key Technologies

- **Rust**: Primary language
- **Windows API**: Direct Windows API access via `windows` crate
- **tray-icon**: System tray integration
- **winit**: Event loop (may be replaced with direct Windows API)

## Core Dependencies

```toml
[dependencies]
windows = { version = "0.52", features = [
    "Win32_Foundation",
    "Win32_UI_WindowsAndMessaging",
    "Win32_Graphics_Gdi",
    "Win32_System_LibraryLoader",
    "Win32_UI_Shell",
]}
tray-icon = "0.14"
winit = "0.29"
```

## Key Concepts

### Workspace Model
- 9 workspaces (numbered 1-9)
- Shared across all monitors
- Each monitor has independent window lists per workspace
- Inactive workspace windows hidden from taskbar

### Window Filtering
- Only "normal" windows are tiled
- Exclude: taskbar, dialogs, tool windows, minimized windows
- Include: visible, non-minimized, user-facing windows

### Tiling Algorithm
- Dwindle (binary space partitioning)
- Configurable gap (default 8px)
- Per-monitor independent tiling

### Keybindings
- Win + Arrows: Move focus
- Win + Shift + Arrows: Move windows
- Win + 1-9: Switch workspaces
- Win + Shift + 1-9: Move windows to workspaces
- Win + W: Close window
- Win + T: Toggle tiling algorithm
- Win + F: Toggle fullscreen

## Module Architecture

### `src/windows_lib.rs`
- Window enumeration (`enumerate_windows`)
- Window filtering (`is_normal_window`)
- Window operations (show, hide, close, fullscreen)
- Monitor enumeration (`enumerate_monitors`)

### `src/workspace.rs`
- Core data structures: `Window`, `Workspace`, `Monitor`
- Window and workspace management
- Focus tracking

### `src/workspace_manager.rs`
- High-level workspace operations
- Monitor management
- Workspace switching
- Window movement and closing

### `src/tiling.rs`
- Dwindle tiling algorithm
- Tiling algorithms enum
- Tile calculation and positioning

### `src/hotkeys.rs`
- Hotkey registration
- Hotkey action enum
- Hotkey to action mapping

### `src/tray.rs`
- System tray icon
- Tray menu
- Exit handling

### `src/autostart.rs`
- Registry operations
- Auto-start enable/disable
- Auto-start state checking

### `src/statusbar.rs`
- Status bar window
- Status updates
- Visibility toggle

### `src/main.rs`
- Application entry point
- Event loop
- Hotkey handling
- Module initialization

## Testing Strategy

Each step includes:
- Manual testing instructions
- Success criteria checklist
- Expected behaviors

**Important**: After completing each step:
1. Run `cargo build` to ensure compilation
2. Run `cargo clippy` for linting
3. Test manually per step instructions
4. Verify all success criteria are met

## Common Patterns

### Error Handling
```rust
pub fn some_function() -> Result<(), String> {
    // ...
    Err("Error message".to_string())
}
```

### Windows API Safety
```rust
unsafe {
    // Windows API calls
}
```

### Thread-Safe State
```rust
use std::sync::{Arc, Mutex};

pub struct WorkspaceManager {
    monitors: Arc<Mutex<Vec<Monitor>>>,
    // ...
}
```

## Known Limitations

- Window event hooks not implemented (external closes not detected)
- Only Dwindle tiling algorithm (more to come)
- Status bar fixed position (top center of primary monitor)
- No per-monitor workspace selection
- No configuration file support
- No DPI awareness for multi-monitor setups

## Environment Notes

- Working directory: `C:\Users\evanr\megatile`
- Platform: Windows (win32)
- Target: Windows 10/11
- Build: `cargo build --release`

## Next Steps for Agent

1. **Start with STEP_1.md**:
   - Initialize Rust project
   - Add dependencies
   - Implement window enumeration
   - Test window filtering

2. **Follow each step sequentially**:
   - Read the step file carefully
   - Implement the code
   - Test per instructions
   - Verify success criteria

3. **Keep code minimal and focused**:
   - Only implement what's specified
   - Don't add extra features
   - Follow existing patterns

4. **Document decisions**:
   - If deviating from plan, note why
   - Update AGENT.md with any changes
   - Keep README and PLAN.md in sync

5. **After each step**:
   - Ensure code compiles
   - Run tests
   - Update AGENT.md with progress

## Progress Tracking

When starting a new agent session:

1. Check which steps are completed
2. Read the latest STEP_N.md to continue
3. Update AGENT.md with completed steps

Completed steps:
- [x] STEP_1: Project scaffolding and window enumeration
- [x] STEP_2: System tray integration
- [x] STEP_3: Global hotkey registration
- [x] STEP_4: Workspace data structures
- [x] STEP_5: Window hiding/showing
- [ ] STEP_6: Dwindle tiling algorithm
- [ ] STEP_7: Focus movement
- [ ] STEP_8: Window movement
- [ ] STEP_9: Workspace switching
- [ ] STEP_10: Move windows to workspaces
- [ ] STEP_11: Window closing
- [ ] STEP_12: Toggle tiling algorithm
- [ ] STEP_13: Toggle fullscreen
- [ ] STEP_14: Multi-monitor support
- [ ] STEP_15: Auto-start configuration
- [ ] STEP_16: Status bar

## Troubleshooting

**Common issues**:
1. **Windows API errors**: Check unsafe blocks and function signatures
2. **Borrow checker issues**: Review Arc<Mutex<>> usage
3. **Hotkey conflicts**: Test different key combinations
4. **Window filtering issues**: Adjust `is_normal_window` logic
5. **Tiling not working**: Verify window enumeration and monitor rects

**Resources**:
- Windows API documentation: https://docs.microsoft.com/en-us/windows/win32/api/
- Windows crate docs: https://docs.rs/windows/
- Rust documentation: https://doc.rust-lang.org/

## Contact & Support

- Project location: `C:\Users\evanr\megatile`
- Issue tracker: (to be added)
- Documentation: README.md, PLAN.md, STEP_N.md

## Notes for Future Agents

- This is a minimal, opinionated window manager
- Don't add features beyond what's specified
- Keep dependencies minimal
- Performance is a priority
- Test thoroughly on Windows
- All Windows API calls should be in `unsafe` blocks
- Use `Arc<Mutex<>>` for shared state

---

**Last Updated**: Initial creation
**MVP Target**: All 16 steps complete
