#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod tray;
mod windows_lib;

use tray::TrayManager;
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, MSG, TranslateMessage,
};

fn main() {
    println!("MegaTile - Window Manager");

    let tray = TrayManager::new().expect("Failed to create tray icon");

    println!("MegaTile is running. Use the tray icon to exit.");

    let mut msg = MSG::default();
    unsafe {
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
            if tray.should_exit() {
                break;
            }
        }
    }
    println!("Exiting MegaTile...");
}
