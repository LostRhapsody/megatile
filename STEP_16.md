# STEP 16: Status Bar (Low Priority)

## Objective

Implement a visual status bar to display the active workspace number, window count, and other useful information. This is a low-priority feature that enhances user experience.

## Tasks

### 16.1 Create Status Bar Module

Create `src/statusbar.rs`:

```rust
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::WindowsAndMessaging::*;

pub struct StatusBar {
    hwnd: HWND,
    parent_hwnd: HWND,
}

impl StatusBar {
    pub fn new(parent_hwnd: HWND) -> Result<Self, String> {
        unsafe {
            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("STATIC"),
                w!(""),
                WS_CHILD | WS_VISIBLE | SS_CENTER,
                0, 0, 0, 0,
                parent_hwnd,
                HMENU::default(),
                GetModuleHandleW(None).unwrap(),
                None,
            );

            if hwnd == HWND::default() {
                return Err("Failed to create status bar window".to_string());
            }

            Ok(StatusBar { hwnd, parent_hwnd })
        }
    }

    pub fn set_text(&self, text: &str) {
        unsafe {
            SetWindowTextW(self.hwnd, &text.encode_utf16().collect::<Vec<u16>>());
        }
    }

    pub fn set_position(&self, x: i32, y: i32, width: i32, height: i32) {
        unsafe {
            SetWindowPos(
                self.hwnd,
                HWND::default(),
                x, y, width, height,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }

    pub fn set_font(&self, hfont: HFONT) {
        unsafe {
            SendMessageW(self.hwnd, WM_SETFONT, WPARAM(hfont.0 as usize), LPARAM(1));
        }
    }

    pub fn show(&self) {
        unsafe {
            ShowWindow(self.hwnd, SW_SHOW);
        }
    }

    pub fn hide(&self) {
        unsafe {
            ShowWindow(self.hwnd, SW_HIDE);
        }
    }

    pub fn update_workspace_info(&self, workspace_num: u8, window_count: usize) {
        let text = format!("Workspace {} - {} Windows", workspace_num, window_count);
        self.set_text(&text);
    }

    pub fn update_full_status(
        &self,
        workspace_num: u8,
        window_count: usize,
        tiling_algorithm: &str,
    ) {
        let text = format!(
            "Workspace {} | {} Windows | {}",
            workspace_num,
            window_count,
            tiling_algorithm
        );
        self.set_text(&text);
    }

    pub fn get_hwnd(&self) -> HWND {
        self.hwnd
    }
}
```

### 16.2 Integrate Status Bar with Workspace Manager

Update `src/workspace_manager.rs`:

```rust
use crate::statusbar::StatusBar;

pub struct WorkspaceManager {
    monitors: Arc<Mutex<Vec<Monitor>>>,
    active_workspace_global: u8,
    tiler: Arc<Mutex<Tiler>>,
    statusbar: Option<StatusBar>, // Add status bar
}

impl WorkspaceManager {
    pub fn new() -> Self {
        WorkspaceManager {
            monitors: Arc::new(Mutex::new(Vec::new())),
            active_workspace_global: 1,
            tiler: Arc::new(Mutex::new(Tiler::default())),
            statusbar: None, // Status bar created later
        }
    }

    pub fn set_statusbar(&mut self, statusbar: StatusBar) {
        self.statusbar = Some(statusbar);
    }

    pub fn update_statusbar(&self) {
        if let Some(statusbar) = &self.statusbar {
            let workspace_num = self.active_workspace_global;
            let window_count = self.get_active_workspace_window_count();
            let tiling_algorithm = format!("{:?}", self.get_tiling_algorithm());

            statusbar.update_full_status(
                workspace_num,
                window_count,
                &tiling_algorithm,
            );
        }
    }

    // Update other methods to call update_statusbar()

    pub fn switch_workspace_with_windows(&mut self, new_workspace: u8) -> Result<(), String> {
        // ... existing code ...

        // Update status bar
        self.update_statusbar();

        Ok(())
    }

    pub fn toggle_tiling_algorithm(&self) -> TilingAlgorithm {
        // ... existing code ...

        // Update status bar
        self.update_statusbar();

        new_algorithm
    }

    pub fn close_focused_window(&mut self) -> Result<(), String> {
        // ... existing code ...

        // Update status bar
        self.update_statusbar();

        Ok(())
    }
}
```

### 16.3 Create Status Bar Window

Update `src/main.rs` to create status bar:

```rust
use crate::statusbar::StatusBar;

// Add this to main() before the event loop
fn create_status_bar(parent_hwnd: HWND) -> Result<StatusBar, String> {
    StatusBar::new(parent_hwnd)
}

// In main(), after creating message window
let statusbar = create_status_bar(hwnd)?;

// Set status bar position and size (top center of primary monitor)
let monitor_infos = windows::enumerate_monitors();
if let Some(primary_monitor) = monitor_infos.first() {
    let rect = primary_monitor.rect;
    let statusbar_width = 400;
    let statusbar_height = 30;
    let x = rect.left + (rect.right - rect.left - statusbar_width) / 2;
    let y = rect.top + 10;

    statusbar.set_position(x, y, statusbar_width, statusbar_height);
}

// Set status bar to topmost
unsafe {
    use windows::Win32::UI::WindowsAndMessaging::*;
    SetWindowPos(
        statusbar.get_hwnd(),
        HWND_TOPMOST,
        0, 0, 0, 0,
        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
    );
}

// Update workspace manager with status bar
workspace_manager.lock().unwrap().set_statusbar(statusbar);
```

