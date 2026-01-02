# STEP 1: Project Scaffolding and Window Enumeration

## Objective

Set up the Rust project structure and implement basic window enumeration to identify and filter normal windows from tool windows, dialogs, and system UI elements.

## Tasks

### 1.1 Initialize Rust Project

- Create new Cargo project: `cargo init`
- Add dependencies to `Cargo.toml`:
  ```toml
  [dependencies]
  windows = { version = "0.52", features = [
      "Win32_Foundation",
      "Win32_UI_WindowsAndMessaging",
      "Win32_Graphics_Gdi",
      "Win32_System_LibraryLoader",
  ]}
  ```

### 1.2 Implement Window Enumeration

Create `src/windows.rs`:

```rust
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::Win32::Graphics::Gdi::*;

pub struct WindowInfo {
    pub hwnd: HWND,
    pub title: String,
    pub class_name: String,
    pub rect: RECT,
    pub is_visible: bool,
    pub is_minimized: bool,
}

pub fn enumerate_windows() -> Vec<WindowInfo> {
    let mut windows = Vec::new();

    unsafe {
        EnumWindows(Some(enum_windows_proc), &mut windows as *mut _ as isize);
    }

    windows
}

unsafe extern "system" fn enum_windows_proc(hwnd: HWND, lparam: isize) -> BOOL {
    let windows = &mut *(lparam as *mut Vec<WindowInfo>);

    // Get window title
    let mut title_buffer = [0u16; 256];
    let length = GetWindowTextW(hwnd, &mut title_buffer);
    let title = String::from_utf16_lossy(&title_buffer[..length as usize]);

    // Get class name
    let mut class_buffer = [0u16; 256];
    GetClassNameW(hwnd, &mut class_buffer);
    let class_name = String::from_utf16_lossy(&class_buffer);

    // Get window rect
    let mut rect = RECT::default();
    GetWindowRect(hwnd, &mut rect);

    // Check visibility
    let is_visible = IsWindowVisible(hwnd).as_bool();

    // Check if minimized
    let is_minimized = IsIconic(hwnd).as_bool();

    windows.push(WindowInfo {
        hwnd,
        title,
        class_name,
        rect,
        is_visible,
        is_minimized,
    });

    TRUE
}
```

### 1.3 Implement Window Filtering

Add filtering logic to `src/windows.rs`:

```rust
pub fn is_normal_window(hwnd: HWND, class_name: &str, title: &str) -> bool {
    unsafe {
        // Check if window is visible
        if !IsWindowVisible(hwnd).as_bool() {
            return false;
        }

        // Check if minimized
        if IsIconic(hwnd).as_bool() {
            return false;
        }

        // Get extended window styles
        let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;

        // Exclude tool windows
        if ex_style & WS_EX_TOOLWINDOW.0 != 0 {
            return false;
        }

        // Exclude windows with WS_EX_NOACTIVATE
        if ex_style & WS_EX_NOACTIVATE.0 != 0 {
            return false;
        }

        // Exclude system windows by class name
        let system_classes = [
            "Shell_TrayWnd",         // Taskbar
            "Shell_SecondaryTrayWnd", // Secondary taskbar
            "Shell_traywnd",         // Taskbar (case variant)
            "WorkerW",               // Desktop worker window
            "Progman",               // Program Manager (desktop)
            "DV2ControlHost",        // Windows 11 taskbar
            "XamlExplorerHostIslandWindow", // Windows 11 UI
        ];

        for sys_class in &system_classes {
            if class_name.eq_ignore_ascii_case(sys_class) {
                return false;
            }
        }

        // Include only windows with WS_EX_APPWINDOW or reasonable title
        if ex_style & WS_EX_APPWINDOW.0 != 0 {
            return true;
        }

        // Include windows with titles (heuristics)
        if !title.trim().is_empty() {
            return true;
        }

        false
    }
}

pub fn get_normal_windows() -> Vec<WindowInfo> {
    enumerate_windows()
        .into_iter()
        .filter(|w| is_normal_window(w.hwnd, &w.class_name, &w.title))
        .collect()
}
```

### 1.4 Create Main Entry Point

Update `src/main.rs`:

```rust
mod windows;

fn main() {
    println!("MegaTile - Window Manager");

    let normal_windows = windows::get_normal_windows();

    println!("\nFound {} normal windows:", normal_windows.len());

    for (i, window) in normal_windows.iter().enumerate() {
        println!(
            "{}. '{}' (Class: {}, HWND: {:?}, Visible: {}, Minimized: {})",
            i + 1,
            window.title,
            window.class_name,
            window.hwnd,
            window.is_visible,
            window.is_minimized
        );
    }

    println!("\nPress Enter to exit...");
    let _ = std::io::stdin().read_line(&mut String::new());
}
```

## Testing

1. Run the application: `cargo run`
2. Open several applications (Notepad, Calculator, File Explorer, etc.)
3. Observe the output:
   - Verify that normal application windows are detected
   - Verify that system windows (taskbar, desktop) are excluded
   - Verify that invisible/minimized windows are excluded

## Success Criteria

- [ ] Cargo project initializes successfully
- [ ] Dependencies compile without errors
- [ ] Application enumerates all top-level windows
- [ ] Filtering correctly identifies normal windows
- [ ] System windows (taskbar, desktop) are excluded
- [ ] Output shows expected windows with titles and class names

## Documentation

### Window Filtering Approach

The filtering uses a multi-pass approach:

1. **Visibility check**: `IsWindowVisible()` - excludes hidden windows
2. **Minimized check**: `IsIconic()` - excludes minimized windows
3. **Extended style check**: Excludes windows with `WS_EX_TOOLWINDOW` or `WS_EX_NOACTIVATE`
4. **Class name blacklist**: Excludes known system window classes
5. **Inclusion heuristics**: Includes windows with `WS_EX_APPWINDOW` or non-empty titles

This ensures only user-facing application windows are managed by the tiling system.

### Windows API Functions Used

- `EnumWindows`: Enumerate all top-level windows
- `GetWindowTextW`: Get window title
- `GetClassNameW`: Get window class name
- `GetWindowRect`: Get window dimensions
- `IsWindowVisible`: Check visibility
- `IsIconic`: Check if minimized
- `GetWindowLongW`: Get extended window styles

## Next Steps

Proceed to [STEP_2.md](STEP_2.md) to implement system tray integration.
