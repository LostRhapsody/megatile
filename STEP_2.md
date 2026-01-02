# STEP 2: System Tray Integration

## Objective

Add a system tray icon with a basic menu (Exit option) to MegaTile, providing a way to gracefully shutdown the application.

## Tasks

### 2.1 Add Tray Icon Dependency

Update `Cargo.toml`:

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

### 2.2 Create Tray Module

Create `src/tray.rs`:

```rust
use std::sync::atomic::{AtomicBool, Ordering};
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem},
    TrayIcon, TrayIconBuilder,
};

static SHOULD_EXIT: AtomicBool = AtomicBool::new(false);

pub struct TrayManager {
    _icon: TrayIcon,
}

impl TrayManager {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // Create menu
        let exit_menu_item = MenuItem::new("Exit", true, None);
        let menu = Menu::new();
        menu.append(&exit_menu_item)?;

        // Create tray icon
        let icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("MegaTile - Tiling Window Manager")
            .build()?;

        // Handle menu events
        let menu_channel = MenuEvent::receiver();
        std::thread::spawn(move || {
            while let Ok(event) = menu_channel.recv() {
                if event.id == exit_menu_item.id() {
                    SHOULD_EXIT.store(true, Ordering::SeqCst);
                }
            }
        });

        Ok(TrayManager { _icon: icon })
    }

    pub fn should_exit(&self) -> bool {
        SHOULD_EXIT.load(Ordering::SeqCst)
    }
}
```

### 2.3 Update Main Entry Point

Update `src/main.rs`:

```rust
mod windows;
mod tray;

use std::time::Duration;
use tray::TrayManager;

fn main() {
    println!("MegaTile - Window Manager");

    // Initialize tray icon
    let tray = TrayManager::new().expect("Failed to create tray icon");

    println!("MegaTile is running. Use the tray icon to exit.");

    // Main event loop
    loop {
        if tray.should_exit() {
            println!("Exiting MegaTile...");
            break;
        }

        std::thread::sleep(Duration::from_millis(100));
    }
}
```

### 2.4 Test Tray Functionality

Run the application and verify:

1. System tray icon appears
2. Right-click shows "Exit" menu
3. Clicking "Exit" terminates the application cleanly
4. No errors in console output

## Success Criteria

- [ ] Tray icon appears in system tray
- [ ] Right-click shows menu with "Exit" option
- [ ] Clicking "Exit" terminates application
- [ ] Application runs without blocking the UI
- [ ] No console errors

## Documentation

### System Tray Implementation

The system tray uses the `tray-icon` crate which provides cross-platform tray icon support. The tray icon is created with a simple menu containing an "Exit" option.

The event loop runs at 100ms intervals, checking the `SHOULD_EXIT` atomic flag set by the menu event handler. This provides a clean shutdown mechanism.

### Tray Icon Resources

Currently using the default tray icon. In a future step, we can add a custom icon file to `resources/tray.ico` and load it:

```rust
let icon_bytes = include_bytes!("../resources/tray.ico");
let icon = tray_icon::Icon::from_buffer(icon_bytes, None, None)?;
```

### Event Loop

The event loop uses a simple polling approach. In Step 3, we'll integrate hotkey handling into this loop.

## Next Steps

Proceed to [STEP_3.md](STEP_3.md) to implement global hotkey registration.
