# STEP 3: Global Hotkey Registration

## Objective

Register global hotkeys using the Windows API and implement basic key event handling to detect when hotkeys are pressed.

## Tasks

### 3.1 Add Hotkey Module

Create `src/hotkeys.rs`:

```rust
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::*;
use std::collections::HashMap;

pub struct HotkeyManager {
    registered_hotkeys: HashMap<i32, HotkeyAction>,
}

#[derive(Debug, Clone, Copy)]
pub enum HotkeyAction {
    // Focus movement
    FocusLeft,
    FocusRight,
    FocusUp,
    FocusDown,

    // Window movement
    MoveLeft,
    MoveRight,
    MoveUp,
    MoveDown,

    // Workspace switching
    SwitchWorkspace(u8),
    MoveToWorkspace(u8),

    // Window operations
    CloseWindow,
    ToggleTiling,
    ToggleFullscreen,
}

impl HotkeyManager {
    pub fn new() -> Self {
        Self {
            registered_hotkeys: HashMap::new(),
        }
    }

    pub fn register_hotkeys(&mut self, hwnd: HWND) -> Result<(), String> {
        let hotkeys = [
            // Focus movement (Win + Arrows)
            (MOD_WIN, VK_LEFT, 1, HotkeyAction::FocusLeft),
            (MOD_WIN, VK_RIGHT, 2, HotkeyAction::FocusRight),
            (MOD_WIN, VK_UP, 3, HotkeyAction::FocusUp),
            (MOD_WIN, VK_DOWN, 4, HotkeyAction::FocusDown),

            // Window movement (Win + Shift + Arrows)
            (MOD_WIN | MOD_SHIFT, VK_LEFT, 5, HotkeyAction::MoveLeft),
            (MOD_WIN | MOD_SHIFT, VK_RIGHT, 6, HotkeyAction::MoveRight),
            (MOD_WIN | MOD_SHIFT, VK_UP, 7, HotkeyAction::MoveUp),
            (MOD_WIN | MOD_SHIFT, VK_DOWN, 8, HotkeyAction::MoveDown),

            // Workspace switching (Win + 1-9)
            (MOD_WIN, VK_1, 10, HotkeyAction::SwitchWorkspace(1)),
            (MOD_WIN, VK_2, 11, HotkeyAction::SwitchWorkspace(2)),
            (MOD_WIN, VK_3, 12, HotkeyAction::SwitchWorkspace(3)),
            (MOD_WIN, VK_4, 13, HotkeyAction::SwitchWorkspace(4)),
            (MOD_WIN, VK_5, 14, HotkeyAction::SwitchWorkspace(5)),
            (MOD_WIN, VK_6, 15, HotkeyAction::SwitchWorkspace(6)),
            (MOD_WIN, VK_7, 16, HotkeyAction::SwitchWorkspace(7)),
            (MOD_WIN, VK_8, 17, HotkeyAction::SwitchWorkspace(8)),
            (MOD_WIN, VK_9, 18, HotkeyAction::SwitchWorkspace(9)),

            // Move to workspace (Win + Shift + 1-9)
            (MOD_WIN | MOD_SHIFT, VK_1, 19, HotkeyAction::MoveToWorkspace(1)),
            (MOD_WIN | MOD_SHIFT, VK_2, 20, HotkeyAction::MoveToWorkspace(2)),
            (MOD_WIN | MOD_SHIFT, VK_3, 21, HotkeyAction::MoveToWorkspace(3)),
            (MOD_WIN | MOD_SHIFT, VK_4, 22, HotkeyAction::MoveToWorkspace(4)),
            (MOD_WIN | MOD_SHIFT, VK_5, 23, HotkeyAction::MoveToWorkspace(5)),
            (MOD_WIN | MOD_SHIFT, VK_6, 24, HotkeyAction::MoveToWorkspace(6)),
            (MOD_WIN | MOD_SHIFT, VK_7, 25, HotkeyAction::MoveToWorkspace(7)),
            (MOD_WIN | MOD_SHIFT, VK_8, 26, HotkeyAction::MoveToWorkspace(8)),
            (MOD_WIN | MOD_SHIFT, VK_9, 27, HotkeyAction::MoveToWorkspace(9)),

            // Window operations
            (MOD_WIN, 0x57, 28, HotkeyAction::CloseWindow),      // W
            (MOD_WIN, 0x54, 29, HotkeyAction::ToggleTiling),     // T
            (MOD_WIN, 0x46, 30, HotkeyAction::ToggleFullscreen), // F
        ];

        for (modifiers, vk, id, action) in hotkeys {
            unsafe {
                if RegisterHotKey(hwnd, id, modifiers, vk.0 as u32).as_bool() {
                    self.registered_hotkeys.insert(id, action);
                    println!("Registered hotkey: {:?} (ID: {})", action, id);
                } else {
                    return Err(format!("Failed to register hotkey: {:?}", action));
                }
            }
        }

        Ok(())
    }

    pub fn get_action(&self, hotkey_id: i32) -> Option<HotkeyAction> {
        self.registered_hotkeys.get(&hotkey_id).copied()
    }

    pub fn unregister_all(&self, hwnd: HWND) {
        for id in self.registered_hotkeys.keys() {
            unsafe {
                UnregisterHotKey(hwnd, *id);
            }
        }
    }
}
```

