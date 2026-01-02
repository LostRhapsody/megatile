use super::workspace::{Monitor, Window};
use crate::windows_lib::{hide_window_from_taskbar, show_window_in_taskbar};
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

    pub fn switch_workspace_with_windows(&mut self, new_workspace: u8) -> Result<(), String> {
        if new_workspace < 1 || new_workspace > 9 {
            return Err("Invalid workspace number".to_string());
        }

        let old_workspace = self.active_workspace_global;

        if old_workspace == new_workspace {
            return Ok(()); // No change needed
        }

        println!(
            "Switching from workspace {} to {}",
            old_workspace, new_workspace
        );

        // Hide windows from old workspace
        self.hide_workspace_windows(old_workspace)?;

        // Show windows from new workspace
        self.show_workspace_windows(new_workspace)?;

        // Update active workspace
        self.active_workspace_global = new_workspace;

        // Update all monitors
        let mut monitors = self.monitors.lock().unwrap();
        for monitor in monitors.iter_mut() {
            monitor.set_active_workspace(new_workspace);
        }

        Ok(())
    }

    fn hide_workspace_windows(&self, workspace_num: u8) -> Result<(), String> {
        let monitors = self.monitors.lock().unwrap();

        for monitor in monitors.iter() {
            if let Some(workspace) = monitor.get_workspace(workspace_num) {
                for window in &workspace.windows {
                    if let Err(e) = hide_window_from_taskbar(window.hwnd) {
                        eprintln!("Failed to hide window {:?}: {}", window.hwnd, e);
                    }
                }
            }
        }

        Ok(())
    }

    fn show_workspace_windows(&self, workspace_num: u8) -> Result<(), String> {
        let monitors = self.monitors.lock().unwrap();

        for monitor in monitors.iter() {
            if let Some(workspace) = monitor.get_workspace(workspace_num) {
                for window in &workspace.windows {
                    if let Err(e) = show_window_in_taskbar(window.hwnd) {
                        eprintln!("Failed to show window {:?}: {}", window.hwnd, e);
                    }
                }
            }
        }

        Ok(())
    }

    pub fn move_window_to_workspace(
        &mut self,
        hwnd: HWND,
        new_workspace: u8,
    ) -> Result<(), String> {
        if new_workspace < 1 || new_workspace > 9 {
            return Err("Invalid workspace number".to_string());
        }

        let mut monitors = self.monitors.lock().unwrap();

        // Find and remove window from its current workspace
        let mut window_to_move = None;
        let mut current_workspace = 0;
        let mut monitor_index = 0;

        for (m_idx, monitor) in monitors.iter_mut().enumerate() {
            for w_idx in 0..9 {
                if let Some(workspace) = monitor.get_workspace_mut((w_idx + 1) as u8) {
                    if let Some(window) = workspace.remove_window(hwnd) {
                        window_to_move = Some(window);
                        current_workspace = w_idx + 1;
                        monitor_index = m_idx;
                        break;
                    }
                }
            }
            if window_to_move.is_some() {
                break;
            }
        }

        if let Some(mut window) = window_to_move {
            // Update window's workspace
            window.workspace = new_workspace;

            // Add to new workspace
            if let Some(monitor) = monitors.get_mut(monitor_index) {
                if let Some(workspace) = monitor.get_workspace_mut(new_workspace) {
                    workspace.add_window(window);
                }
            }

            // If moving from active workspace, hide it
            if current_workspace == self.active_workspace_global {
                hide_window_from_taskbar(hwnd).ok();
            }
            // If moving to active workspace, show it
            else if new_workspace == self.active_workspace_global {
                show_window_in_taskbar(hwnd).ok();
            }

            Ok(())
        } else {
            Err("Window not found".to_string())
        }
    }
}

impl Default for WorkspaceManager {
    fn default() -> Self {
        Self::new()
    }
}
