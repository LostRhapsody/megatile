//! System tray icon integration.
//!
//! Provides a system tray icon with an exit menu option for graceful shutdown.

use std::sync::atomic::{AtomicBool, Ordering};
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder,
    menu::{Menu, MenuEvent, MenuItem},
};

/// Global flag indicating the application should exit.
pub static SHOULD_EXIT: AtomicBool = AtomicBool::new(false);

/// Creates a simple orange 32x32 icon for the system tray.
pub fn create_default_icon() -> Result<Icon, Box<dyn std::error::Error>> {
    let width = 32;
    let height = 32;
    let mut icon_data = Vec::with_capacity(width * height * 4);

    for _ in 0..(width * height) {
        icon_data.push(255);
        icon_data.push(165);
        icon_data.push(0);
        icon_data.push(255);
    }

    Icon::from_rgba(icon_data, width as u32, height as u32)
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
}

/// Manages the system tray icon and menu.
pub struct TrayManager {
    /// The tray icon (kept alive for the duration of the program).
    _icon: TrayIcon,
}

impl TrayManager {
    /// Creates a new tray manager with an icon and exit menu.
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let exit_menu_item = MenuItem::with_id("exit", "Exit", true, None);
        let menu = Menu::new();
        menu.append_items(&[&exit_menu_item])?;

        let tray_icon = create_default_icon()?;
        let icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("MegaTile - Tiling Window Manager")
            .with_icon(tray_icon)
            .build()
            .unwrap();

        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            if event.id.0.as_str() == "exit" {
                SHOULD_EXIT.store(true, Ordering::SeqCst);
            }
        }));

        Ok(TrayManager { _icon: icon })
    }

    /// Returns true if the exit menu item was clicked.
    pub fn should_exit(&self) -> bool {
        SHOULD_EXIT.load(Ordering::SeqCst)
    }
}