### 16.4 Add Hotkey to Toggle Status Bar

Add hotkey to show/hide status bar:

```rust
// In src/hotkeys.rs, add new hotkey
pub enum HotkeyAction {
    // ... existing actions ...
    ToggleStatusBar,
}

// Register Win + B for status bar toggle
(MOD_WIN, 0x42, 33, HotkeyAction::ToggleStatusBar), // B
```

```rust
// In main.rs, store statusbar visibility state
use std::sync::atomic::{AtomicBool, Ordering};

let statusbar_visible = Arc::new(AtomicBool::new(true));

// In hotkey handler
hotkeys::HotkeyAction::ToggleStatusBar => {
    let mut visible = statusbar_visible.load(Ordering::SeqCst);
    visible = !visible;
    statusbar_visible.store(visible, Ordering::SeqCst);

    if visible {
        statusbar.show();
    } else {
        statusbar.hide();
    }

    println!("Status bar: {}", if visible { "visible" } else { "hidden" });
}
```

### 16.5 Add Styling

Make status bar more visually appealing:

```rust
// In src/main.rs, create a custom font
fn create_statusbar_font() -> Result<HFONT, String> {
    unsafe {
        let font = CreateFontW(
            20, // Height
            0,  // Width
            0,  // Escapement
            0,  // Orientation
            FW_NORMAL, // Weight
            0,  // Italic
            0,  // Underline
            0,  // StrikeOut
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY,
            DEFAULT_PITCH | FF_DONTCARE,
            w!("Segoe UI"),
        );

        if font.is_invalid() {
            return Err("Failed to create font".to_string());
        }

        Ok(font)
    }
}

// Apply font to status bar
let font = create_statusbar_font()?;
statusbar.set_font(font);
```

## Testing

1. Run the application
2. Verify status bar appears at top center of screen
3. Open applications and verify status bar shows window count
4. Switch workspaces and verify workspace number updates
5. Press Win + B to toggle status bar visibility
6. Verify status bar hides/shows correctly
7. Toggle tiling algorithm and verify status bar updates

## Success Criteria

- [ ] Status bar displays at top center of screen
- [ ] Shows current workspace number
- [ ] Shows window count
- [ ] Shows tiling algorithm
- [ ] Updates when workspace changes
- [ ] Updates when window count changes
- [ ] Can be toggled with Win + B
- [ ] Has proper styling and font

## Documentation

### Status Bar Design

**Position**: Top center of primary monitor
**Size**: 400px width, 30px height
**Style**: Centered text, Segoe UI font, 20px height
**Z-order**: Topmost (visible above all windows)

### Status Bar Content

**Format**: `Workspace N | X Windows | Algorithm`
- N: Current workspace number (1-9)
- X: Number of windows in active workspace
- Algorithm: Current tiling algorithm name

**Update triggers**:
- Workspace switch
- Window added/closed
- Algorithm toggle
- Window moved to/from workspace

### Status Bar Features

**Visibility toggle**:
- Win + B: Show/hide status bar
- Preserves state across workspace switches
- Updates when visible

**Styling**:
- ClearType quality font rendering
- Segoe UI (Windows system font)
- Topmost z-order for visibility

### Status Bar Implementation

**Window type**: STATIC control (text display)
**Parent**: Message-only window (for message handling)
**Messages**: WM_SETFONT for custom font
**Text updates**: SetWindowTextW API

### Performance Considerations

**Update frequency**:
- Only updates when data changes
- Not on every frame or tick
- Minimizes redraws

**Rendering**:
- Static control handles rendering
- No custom drawing needed
- Efficient for simple text display

### Limitations

- Single status bar (per primary monitor only)
- Fixed position (top center)
- Fixed size (400x30)
- No custom positioning
- No configuration options

### Future Enhancements

1. **Per-monitor status bars**: One per monitor
2. **Customizable position**: User-defined position
3. **Configurable content**: Toggle info fields
4. **Custom colors/themes**: Match system theme
5. **More information**: Battery, time, etc.
6. **Interactive elements**: Click to cycle workspaces
7. **Resizable/draggable**: Full window manager for status bar

### Integration with Other Features

**Workspace switching**: Updates workspace number and window count
**Window operations**: Updates window count when windows are closed
**Algorithm toggle**: Updates algorithm name
**Multi-monitor**: Currently only on primary monitor

### MVP Completion

This step completes the MVP for MegaTile! All core features are now implemented:
- ✅ Window enumeration and filtering
- ✅ System tray integration
- ✅ Global hotkey registration
- ✅ Workspace management
- ✅ Window hiding/showing
- ✅ Dwindle tiling algorithm
- ✅ Focus movement
- ✅ Window movement
- ✅ Workspace switching
- ✅ Move windows to workspaces
- ✅ Window closing
- ✅ Toggle tiling algorithm
- ✅ Toggle fullscreen
- ✅ Multi-monitor support
- ✅ Auto-start configuration
- ✅ Status bar

## Next Steps

The MVP is complete! Future work could include:
- Additional tiling algorithms (Spiral, Stack, Column)
- Window gaps adjustment
- Status bar customization
- Window event hooks for automatic cleanup
- Per-monitor workspace selection
- Configuration file support
- Visual indicators (workspace numbers, window borders)
- Scratchpad workspace
- Floating window mode

These can be implemented as needed based on user feedback.
