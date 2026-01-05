# Megatile TODO

## Completed

- ✅ Improve status bar
  - Added numbers to workspace indicators (1-9)
  - Active workspace shown as opaque dot with accent color
  - Inactive workspace dots are semi-transparent with workspace number
  - Dots positioned at far left side of status bar
  - Date and time displayed at far right in format hh:mm dd/mm
  - Backdrop is dimmed/desaturated accent color for better readability
- ✅ Minimized gaps between status bar and windows (reduced to 2px edge gap)
- ✅ Minimized gap at bottom of monitor (reduced to 2px)
- ✅ Reduced gap between windows (from 8px to 4px, consistent on all sides)
- ✅ Ensured popups and dialog boxes are not tiled
  - Added filtering for WS_EX_DLGMODALFRAME windows
  - Added filtering for owned windows (dialogs)
  - Added filtering for popup windows without thick frames
  - Added filtering for #32770 dialog class
- ✅ Fixed bug where new window in empty workspace was tiled incorrectly
  - Layout tree is now cleared when windows are added or removed
- ✅ Fixed window sizing issues with VSCode and similar apps
  - Added DWM frame border compensation when setting window positions
  - Windows now tile to their visible bounds, not including invisible borders
- ✅ Improved filtering of fake/invisible windows
  - Added IsWindow validation
  - Added zero-alpha layered window filtering
  - Added off-screen window filtering
  - Added more system class filtering (TaskListThumbnailWnd, etc.)
  - Filter windows with empty titles

## Next Steps

(No pending tasks)
