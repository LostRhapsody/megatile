use super::workspace::{Monitor, Window};
use crate::tiling::DwindleTiler;
use crate::windows_lib::{hide_window_from_taskbar, show_window_in_taskbar};
use std::sync::{Arc, Mutex};
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::UI::WindowsAndMessaging::{SetWindowPos, SWP_NOACTIVATE, SWP_NOZORDER};

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

    pub fn get_all_managed_hwnds(&self) -> Vec<isize> {
        let mut hwnds = Vec::new();
        let monitors = self.monitors.lock().unwrap();
        for monitor in monitors.iter() {
            for workspace in &monitor.workspaces {
                for window in &workspace.windows {
                    hwnds.push(window.hwnd);
                }
            }
        }
        hwnds
    }

    pub fn get_monitor_for_window(&self, hwnd: HWND) -> Option<usize> {
        use windows::Win32::UI::WindowsAndMessaging::GetWindowRect;

        let mut rect = RECT::default();
        unsafe {
            if GetWindowRect(hwnd, &mut rect).is_err() {
                return None;
            }
        }

        let monitors = self.monitors.lock().unwrap();
        for (i, monitor) in monitors.iter().enumerate() {
            if rect.left >= monitor.rect.left
                && rect.top >= monitor.rect.top
                && rect.right <= monitor.rect.right
                && rect.bottom <= monitor.rect.bottom
            {
                return Some(i);
            }
        }
        None
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
                    if let Err(e) = hide_window_from_taskbar(HWND(window.hwnd as _)) {
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
                    if let Err(e) = show_window_in_taskbar(HWND(window.hwnd as _)) {
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

    pub fn tile_active_workspaces(&self) {
        let tiler = DwindleTiler::default();
        let mut monitors = self.monitors.lock().unwrap();

        for monitor in monitors.iter_mut() {
            let workspace_idx = (monitor.active_workspace - 1) as usize;

            if !monitor.workspaces[workspace_idx].windows.is_empty() {
                // Create a copy of the monitor for reading
                let monitor_copy = monitor.clone();
                let windows = &mut monitor.workspaces[workspace_idx].windows;
                tiler.tile_windows(&monitor_copy, windows);
            }
        }
    }

    pub fn apply_window_positions(&self) {
        let monitors = self.monitors.lock().unwrap();

        for monitor in monitors.iter() {
            let active_workspace = monitor.get_active_workspace();

            for window in &active_workspace.windows {
                self.set_window_position(HWND(window.hwnd as _), &window.rect);
            }
        }
    }

    fn set_window_position(&self, hwnd: HWND, rect: &RECT) {
        unsafe {
            SetWindowPos(
                hwnd,
                None,
                rect.left,
                rect.top,
                rect.right - rect.left,
                rect.bottom - rect.top,
                SWP_NOZORDER | SWP_NOACTIVATE,
            )
            .ok();
        }
    }

    pub fn get_focused_window(&self) -> Option<Window> {
        use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

        unsafe {
            let hwnd = GetForegroundWindow();
            self.get_window(hwnd)
        }
    }

    pub fn move_focus(&self, direction: FocusDirection) -> Result<(), String> {
        use windows::Win32::UI::WindowsAndMessaging::*;

        let focused = self.get_focused_window();

        // Find all windows in active workspace on all monitors
        let mut active_windows: Vec<(Window, RECT)> = Vec::new();
        let monitors = self.monitors.lock().unwrap();

        for monitor in monitors.iter() {
            let active_workspace = monitor.get_active_workspace();
            for window in &active_workspace.windows {
                active_windows.push((window.clone(), window.rect));
            }
        }

        if active_windows.is_empty() {
            return Ok(()); // No windows to focus
        }

        let target = if let Some(focused) = focused {
            // Find window to move focus to based on direction
            self.find_next_focus(&focused, direction, &active_windows)
        } else {
            // No window focused, focus the first window
            active_windows.first().map(|(w, _)| w.clone())
        };

        if let Some(target_window) = target {
            self.set_window_focus(HWND(target_window.hwnd as _));
        }

        Ok(())
    }

    fn find_next_focus(
        &self,
        focused: &Window,
        direction: FocusDirection,
        windows: &[(Window, RECT)],
    ) -> Option<Window> {
        let focused_rect = focused.rect;

        let candidates: Vec<&(Window, RECT)> = windows
            .iter()
            .filter(|(w, _)| w.hwnd != focused.hwnd)
            .collect();

        if candidates.is_empty() {
            return None;
        }

        // Find the best candidate based on direction
        match direction {
            FocusDirection::Left => candidates
                .iter()
                .filter(|(_, rect)| rect.right < focused_rect.left)
                .min_by_key(|(_, rect)| focused_rect.left - rect.right)
                .map(|(w, _)| w.clone()),
            FocusDirection::Right => candidates
                .iter()
                .filter(|(_, rect)| rect.left > focused_rect.right)
                .min_by_key(|(_, rect)| rect.left - focused_rect.right)
                .map(|(w, _)| w.clone()),
            FocusDirection::Up => candidates
                .iter()
                .filter(|(_, rect)| rect.bottom < focused_rect.top)
                .min_by_key(|(_, rect)| focused_rect.top - rect.bottom)
                .map(|(w, _)| w.clone()),
            FocusDirection::Down => candidates
                .iter()
                .filter(|(_, rect)| rect.top > focused_rect.bottom)
                .min_by_key(|(_, rect)| rect.top - focused_rect.bottom)
                .map(|(w, _)| w.clone()),
        }
    }

    fn set_window_focus(&self, hwnd: HWND) {
        use windows::Win32::UI::WindowsAndMessaging::*;

        unsafe {
            let _ = SetForegroundWindow(hwnd);
        }
    }

    pub fn move_window(&self, direction: FocusDirection) -> Result<(), String> {
        // Find all windows in active workspace on all monitors first
        let mut active_windows: Vec<(Window, RECT)> = Vec::new();
        {
            let monitors = self.monitors.lock().unwrap();
            for monitor in monitors.iter() {
                let active_workspace = monitor.get_active_workspace();
                for window in &active_workspace.windows {
                    active_windows.push((window.clone(), window.rect));
                }
            }
        }

        if active_windows.is_empty() {
            println!("No active windows found");
            return Ok(()); // No windows to move
        }

        // Find the focused window in our active windows list
        let focused_hwnd = unsafe {
            use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
            GetForegroundWindow()
        };

        let focused = active_windows
            .iter()
            .find(|(window, _)| window.hwnd == focused_hwnd.0 as isize)
            .map(|(window, _)| window.clone());

        println!("Focused window: {:?}", focused);

        if focused.is_none() {
            return Err("Focused window not found in active workspace".to_string());
        }

        let focused = focused.unwrap();

        println!("Active windows: {:?}", active_windows);

        // Find target window to swap with
        let target = self.find_next_focus(&focused, direction, &active_windows);

        println!("Target window: {:?}", target);

        if let Some(target_window) = target {
            println!("Swapping window positions");
            let swap_result =
                self.swap_window_positions(HWND(focused.hwnd as _), HWND(target_window.hwnd as _));
            match swap_result {
                Ok(()) => {
                    // Re-apply window positions after swap
                    println!("Re-applying window positions");
                    self.apply_window_positions();
                    println!("Window positions swapped");
                    // Keep focus on the moved window
                    self.set_window_focus(HWND(focused.hwnd as _));
                    println!("Focus restored");
                }
                Err(err) => {
                    println!("Failed to swap window positions: {}", err);
                }
            }
        }

        Ok(())
    }

    fn swap_window_positions(&self, hwnd1: HWND, hwnd2: HWND) -> Result<(), String> {
        println!("Swapping window positions...");
        println!("Locking windows");
        let mut monitors = self.monitors.lock().unwrap();

        // Find both windows and swap their rects
        let mut window1_info: Option<(usize, usize, RECT)> = None;
        let mut window2_info: Option<(usize, usize, RECT)> = None;

        println!("Finding windows...");
        for (monitor_idx, monitor) in monitors.iter().enumerate() {
            let workspace_idx = (monitor.active_workspace - 1) as usize;
            for (win_idx, window) in monitor.workspaces[workspace_idx].windows.iter().enumerate() {
                if window.hwnd == hwnd1.0 as isize {
                    window1_info = Some((monitor_idx, win_idx, window.rect));
                }
                if window.hwnd == hwnd2.0 as isize {
                    window2_info = Some((monitor_idx, win_idx, window.rect));
                }
            }
        }

        println!("Windows found.");

        match (window1_info, window2_info) {
            (Some((m1, w1, rect1)), Some((m2, w2, rect2))) => {
                println!("Swapping windows...");
                // Store workspace indices to avoid borrowing issues
                let ws1_idx = (monitors[m1].active_workspace - 1) as usize;
                let ws2_idx = (monitors[m2].active_workspace - 1) as usize;

                // Swap the rects
                monitors[m1].workspaces[ws1_idx].windows[w1].rect = rect2;
                monitors[m2].workspaces[ws2_idx].windows[w2].rect = rect1;
                Ok(())
            }
            (None, None) => {
                println!("Could not find both windows to swap");
                return Err("Could not find both windows to swap".to_string());
            }
            _ => Err("Unexpected error occurred".to_string()),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum FocusDirection {
    Left,
    Right,
    Up,
    Down,
}

impl Default for WorkspaceManager {
    fn default() -> Self {
        Self::new()
    }
}
