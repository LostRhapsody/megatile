# STEP 4: Workspace Data Structures

## Objective

Define and implement the core data structures for workspace management, including Workspace and Monitor structs, and implement basic workspace switching logic.

## Tasks

### 4.1 Create Workspace Module

Create `src/workspace.rs`:

```rust
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::RECT;

#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub hwnd: HWND,
    pub title: String,
    pub class_name: String,
    pub rect: RECT,
    pub is_visible: bool,
    pub is_minimized: bool,
}

#[derive(Debug, Clone)]
pub struct Window {
    pub hwnd: HWND,
    pub workspace: u8,
    pub monitor: usize,
    pub rect: RECT,
    pub is_focused: bool,
    pub original_rect: RECT, // For restoring from fullscreen/hidden state
}

impl Window {
    pub fn new(hwnd: HWND, workspace: u8, monitor: usize, rect: RECT) -> Self {
        Window {
            hwnd,
            workspace,
            monitor,
            rect,
            is_focused: false,
            original_rect: rect,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Workspace {
    pub windows: Vec<Window>,
}

impl Workspace {
    pub fn new() -> Self {
        Workspace {
            windows: Vec::new(),
        }
    }

    pub fn add_window(&mut self, window: Window) {
        self.windows.push(window);
    }

    pub fn remove_window(&mut self, hwnd: HWND) -> Option<Window> {
        let pos = self.windows.iter().position(|w| w.hwnd == hwnd)?;
        Some(self.windows.remove(pos))
    }

    pub fn get_window(&self, hwnd: HWND) -> Option<&Window> {
        self.windows.iter().find(|w| w.hwnd == hwnd)
    }

    pub fn get_window_mut(&mut self, hwnd: HWND) -> Option<&mut Window> {
        self.windows.iter_mut().find(|w| w.hwnd == hwnd)
    }

    pub fn get_focused_window(&self) -> Option<&Window> {
        self.windows.iter().find(|w| w.is_focused)
    }

    pub fn set_focus(&mut self, hwnd: HWND, focused: bool) {
        if let Some(window) = self.get_window_mut(hwnd) {
            window.is_focused = focused;
        }
    }

    pub fn window_count(&self) -> usize {
        self.windows.len()
    }
}

#[derive(Debug, Clone)]
pub struct Monitor {
    pub hmonitor: isize, // HMONITOR
    pub rect: RECT,
    pub workspaces: [Workspace; 9],
    pub active_workspace: u8,
}

impl Monitor {
    pub fn new(hmonitor: isize, rect: RECT) -> Self {
        Monitor {
            hmonitor,
            rect,
            workspaces: [
                Workspace::new(),
                Workspace::new(),
                Workspace::new(),
                Workspace::new(),
                Workspace::new(),
                Workspace::new(),
                Workspace::new(),
                Workspace::new(),
                Workspace::new(),
            ],
            active_workspace: 1,
        }
    }

    pub fn get_active_workspace(&self) -> &Workspace {
        &self.workspaces[(self.active_workspace - 1) as usize]
    }

    pub fn get_active_workspace_mut(&mut self) -> &mut Workspace {
        &mut self.workspaces[(self.active_workspace - 1) as usize]
    }

    pub fn get_workspace(&self, workspace_num: u8) -> Option<&Workspace> {
        if workspace_num < 1 || workspace_num > 9 {
            return None;
        }
        Some(&self.workspaces[(workspace_num - 1) as usize])
    }

    pub fn get_workspace_mut(&mut self, workspace_num: u8) -> Option<&mut Workspace> {
        if workspace_num < 1 || workspace_num > 9 {
            return None;
        }
        Some(&mut self.workspaces[(workspace_num - 1) as usize])
    }

    pub fn set_active_workspace(&mut self, workspace_num: u8) -> bool {
        if workspace_num < 1 || workspace_num > 9 {
            return false;
        }
        self.active_workspace = workspace_num;
        true
    }

    pub fn add_window(&mut self, window: Window) {
        if let Some(workspace) = self.get_workspace_mut(window.workspace) {
            workspace.add_window(window);
        }
    }

    pub fn remove_window(&mut self, hwnd: HWND) -> Option<Window> {
        for workspace in &mut self.workspaces {
            if let Some(window) = workspace.remove_window(hwnd) {
                return Some(window);
            }
        }
        None
    }

    pub fn get_window(&self, hwnd: HWND) -> Option<&Window> {
        for workspace in &self.workspaces {
            if let Some(window) = workspace.get_window(hwnd) {
                return Some(window);
            }
        }
        None
    }
}
```

