# MegaTile Long-Term Plan

## Project Vision

Build a fast, lightweight, opinionated tiling window manager for Windows that uses minimal dependencies and provides a streamlined window management experience.

## Core Principles

1. **Performance First**: Use Windows API directly, no abstraction layers
2. **Minimal Dependencies**: Only essential crates, keep binary small
3. **Opinionated Design**: Fixed keybindings, no configuration needed
4. **Workspace-First**: 9 workspaces shared across monitors
5. **Smart Filtering**: Only tile normal windows, ignore system UI

## Technical Architecture

### Components

1. **Window Monitor**
   - Enumerate all top-level windows
   - Filter for normal windows only (exclude WS_EX_TOOLWINDOW, taskbar, dialogs)
   - Track window creation/destruction events
   - Monitor window state changes (minimized, maximized, etc.)

2. **Workspace Manager**
   - 9 workspaces (numbered 1-9)
   - Each workspace tracks windows per monitor
   - Active workspace shown on all monitors
   - Inactive workspace windows hidden from taskbar

3. **Tiling Engine**
   - Dwindle algorithm (binary space partitioning)
   - Per-monitor independent layouts
   - Recalculate on window changes
   - Support for fullscreen toggle

4. **Input Handler**
   - Register global hotkeys with Win modifier
   - Process key events
   - Route to appropriate handlers

5. **Window Hider**
   - Hide windows on inactive workspaces (SetWindowPos with SWP_HIDEWINDOW)
   - Remove from taskbar when hidden
   - Restore windows when workspace becomes active

6. **System Tray**
   - Tray icon with menu
   - Exit option
   - Auto-start toggle
   - Optional status bar

### Data Structures

```rust
struct Window {
    hwnd: HWND,
    workspace: u8,
    monitor: usize,
    rect: RECT,
    is_focused: bool,
}

struct Workspace {
    windows: Vec<Window>,
}

struct Monitor {
    hmonitor: HMONITOR,
    rect: RECT,
    workspaces: [Workspace; 9],
}

struct MegaTileState {
    monitors: Vec<Monitor>,
    active_workspace: u8,
    tiling_algorithm: Algorithm,
    is_fullscreen: bool,
}
```

### Window Filtering Logic

- Include: WS_EX_APPWINDOW, visible, not minimized
- Exclude: WS_EX_TOOLWINDOW, WS_EX_NOACTIVATE, windows with no title
- Special handling: Taskbar (Shell_TrayWnd), start menu, system dialogs

### Hotkey Registration

Use `RegisterHotKey` API with MOD_WIN (0x8):
- Arrow keys: 0x25-0x28 (VK_LEFT, VK_UP, VK_RIGHT, VK_DOWN)
- Number keys: 0x31-0x39 (VK_1-VK_9)
- Letters: W (0x57), T (0x54), F (0x46)

## Implementation Phases

### Phase 1: Foundation (Steps 1-5)
- Project setup and dependencies
- Window enumeration and filtering
- System tray integration
- Hotkey registration
- Workspace data structures and switching

### Phase 2: Core Tiling (Steps 6-8)
- Dwindle tiling algorithm
- Focus movement
- Window movement between tiles

### Phase 3: Workspace Management (Steps 9-10)
- Workspace switching
- Moving windows between workspaces
- Window hiding/showing

### Phase 4: Window Operations (Steps 11-13)
- Window closing
- Tiling algorithm toggle
- Fullscreen toggle

### Phase 5: Polish (Steps 14-16)
- Multi-monitor support
- Auto-start configuration
- Status bar (low priority)

## Technical Challenges & Solutions

### Challenge 1: Hiding windows from taskbar
**Solution**: Use `SetWindowPos(hwnd, HWND_BOTTOM, 0, 0, 0, 0, SWP_HIDEWINDOW | SWP_NOACTIVATE)` and remove from taskbar by temporarily removing WS_EX_APPWINDOW style

### Challenge 2: Multi-monitor tiling
**Solution**: Enumerate monitors with `EnumDisplayMonitors`, maintain independent tiling state per monitor, handle monitor hot-plug events

### Challenge 3: Window creation/destruction tracking
**Solution**: Use `SetWinEventHook` with EVENT_OBJECT_CREATE and EVENT_OBJECT_DESTROY to detect window changes

### Challenge 4: Preventing window resize fights
**Solution**: Use `SetWindowPos` in response to window messages, implement a cooldown period to avoid fighting with applications that self-resize

### Challenge 5: Fullscreen apps (games, video)
**Solution**: Detect fullscreen windows by checking if window rect equals monitor rect, skip tiling for fullscreen windows

## Dependencies

**Core**:
- `windows` crate (Windows API bindings)
- `windows-sys` crate (system-specific types)

**Optional**:
- `tray-icon` (system tray)
- `winit` (event loop) - may use windows-rs directly instead

**Testing**:
- `windows-test` (Windows API testing utilities)

## Performance Goals

- Startup time: < 1 second
- Workspace switch: < 50ms
- Window move: < 100ms
- Memory footprint: < 20MB
- Binary size: < 2MB

## Future Enhancements (Post-MVP)

- Additional tiling algorithms (Spiral, Stack)
- Visual indicators (workspace numbers, window borders)
- Scratchpad/popup workspace
- Window gaps and padding
- Floating windows toggle
- Status bar with workspace indicators
- Config file support (optional)

## Success Criteria

- All keybindings work reliably
- Workspace switching hides/shows windows correctly
- Tiling works on single and multi-monitor setups
- Normal windows are tiled, system windows are ignored
- System tray provides exit and auto-start options
- Application is stable and responsive

## Risks & Mitigations

**Risk**: Windows API complexity
**Mitigation**: Start with minimal API usage, expand gradually, use documentation extensively

**Risk**: Hotkey conflicts with other apps
**Mitigation**: Document conflicts, provide option to disable specific hotkeys if needed

**Risk**: Window filtering may miss edge cases
**Mitigation**: Test with various applications, iterate on filtering logic

**Risk**: Multi-monitor edge cases (different resolutions, DPI)
**Mitigation**: Handle DPI scaling, test with various monitor configurations
