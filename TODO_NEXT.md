# TODO_NEXT.md

## Issues to Fix Tomorrow

### 1. Instant Tiling After Window Move
- **Problem**: When moving a window to a new workspace, the source workspace doesn't re-tile immediately. The window moves and focus switches, but the remaining windows in the source workspace keep their old positions until the workspace is switched back.
- **Current Behavior**: `switch_workspace_with_windows` only tiles the new active workspaces. The old active workspace, now inactive with one less window, isn't re-tiled.
- **Solution**: In `move_window_to_workspace`, before switching, check if the source workspace was active. If so, call `tile_active_workspaces()` and `apply_window_positions()` on the current active workspace, then proceed with the switch.
- **Testing**: After fix, move a window from an active workspace with multiple windows; verify the source workspace re-tiles instantly.

### 2. Window Cleanup on Shutdown
- **Problem**: When MegaTile shuts down, hidden windows remain hidden and can't be found/seen anymore. The `cleanup_on_exit` function doesn't properly restore all windows.
- **Investigation Needed**:
  - Check if `show_window_in_taskbar` calls succeed for all windows.
  - Verify that all managed windows are included in the cleanup loop.
  - Test if windows hidden by `hide_window_from_taskbar` are correctly shown during cleanup.
  - Check for any race conditions or issues with window state persistence.
- **Potential Fixes**:
  - Ensure cleanup iterates over all windows in `self.get_all_managed_hwnds()` instead of per-workspace.
  - Add logging to `cleanup_on_exit` to see which windows fail to show.
  - Force show all windows regardless of current workspace state.
- **Testing**: Run MegaTile, move windows between workspaces (causing hides/shows), then exit and verify all windows are visible in the taskbar.

## Additional Notes
- Prioritize the tiling fix as it's core functionality.
- For cleanup, start with logging to diagnose the issue.
- After fixes, run full test suite: move windows, switch workspaces, exit, and restart to ensure state consistency.</content>
<parameter name="filePath">TODO_NEXT.md