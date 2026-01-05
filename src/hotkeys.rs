//! Global hotkey registration and action mapping.
//!
//! This module handles registering system-wide hotkeys with Windows
//! and mapping them to [`HotkeyAction`] values for the window manager.

use std::collections::HashMap;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::*;

/// Manages global hotkey registration and lookup.
pub struct HotkeyManager {
    registered_hotkeys: HashMap<i32, HotkeyAction>,
}

/// Actions that can be triggered by hotkeys.
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

    // Window resizing
    ResizeHorizontalIncrease,
    ResizeHorizontalDecrease,
    ResizeVerticalIncrease,
    ResizeVerticalDecrease,

    // Layout operations
    FlipRegion,

    // Workspace switching
    SwitchWorkspace(u8),
    MoveToWorkspace(u8),

    // Window operations
    CloseWindow,
    ToggleTiling,
    ToggleFullscreen,
    ToggleStatusBar,

    // Monitor movement
    MoveToMonitorLeft,
    MoveToMonitorRight,
    MoveToMonitorUp,
    MoveToMonitorDown,
}

impl HotkeyManager {
    /// Creates a new hotkey manager.
    pub fn new() -> Self {
        Self {
            registered_hotkeys: HashMap::new(),
        }
    }

    /// Registers all hotkeys with Windows.
    ///
    /// # Hotkey Bindings
    /// - `Alt + Arrows`: Move focus
    /// - `Alt + Shift + Arrows`: Move window
    /// - `Alt + Ctrl + Arrows`: Move window to adjacent monitor
    /// - `Alt + 1-9`: Switch workspace
    /// - `Alt + Shift + 1-9`: Move window to workspace and follow
    /// - `Alt + +/-`: Resize horizontally
    /// - `Alt + Shift + +/-`: Resize vertically
    /// - `Alt + J`: Flip region
    /// - `Alt + W`: Close window
    /// - `Alt + T`: Toggle tiling
    /// - `Alt + F`: Toggle fullscreen
    /// - `Alt + B`: Toggle status bar
    pub fn register_hotkeys(&mut self, hwnd: HWND) -> Result<(), String> {
        // Virtual key codes for number keys 1-9
        const VK_NUMS: [VIRTUAL_KEY; 9] = [VK_1, VK_2, VK_3, VK_4, VK_5, VK_6, VK_7, VK_8, VK_9];

        let mut hotkeys: Vec<(HOT_KEY_MODIFIERS, VIRTUAL_KEY, i32, HotkeyAction)> = vec![
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
            // Window resizing
            (
                MOD_ALT,
                VIRTUAL_KEY(0xBB),
                28,
                HotkeyAction::ResizeHorizontalIncrease,
            ),
            (
                MOD_ALT,
                VIRTUAL_KEY(0xBD),
                29,
                HotkeyAction::ResizeHorizontalDecrease,
            ),
            (
                MOD_ALT | MOD_SHIFT,
                VIRTUAL_KEY(0xBB),
                30,
                HotkeyAction::ResizeVerticalIncrease,
            ),
            (
                MOD_ALT | MOD_SHIFT,
                VIRTUAL_KEY(0xBD),
                31,
                HotkeyAction::ResizeVerticalDecrease,
            ),
            // Layout and window operations
            (MOD_ALT, VIRTUAL_KEY(0x4A), 32, HotkeyAction::FlipRegion),
            (MOD_ALT, VIRTUAL_KEY(0x57), 33, HotkeyAction::CloseWindow),
            (MOD_ALT, VIRTUAL_KEY(0x54), 34, HotkeyAction::ToggleTiling),
            (
                MOD_ALT,
                VIRTUAL_KEY(0x46),
                35,
                HotkeyAction::ToggleFullscreen,
            ),
            (
                MOD_ALT,
                VIRTUAL_KEY(0x42),
                45,
                HotkeyAction::ToggleStatusBar,
            ),
            // Monitor movement (Alt + Ctrl + Arrows)
            (
                MOD_ALT | MOD_CONTROL,
                VK_LEFT,
                50,
                HotkeyAction::MoveToMonitorLeft,
            ),
            (
                MOD_ALT | MOD_CONTROL,
                VK_RIGHT,
                51,
                HotkeyAction::MoveToMonitorRight,
            ),
            (
                MOD_ALT | MOD_CONTROL,
                VK_UP,
                52,
                HotkeyAction::MoveToMonitorUp,
            ),
            (
                MOD_ALT | MOD_CONTROL,
                VK_DOWN,
                53,
                HotkeyAction::MoveToMonitorDown,
            ),
        ];

        // Add workspace hotkeys (1-9) using iteration
        for (i, &vk) in VK_NUMS.iter().enumerate() {
            let ws = (i + 1) as u8;
            hotkeys.push((
                MOD_ALT,
                vk,
                10 + i as i32,
                HotkeyAction::SwitchWorkspace(ws),
            ));
            hotkeys.push((
                MOD_ALT | MOD_SHIFT,
                vk,
                19 + i as i32,
                HotkeyAction::MoveToWorkspace(ws),
            ));
        }

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

    /// Returns the action associated with a hotkey ID.
    pub fn get_action(&self, hotkey_id: i32) -> Option<HotkeyAction> {
        self.registered_hotkeys.get(&hotkey_id).copied()
    }

    /// Unregisters all hotkeys.
    pub fn unregister_all(&self, hwnd: HWND) {
        for id in self.registered_hotkeys.keys() {
            unsafe {
                let _ = UnregisterHotKey(Some(hwnd), *id);
            }
        }
    }
}