### 3.2 Update Windows Dependencies

Ensure `Cargo.toml` has necessary features:

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

### 3.3 Integrate with Main Loop

Update `src/main.rs`:

```rust
mod windows;
mod tray;
mod hotkeys;

use std::sync::Arc;
use std::time::Duration;
use tray::TrayManager;
use hotkeys::HotkeyManager;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::*;

fn main() {
    println!("MegaTile - Window Manager");

    // Initialize tray icon
    let tray = TrayManager::new().expect("Failed to create tray icon");

    // Create hidden window for hotkey messages
    let hwnd = create_message_window().expect("Failed to create message window");

    // Register hotkeys
    let mut hotkey_manager = HotkeyManager::new();
    hotkey_manager.register_hotkeys(hwnd).expect("Failed to register hotkeys");

    println!("MegaTile is running. Use the tray icon to exit.");

    // Main event loop
    loop {
        if tray.should_exit() {
            println!("Exiting MegaTile...");
            hotkey_manager.unregister_all(hwnd);
            break;
        }

        // Process window messages
        let mut msg = MSG::default();
        while unsafe { PeekMessageW(&mut msg, HWND::default(), 0, 0, PM_REMOVE) }.as_bool() {
            unsafe {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);

                if msg.message == WM_HOTKEY {
                    let action = hotkey_manager.get_action(msg.wParam.0 as i32);
                    if let Some(action) = action {
                        println!("Hotkey pressed: {:?}", action);
                        // TODO: Handle hotkey action
                    }
                } else if msg.message == WM_DESTROY {
                    PostQuitMessage(0);
                }
            }
        }

        std::thread::sleep(Duration::from_millis(10));
    }
}

fn create_message_window() -> Result<HWND, String> {
    unsafe extern "system" fn window_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if msg == WM_DESTROY {
            PostQuitMessage(0);
        }
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }

    unsafe {
        let class_name = w!("MegaTileMessageWindow");

        let wc = WNDCLASSW {
            hInstance: GetModuleHandleW(None).unwrap(),
            lpfnWndProc: Some(window_proc),
            lpszClassName: class_name,
            ..Default::default()
        };

        if RegisterClassW(&wc) == 0 {
            return Err("Failed to register window class".to_string());
        }

        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class_name,
            w!("MegaTile"),
            WINDOW_STYLE::default(),
            0,
            0,
            0,
            0,
            HWND::default(),
            HMENU::default(),
            GetModuleHandleW(None).unwrap(),
            None,
        );

        if hwnd == HWND::default() {
            return Err("Failed to create window".to_string());
        }

        Ok(hwnd)
    }
}
```

## Testing

1. Run the application
2. Press various hotkey combinations:
   - Win + Arrow keys
   - Win + Shift + Arrow keys
   - Win + Number keys (1-9)
   - Win + W, Win + T, Win + F
3. Verify console output shows hotkey detection

## Success Criteria

- [ ] All 30 hotkeys register successfully
- [ ] Pressing hotkeys logs to console
- [ ] No hotkey registration errors
- [ ] Application remains responsive during hotkey handling
- [ ] Exit via tray still works

## Documentation

### Hotkey Registration

The `RegisterHotKey` API is used to register global hotkeys. Each hotkey has:
- Modifiers: MOD_WIN (0x8), MOD_SHIFT (0x4)
- Virtual key code: VK_LEFT (0x25) through VK_DOWN (0x28), VK_1-VK_9, etc.
- Unique ID: Used to identify the hotkey in WM_HOTKEY messages

### Message Window

A hidden message-only window is created to receive WM_HOTKEY messages. This window:
- Has no visual presence
- Processes window messages in the event loop
- Receives WM_HOTKEY when registered hotkeys are pressed

### Event Loop Processing

The event loop uses `PeekMessageW` to check for messages without blocking, allowing the tray check to run periodically. Messages are processed with:
- `TranslateMessage`: Translates virtual key messages
- `DispatchMessageW`: Dispatches to window procedure
- Custom handling for WM_HOTKEY and WM_DESTROY

## Next Steps

Proceed to [STEP_4.md](STEP_4.md) to implement workspace data structures and state management.
