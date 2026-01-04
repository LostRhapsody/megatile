# MegaTile TODO

## High Priority

### 1. New Window Registration
**Issue**: Opening a new window triggers the window created event and logs it, but the window is not registered with MegaTile and doesn't get tiled.

**Status**: Open
**Severity**: Critical

**Description**:
- Windows event hook detects new window creation
- Event is queued and processed in main loop
- Window appears in logs but is not added to workspace management
- Window does not get tiled automatically

**Possible Causes**:
- Window filtering logic may be rejecting normal windows
- Window enumeration in event handler may not be finding the new window
- Race condition between window creation and our enumeration
- Window may be created with different initial state than expected

**Steps to Fix**:
- Debug why `is_normal_window` check fails for new windows
- Verify window enumeration in event handler finds newly created windows
- Add delay/retry logic if needed for window initialization
- Ensure new windows are added to active workspace and tiled

### 2. Fullscreen Toggle Position Restoration
**Issue**: When triggering fullscreen then un-triggering it, the window remains in its fullscreen position instead of returning to its tiled position.

**Status**: Open
**Severity**: High

**Description**:
- Fullscreen mode works (window goes fullscreen)
- Restoring from fullscreen keeps window in fullscreen position
- Expected behavior: Window should return to its original tiled position

**Possible Causes**:
- Original rectangle is not being stored correctly before fullscreen
- Restoration logic uses wrong rectangle (fullscreen rect instead of tiled rect)
- Window position update overwrites stored original rectangle

**Steps to Fix**:
- Verify `original_rect` is stored before entering fullscreen
- Ensure `restore_window_from_fullscreen` uses the correct `original_rect`
- Check that fullscreen restoration doesn't trigger position updates that overwrite the rect
- Test fullscreen toggle restores to tiled position correctly

## Medium Priority

### 3. Window Move Detection
**Issue**: Window move events are queued but may not be processed efficiently.

**Status**: Open
**Severity**: Medium

**Description**:
- `EVENT_OBJECT_LOCATIONCHANGE` is hooked
- All move events trigger position updates
- May cause excessive CPU usage for frequent moves

**Steps to Fix**:
- Consider debouncing move events
- Only update positions when move is complete (not during drag)
- Optimize position update logic

### 4. Monitor Hotplug Handling
**Issue**: Monitor addition/removal events are not currently hooked.

**Status**: Open
**Severity**: Medium

**Description**:
- Only periodic checks detect monitor changes
- No real-time detection of monitor hotplug events
- Could cause issues during monitor changes

**Steps to Fix**:
- Add `RegisterDeviceNotification` for monitor device changes
- Handle `WM_DEVICECHANGE` messages in event loop
- Test monitor hotplug scenarios

## Low Priority

### 5. Window Destruction Cleanup
**Issue**: Window destruction events are handled but may need cleanup verification.

**Status**: Open
**Severity**: Low

**Description**:
- Windows are removed from management when destroyed
- Need to verify no memory leaks or orphaned state

### 6. Event Queue Performance
**Issue**: Event queue uses simple `VecDeque` - may need optimization for high-frequency events.

**Status**: Open
**Severity**: Low

**Description**:
- Single event per iteration may not scale with many rapid events
- Consider batching similar events or optimizing queue processing

## Completed

- ✅ Transition to single-threaded event-driven architecture
- ✅ Remove all mutex locking and timeout issues
- ✅ Implement Windows event hooks for window events
- ✅ Create event queue system
- ✅ Simplify WorkspaceManager data structures