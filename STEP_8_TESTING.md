# STEP 8: Window Movement - Testing Guide

## Test Steps

### Prerequisites
1. Build the project: `cargo build --release`
2. Close any running MegaTile instances
3. Open 3-4 test applications (e.g., Notepad, Calculator, File Explorer)

### Test Scenario 1: Basic Window Movement
1. **Start MegaTile**
   - Run: `.\target\release\megatile.exe`
   - Verify tray icon appears
   - Verify console shows initial tiling applied

2. **Test Horizontal Movement**
   - Note the current window layout (left/right positions)
   - Press `Alt + Shift + Right Arrow`
   - **Expected**: Focused window swaps with window to its right
   - **Verify**: Console logs "Moved window right"
   - **Verify**: Windows physically swap positions on screen
   - Press `Alt + Shift + Left Arrow`
   - **Expected**: Window moves back to original position
   - **Verify**: Console logs "Moved window left"

3. **Test Vertical Movement**
   - Note the current window layout (top/bottom positions)
   - Press `Alt + Shift + Down Arrow`
   - **Expected**: Focused window swaps with window below it
   - **Verify**: Console logs "Moved window down"
   - **Verify**: Windows physically swap positions on screen
   - Press `Alt + Shift + Up Arrow`
   - **Expected**: Window moves back to original position
   - **Verify**: Console logs "Moved window up"

### Test Scenario 2: Edge Cases
1. **Movement at Screen Edge**
   - Move focus to leftmost window (Alt + Left Arrow repeatedly)
   - Press `Alt + Shift + Left Arrow`
   - **Expected**: No movement occurs (no window to the left)
   - **Verify**: Console logs movement attempt
   - **Verify**: No crash or error

2. **Single Window**
   - Close all windows except one
   - Press `Alt + Shift + Any Arrow`
   - **Expected**: No movement occurs
   - **Verify**: Application remains stable

3. **Rapid Movement**
   - Press `Alt + Shift + Right Arrow` multiple times quickly
   - **Expected**: Window continues to move right through layout
   - **Verify**: Each movement is applied correctly
   - **Verify**: No race conditions or crashes

### Test Scenario 3: Focus Tracking
1. **Focus Follows Window**
   - Note which window is focused
   - Press `Alt + Shift + Right Arrow`
   - **Expected**: Focus stays with the moved window (follows it)
   - **Verify**: Same application remains in foreground
   - **Verify**: Titlebar shows as active

2. **Movement + Focus Navigation**
   - Press `Alt + Shift + Right Arrow` to move window
   - Press `Alt + Left Arrow` to change focus
   - **Expected**: Focus moves to different window
   - Press `Alt + Shift + Up Arrow`
   - **Expected**: Newly focused window moves up

### Test Scenario 4: Complex Layouts
1. **Four Window Grid**
   - Open exactly 4 windows
   - Verify they tile in 2x2 grid
   - Move top-left window right: `Alt + Shift + Right`
   - **Expected**: Swaps with top-right window
   - Move it down: `Alt + Shift + Down`
   - **Expected**: Swaps with bottom window

2. **Six Window Layout**
   - Open 6 windows
   - Test movement in various directions
   - **Verify**: All movements preserve tiling gaps
   - **Verify**: No windows overlap after movement

### Test Scenario 5: Workspace Integration
1. **Movement in Different Workspaces**
   - Switch to workspace 2: `Alt + 2`
   - Open 2-3 windows in workspace 2
   - Test window movement: `Alt + Shift + Arrows`
   - **Expected**: Movement works in workspace 2
   - Switch back to workspace 1: `Alt + 1`
   - **Expected**: Original layout preserved

## Validation Criteria

### ✅ Functional Requirements
- [ ] Alt + Shift + Arrow keys trigger window movement
- [ ] Windows swap positions correctly in all 4 directions
- [ ] Tiling layout remains intact after each movement
- [ ] Gaps (8px default) are preserved between windows
- [ ] Focus follows the moved window
- [ ] No crashes or exceptions during movement

### ✅ Edge Case Handling
- [ ] Moving at screen edges doesn't crash
- [ ] Moving with single window doesn't crash
- [ ] Moving with no adjacent window logs appropriately
- [ ] Rapid movement operations complete correctly

### ✅ Console Output
Expected console messages:
```
Moving left
Swapping window positions...
Locking windows
Finding windows...
Windows found.
Swapping windows...
Re-applying window positions
Window positions swapped
Focus restored
Moved window left
```

### ✅ Visual Verification
- [ ] Windows physically move on screen
- [ ] No flickering or visual artifacts
- [ ] Window borders remain properly drawn
- [ ] Taskbar icons remain correct

### ✅ Performance
- [ ] Movement completes in < 100ms (feels instant)
- [ ] No lag when moving windows
- [ ] CPU usage remains reasonable

## Known Issues / Limitations
1. Movement is spatial (based on position), not tree-based
2. Can only swap with adjacent windows
3. No animation during movement
4. Multi-monitor movement not yet supported

## Troubleshooting

### Issue: Windows don't move
- **Check**: Are hotkeys registered? Look for console output
- **Check**: Is a window focused? Try clicking a window first
- **Solution**: Restart MegaTile

### Issue: Windows move but positions are wrong
- **Check**: Console for error messages
- **Solution**: This may indicate tiling algorithm issue
- **Action**: Report in STEP_8.md notes

### Issue: Focus lost after movement
- **Check**: Console shows "Focus restored"
- **Solution**: This is a known limitation of SetForegroundWindow
- **Workaround**: Manually click the window

## Success Confirmation

When all tests pass, you can confirm:
✅ **STEP_8 is COMPLETE**

Update AGENT.md:
```markdown
- [x] STEP_8: Window movement
```

## Next Steps
Proceed to STEP_9 (Workspace switching - partially complete) or STEP_10 (Move windows to workspaces).
