use super::workspace::{Monitor, Window};
use std::sync::{Arc, Mutex};
use windows::Win32::Foundation::HWND;

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

    pub fn remove_window(&self, hwnd: HWND) -> Option<Window> {
        let mut monitors = self.monitors.lock().unwrap();
        for monitor in monitors.iter_mut() {
            if let Some(window) = monitor.remove_window(hwnd) {
                return Some(window);
            }
        }
        None
    }

    pub fn get_window(&self, hwnd: HWND) -> Option<Window> {
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