### 4.2 Create Workspace Manager

Create `src/workspace_manager.rs`:

```rust
use super::workspace::{Monitor, Window, Workspace};
use std::sync::{Arc, Mutex};

pub struct WorkspaceManager {
    monitors: Arc<Mutex<Vec<Monitor>>>,
    active_workspace_global: u8, // All monitors share the same active workspace
}

impl WorkspaceManager {
    pub fn new() -> Self {
        WorkspaceManager {
            monitors: Arc::new(Mutex::new(Vec::new())),
            active_workspace_global: 1,
        }
    }

    pub fn set_monitors(&self, monitors: Vec<Monitor>) {
        let mut m = self.monitors.lock().unwrap();
        *m = monitors;
    }

    pub fn get_active_workspace(&self) -> u8 {
        self.active_workspace_global
    }

    pub fn switch_workspace(&mut self, workspace_num: u8) -> bool {
        if workspace_num < 1 || workspace_num > 9 {
            return false;
        }

        self.active_workspace_global = workspace_num;

        // Update all monitors
        let mut monitors = self.monitors.lock().unwrap();
        for monitor in monitors.iter_mut() {
            monitor.set_active_workspace(workspace_num);
        }

        true
    }

    pub fn add_window(&self, window: Window) {
        let mut monitors = self.monitors.lock().unwrap();
        if let Some(monitor) = monitors.get_mut(window.monitor) {
            monitor.add_window(window);
        }
    }

    pub fn remove_window(&self, hwnd: isize) -> Option<Window> {
        let mut monitors = self.monitors.lock().unwrap();
        for monitor in monitors.iter_mut() {
            if let Some(window) = monitor.remove_window(hwnd) {
                return Some(window);
            }
        }
        None
    }

    pub fn get_window(&self, hwnd: isize) -> Option<Window> {
        let monitors = self.monitors.lock().unwrap();
        for monitor in monitors.iter() {
            if let Some(window) = monitor.get_window(hwnd) {
                return Some(window.clone());
            }
        }
        None
    }

    pub fn get_active_workspace_windows(&self, monitor_index: usize) -> Vec<Window> {
        let monitors = self.monitors.lock().unwrap();
        if let Some(monitor) = monitors.get(monitor_index) {
            monitor.get_active_workspace().windows.clone()
        } else {
            Vec::new()
        }
    }

    pub fn get_monitors(&self) -> Vec<Monitor> {
        let monitors = self.monitors.lock().unwrap();
        monitors.clone()
    }
}

impl Default for WorkspaceManager {
    fn default() -> Self {
        Self::new()
    }
}
```

### 4.3 Add Monitor Enumeration

Update `src/windows.rs` to add monitor enumeration:

