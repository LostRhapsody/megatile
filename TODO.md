# Megatile TODO

## Next Steps

- Improve status bar
  - add numbers
  - the active workspace should be an opaque dot using the accent color (current functionality)
  - inactive workspace dots should be semi-transparent dots with the workspace number in it, i.e. 1-9.
  - the dots should sit at the far left side of the status bar, and on the far right side of the bar should be the date and time in the format hh:mm dd/mm
  - the backdrop of the bar should be the accent color, but dimmed or desaturated to make the numbers easier to read and make it a bit more subtle.
- Ensure the gap between the status bar and windows is minimal, minimize wasted workspace
- Ensure the gap between the bottom of the monitor and the windows is minimal, a few pixels
- Ensure the gap between windows is relatively minimal as well, right now it's a bit too large, make it consistent on all sides.
- Ensure popups and dialog boxes are not tiled.
- Sometimes, when opening a new window in an EMPTY workspace, the window is tiled as if there are MANY windows in that workspace. A case of it not being filtered correctly I imagine.
- Some windows, like VSCode, seem to not fit quite right. They are a bit wider or a bit taller for some weird reason.
- Still getting lots of fake windows. Had 3 open, but after shutting down, had 7 being tracked. Make sure we filter out OS windows or anything invisible to the user.
