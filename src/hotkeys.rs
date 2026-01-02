use std::collections::HashMap;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::*;

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
            // Focus movement (Alt + Arrows)
            (MOD_ALT, VK_LEFT, 1, HotkeyAction::FocusLeft),
            (MOD_ALT, VK_RIGHT, 2, HotkeyAction::FocusRight),
            (MOD_ALT, VK_UP, 3, HotkeyAction::FocusUp),
            (MOD_ALT, VK_DOWN, 4, HotkeyAction::FocusDown),
            // Window movement (Alt + Shift + Arrows)
            (MOD_ALT | MOD_SHIFT, VK_LEFT, 5, HotkeyAction::MoveLeft),
            (MOD_ALT | MOD_SHIFT, VK_RIGHT, 6, HotkeyAction::MoveRight),
            (MOD_ALT | MOD_SHIFT, VK_UP, 7, HotkeyAction::MoveUp),
            (MOD_ALT | MOD_SHIFT, VK_DOWN, 8, HotkeyAction::MoveDown),
            // Workspace switching (Alt + 1-9)
            (MOD_ALT, VK_1, 10, HotkeyAction::SwitchWorkspace(1)),
            (MOD_ALT, VK_2, 11, HotkeyAction::SwitchWorkspace(2)),
            (MOD_ALT, VK_3, 12, HotkeyAction::SwitchWorkspace(3)),
            (MOD_ALT, VK_4, 13, HotkeyAction::SwitchWorkspace(4)),
            (MOD_ALT, VK_5, 14, HotkeyAction::SwitchWorkspace(5)),
            (MOD_ALT, VK_6, 15, HotkeyAction::SwitchWorkspace(6)),
            (MOD_ALT, VK_7, 16, HotkeyAction::SwitchWorkspace(7)),
            (MOD_ALT, VK_8, 17, HotkeyAction::SwitchWorkspace(8)),
            (MOD_ALT, VK_9, 18, HotkeyAction::SwitchWorkspace(9)),
            // Move to workspace (Alt + Shift + 1-9)
            (
                MOD_ALT | MOD_SHIFT,
                VK_1,
                19,
                HotkeyAction::MoveToWorkspace(1),
            ),
            (
                MOD_ALT | MOD_SHIFT,
                VK_2,
                20,
                HotkeyAction::MoveToWorkspace(2),
            ),
            (
                MOD_ALT | MOD_SHIFT,
                VK_3,
                21,
                HotkeyAction::MoveToWorkspace(3),
            ),
            (
                MOD_ALT | MOD_SHIFT,
                VK_4,
                22,
                HotkeyAction::MoveToWorkspace(4),
            ),
            (
                MOD_ALT | MOD_SHIFT,
                VK_5,
                23,
                HotkeyAction::MoveToWorkspace(5),
            ),
            (
                MOD_ALT | MOD_SHIFT,
                VK_6,
                24,
                HotkeyAction::MoveToWorkspace(6),
            ),
            (
                MOD_ALT | MOD_SHIFT,
                VK_7,
                25,
                HotkeyAction::MoveToWorkspace(7),
            ),
            (
                MOD_ALT | MOD_SHIFT,
                VK_8,
                26,
                HotkeyAction::MoveToWorkspace(8),
            ),
            (
                MOD_ALT | MOD_SHIFT,
                VK_9,
                27,
                HotkeyAction::MoveToWorkspace(9),
            ),
            // Window operations
            (MOD_ALT, VIRTUAL_KEY(0x57), 28, HotkeyAction::CloseWindow), // W
            (MOD_ALT, VIRTUAL_KEY(0x54), 29, HotkeyAction::ToggleTiling), // T
            (
                MOD_ALT,
                VIRTUAL_KEY(0x46),
                30,
                HotkeyAction::ToggleFullscreen,
            ), // F
        ];

        for (modifiers, vk, id, action) in hotkeys {
            unsafe {
                println!("Registering hotkey: vk={}, id={}", vk.0, id);
                match RegisterHotKey(Some(hwnd), id, modifiers, vk.0 as u32) {
                    Ok(()) => {
                        self.registered_hotkeys.insert(id, action);
                        println!("Registered hotkey: {:?} (ID: {})", action, id);
                    }
                    Err(e) => {
                        return Err(format!(
                            "Failed to register hotkey: {:?} (vk={}, id={}, error={:?})",
                            action, vk.0, id, e
                        ));
                    }
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
                let _ = UnregisterHotKey(Some(hwnd), *id);
            }
        }
    }
}