```rust
use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MONITORINFO};

pub struct MonitorInfo {
    pub hmonitor: isize,
    pub rect: RECT,
    pub is_primary: bool,
}

pub fn enumerate_monitors() -> Vec<MonitorInfo> {
    let mut monitors = Vec::new();

    unsafe extern "system" fn enum_monitors_proc(
        hmonitor: HMONITOR,
        _hdc: HDC,
        _lprect: *mut RECT,
        lparam: LPARAM,
    ) -> BOOL {
        let monitors = &mut *(lparam.0 as *mut Vec<MonitorInfo>);

        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };

        if GetMonitorInfoW(hmonitor, &mut info).as_bool() {
            monitors.push(MonitorInfo {
                hmonitor: hmonitor.0,
                rect: info.rcMonitor,
                is_primary: info.dwFlags & MONITORINFOF_PRIMARY != 0,
            });
        }

        TRUE
    }

    unsafe {
        EnumDisplayMonitors(
            HDC::default(),
            None,
            Some(enum_monitors_proc),
            LPARAM(&mut monitors as *mut _ as isize),
        );
    }

    monitors
}
```

### 4.4 Update Main Entry Point

Update `src/main.rs` to integrate workspace manager:

```rust
mod windows;
mod tray;
mod hotkeys;
mod workspace;
mod workspace_manager;

use std::sync::Arc;
use std::time::Duration;
use tray::TrayManager;
use hotkeys::HotkeyManager;
use workspace_manager::WorkspaceManager;
use windows::{enumerate_monitors, get_normal_windows};

// ... keep existing hotkey code ...

fn main() {
    println!("MegaTile - Window Manager");

    // Initialize workspace manager
    let workspace_manager = Arc::new(Mutex::new(WorkspaceManager::new()));

    // Enumerate monitors and create monitor structs
    let monitor_infos = enumerate_monitors();
    println!("Found {} monitor(s):", monitor_infos.len());

    let monitors: Vec<workspace::Monitor> = monitor_infos
        .iter()
        .enumerate()
        .map(|(i, info)| {
            println!("  Monitor {}: {:?}", i + 1, info.rect);
            workspace::Monitor::new(info.hmonitor, info.rect)
        })
        .collect();

    workspace_manager.lock().unwrap().set_monitors(monitors);

    // Enumerate windows
    let normal_windows = get_normal_windows();
    println!("Found {} normal windows", normal_windows.len());

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
                        handle_hotkey(action, &workspace_manager);
                    }
                } else if msg.message == WM_DESTROY {
                    PostQuitMessage(0);
                }
            }
        }

        std::thread::sleep(Duration::from_millis(10));
    }
}

fn handle_hotkey(action: hotkeys::HotkeyAction, workspace_manager: &Arc<Mutex<WorkspaceManager>>) {
    match action {
        hotkeys::HotkeyAction::SwitchWorkspace(num) => {
            let mut wm = workspace_manager.lock().unwrap();
            if wm.switch_workspace(num) {
                println!("Switched to workspace {}", num);
            }
        }
        _ => {
            println!("Hotkey action: {:?}", action);
            // TODO: Implement other actions in future steps
        }
    }
}
```

## Testing

1. Run the application
2. Verify monitor enumeration (should detect your monitor(s))
3. Verify workspace switching (Win + 1-9 keys)
4. Check console output for workspace changes

## Success Criteria

- [ ] Workspace data structures compile and work correctly
- [ ] Monitor enumeration detects all monitors
- [ ] Workspace switching (Win + 1-9) updates active workspace
- [ ] Multiple monitors are tracked independently
- [ ] No errors in console output

## Documentation

### Data Structure Design

**Window**: Represents a managed window with:
- HWND for window handle
- Workspace assignment (1-9)
- Monitor index
- Current rectangle
- Focus state
- Original rectangle (for restoration)

**Workspace**: Contains a list of windows for a single workspace on a single monitor.

**Monitor**: Represents a physical monitor with:
- HMONITOR handle
- Screen rectangle
- Array of 9 workspaces
- Currently active workspace

**WorkspaceManager**: Manages all monitors and the global active workspace state.

### Shared Workspace Model

All monitors share the same active workspace number. Switching to workspace 2 shows workspace 2 on all monitors, but each monitor maintains independent window lists for each workspace.

## Next Steps

Proceed to [STEP_5.md](STEP_5.md) to implement window hiding/showing for workspace switching.
