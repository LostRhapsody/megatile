# STEP 8: Window Movement (Alt + Shift + Arrows)

## Objective

Implement window movement functionality using Alt + Shift + Arrow keys to swap windows within the tiling layout.

## Status

**COMPLETED** - Window movement has been implemented and integrated into main.rs.

## Implementation Summary

### Core Features Implemented

1. **Window Position Swapping** (`src/workspace_manager.rs`)
   - `move_window()` method finds adjacent window in specified direction
   - `swap_window_positions()` swaps the rectangles of two windows
   - Uses the same `find_next_focus()` algorithm to identify target windows
   - Re-applies tiling after swap to ensure proper positioning
   - Focus follows the moved window

2. **Hotkey Integration** (`src/main.rs`)
   - Alt + Shift + Left/Right/Up/Down registered in `hotkeys.rs`
   - Hotkey handler calls `move_window()` with appropriate direction
   - Error handling and logging for movement operations

3. **Tiling Preservation**
   - After swapping window rectangles, `tile_active_workspaces()` is called
   - `apply_window_positions()` updates actual window positions
   - Gaps and layout structure maintained

## How It Works

1. User presses Alt + Shift + Arrow key
2. System identifies currently focused window
3. `find_next_focus()` locates adjacent window in specified direction
4. Window rectangles are swapped in workspace data structure
5. Tiling is re-applied to ensure consistent layout
6. Window positions are updated via Windows API
7. Focus is restored to the moved window

## Testing

Manual testing confirms:
- Windows swap positions correctly in all directions
- Tiling layout remains intact after movement
- Focus follows the moved window
- Edge cases handled (no target window in direction)
- Multiple windows (2-6) tested successfully

## Success Criteria

- [x] Alt + Shift + Arrow keys move windows between tile positions
- [x] Tiling layout remains intact after window movement
- [x] Focus correctly follows the moved window
- [x] No crashes or unexpected behavior during movement
- [x] Console logs movement actions for debugging

## Known Limitations

- Movement is based on spatial proximity, not tiling tree structure
- Cannot move windows to arbitrary positions (only swap with adjacent)
- Multi-monitor movement not yet implemented (for STEP_14)
- No animation during window movement

## Code Changes

### src/workspace_manager.rs
- Added `move_window()` method (lines 384-449)
- Added `swap_window_positions()` helper method (lines 451-493)
- Reuses existing `find_next_focus()` and `set_window_focus()` methods

### src/main.rs
- Updated `handle_hotkey()` to handle MoveLeft/Right/Up/Down actions (lines 230-294)
- Added proper error handling and logging

### src/hotkeys.rs
- Hotkeys already registered with MOD_ALT | MOD_SHIFT (lines 48-51)

## Next Steps

Proceed to [STEP_9.md](STEP_9.md) to implement workspace switching (already partially complete) or [STEP_10.md](STEP_10.md) to implement moving windows to different workspaces.
