use super::workspace::{Monitor, Window};
use crate::tiling::DwindleTiler;
use crate::windows_lib::{hide_window_from_taskbar, show_window_in_taskbar};
use std::sync::{Arc, Mutex};
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::UI::WindowsAndMessaging::{
    IsZoomed, SetWindowPos, ShowWindow, SWP_NOACTIVATE, SWP_NOZORDER, SW_RESTORE,
};

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
        println!("DEBUG: Setting {} monitors", monitors.len());
        for (i, monitor) in monitors.iter().enumerate() {
            println!(
                "DEBUG: Monitor {}: hmonitor={:?}, rect={:?}, active_workspace={}",
                i, monitor.hmonitor, monitor.rect, monitor.active_workspace
            );
        }
        let mut m = self.monitors.lock().unwrap();
        *m = monitors;
        println!("DEBUG: Monitors set successfully");
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

        println!("DEBUG: Finding monitor for window {:?}", hwnd.0);
        let mut rect = RECT::default();
        unsafe {
            if GetWindowRect(hwnd, &mut rect).is_err() {
                println!("DEBUG: Failed to get window rect for {:?}", hwnd.0);
                return None;
            }
        }

        println!("DEBUG: Window rect: {:?}", rect);
        let monitors = self.monitors.lock().unwrap();
        println!(
            "DEBUG: Checking {} monitors for window containment",
            monitors.len()
        );

        for (i, monitor) in monitors.iter().enumerate() {
            println!("DEBUG: Checking monitor {} rect: {:?}", i, monitor.rect);
            if rect.left >= monitor.rect.left
                && rect.top >= monitor.rect.top
                && rect.right <= monitor.rect.right
                && rect.bottom <= monitor.rect.bottom
            {
                println!("DEBUG: Window {:?} contained in monitor {}", hwnd.0, i);
                return Some(i);
            }
        }
        println!("DEBUG: Window {:?} not contained in any monitor", hwnd.0);
        None
    }

    pub fn switch_workspace(&mut self, workspace_num: u8) -> bool {
        if !(1..=9).contains(&workspace_num) {
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
        println!(
            "DEBUG: Adding window {:?} to workspace {} on monitor {}",
            window.hwnd, window.workspace, window.monitor
        );
        let mut monitors = self.monitors.lock().unwrap();
        if let Some(monitor) = monitors.get_mut(window.monitor) {
            println!(
                "DEBUG: Monitor {} found, adding window to workspace {}",
                window.monitor, window.workspace
            );
            monitor.add_window(window);
            println!("DEBUG: Window added successfully");
        } else {
            println!(
                "DEBUG: Monitor {} not found, cannot add window",
                window.monitor
            );
        }
    }

    pub fn remove_window(&self, hwnd: HWND) -> Option<Window> {
        println!("DEBUG: Removing window {:?}", hwnd.0);
        let mut monitors = self.monitors.lock().unwrap();
        for (monitor_idx, monitor) in monitors.iter_mut().enumerate() {
            println!(
                "DEBUG: Checking monitor {} for window {:?}",
                monitor_idx, hwnd.0
            );
            if let Some(window) = monitor.remove_window(hwnd) {
                println!(
                    "DEBUG: Found and removed window {:?} from monitor {}",
                    window.hwnd, monitor_idx
                );
                return Some(window);
            }
        }
        println!("DEBUG: Window {:?} not found in any monitor", hwnd.0);
        None
    }

    pub fn remove_window_with_tiling(&self, hwnd: HWND) -> Option<Window> {
        println!("DEBUG: Removing window with tiling update: {:?}", hwnd.0);
        let removed_window = self.remove_window(hwnd);

        if let Some(ref window) = removed_window {
            println!(
                "DEBUG: Window {:?} from workspace {} removed, re-tiling affected workspaces",
                window.hwnd, window.workspace
            );
            // Re-tile the workspace that had the window removed
            self.tile_active_workspaces();
            self.apply_window_positions();
            println!("DEBUG: Re-tiling completed after window removal");
        } else {
            println!("DEBUG: No window removed, skipping re-tiling");
        }

        removed_window
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

    pub fn get_workspace_window_count(&self, workspace_num: u8) -> usize {
        let monitors = self.monitors.lock().unwrap();
        let mut count = 0;
        for monitor in monitors.iter() {
            if let Some(workspace) = monitor.get_workspace(workspace_num) {
                count += workspace.windows.len();
            }
        }
        count
    }

    pub fn switch_workspace_with_windows(&mut self, new_workspace: u8) -> Result<(), String> {
        if !(1..=9).contains(&new_workspace) {
            println!(
                "DEBUG: Invalid workspace number requested: {}",
                new_workspace
            );
            return Err("Invalid workspace number".to_string());
        }

        let old_workspace = self.active_workspace_global;

        if old_workspace == new_workspace {
            println!(
                "DEBUG: Workspace switch requested to same workspace {}, no action needed",
                new_workspace
            );
            return Ok(()); // No change needed
        }

        println!(
            "DEBUG: Switching from workspace {} to {}",
            old_workspace, new_workspace
        );

        // Count windows in old workspace before switching
        let old_workspace_window_count = self.get_workspace_window_count(old_workspace);
        println!(
            "DEBUG: Old workspace {} has {} windows",
            old_workspace, old_workspace_window_count
        );

        // Count windows in new workspace
        let new_workspace_window_count = self.get_workspace_window_count(new_workspace);
        println!(
            "DEBUG: New workspace {} has {} windows",
            new_workspace, new_workspace_window_count
        );

        // Re-tile the old workspace before hiding windows (in case windows changed)
        println!(
            "DEBUG: Re-tiling old workspace {} before hiding windows",
            old_workspace
        );
        self.tile_active_workspaces();

        // Hide windows from old workspace
        println!("DEBUG: Hiding windows from workspace {}", old_workspace);
        self.hide_workspace_windows(old_workspace)?;

        // Show windows from new workspace
        println!("DEBUG: Showing windows from workspace {}", new_workspace);
        self.show_workspace_windows(new_workspace)?;

        // Update active workspace IMMEDIATELY after hide/show, before tiling
        println!(
            "DEBUG: Updating active workspace global to {}",
            new_workspace
        );
        self.active_workspace_global = new_workspace;

        // Update all monitors to reflect the new active workspace
        println!("DEBUG: Updating active workspace on all monitors");
        let mut monitors = self.monitors.lock().unwrap();
        for (i, monitor) in monitors.iter_mut().enumerate() {
            println!(
                "DEBUG: Setting monitor {} active workspace to {}",
                i, new_workspace
            );
            monitor.set_active_workspace(new_workspace);
        }
        drop(monitors);

        // Now tile the new workspace with correct active workspace state
        println!(
            "DEBUG: Tiling new workspace {} with updated state",
            new_workspace
        );
        self.tile_active_workspaces();

        // Apply window positions immediately
        println!(
            "DEBUG: Applying window positions for new workspace {}",
            new_workspace
        );
        self.apply_window_positions();

        println!("DEBUG: Workspace switch completed successfully");
        Ok(())
    }

    fn hide_workspace_windows(&self, workspace_num: u8) -> Result<(), String> {
        let monitors = self.monitors.lock().unwrap();
        let mut total_hidden = 0;
        let mut failed_count = 0;

        println!("DEBUG: Hiding windows for workspace {}", workspace_num);

        for (monitor_idx, monitor) in monitors.iter().enumerate() {
            if let Some(workspace) = monitor.get_workspace(workspace_num) {
                println!(
                    "DEBUG: Monitor {} has {} windows in workspace {}",
                    monitor_idx,
                    workspace.windows.len(),
                    workspace_num
                );
                for window in &workspace.windows {
                    println!("DEBUG: Hiding window {:?} from taskbar", window.hwnd);
                    if let Err(e) = hide_window_from_taskbar(HWND(window.hwnd as _)) {
                        eprintln!("DEBUG: Failed to hide window {:?}: {}", window.hwnd, e);
                        failed_count += 1;
                    } else {
                        total_hidden += 1;
                    }
                }
            } else {
                println!(
                    "DEBUG: Monitor {} has no workspace {}",
                    monitor_idx, workspace_num
                );
            }
        }

        println!(
            "DEBUG: Hidden {} windows, {} failed",
            total_hidden, failed_count
        );
        Ok(())
    }

    fn show_workspace_windows(&self, workspace_num: u8) -> Result<(), String> {
        let monitors = self.monitors.lock().unwrap();
        let mut total_shown = 0;
        let mut failed_count = 0;

        println!("DEBUG: Showing windows for workspace {}", workspace_num);

        for (monitor_idx, monitor) in monitors.iter().enumerate() {
            if let Some(workspace) = monitor.get_workspace(workspace_num) {
                println!(
                    "DEBUG: Monitor {} has {} windows in workspace {}",
                    monitor_idx,
                    workspace.windows.len(),
                    workspace_num
                );
                for window in &workspace.windows {
                    println!("DEBUG: Showing window {:?} in taskbar", window.hwnd);
                    if let Err(e) = show_window_in_taskbar(HWND(window.hwnd as _)) {
                        eprintln!("DEBUG: Failed to show window {:?}: {}", window.hwnd, e);
                        failed_count += 1;
                    } else {
                        total_shown += 1;
                    }
                }
            } else {
                println!(
                    "DEBUG: Monitor {} has no workspace {}",
                    monitor_idx, workspace_num
                );
            }
        }

        println!(
            "DEBUG: Shown {} windows, {} failed",
            total_shown, failed_count
        );
        Ok(())
    }

    pub fn move_window_to_workspace(&mut self, new_workspace: u8) -> Result<(), String> {
        if !(1..=9).contains(&new_workspace) {
            println!(
                "DEBUG: Invalid workspace number {} requested for window move",
                new_workspace
            );
            return Err("Invalid workspace number".to_string());
        }

        // Get currently focused window
        let focused = self.get_focused_window();

        if focused.is_none() {
            println!("DEBUG: No focused window found for moving");
            return Err("No focused window".to_string());
        }

        let focused_window = focused.unwrap();
        let hwnd = HWND(focused_window.hwnd as *mut std::ffi::c_void);

        let old_workspace = focused_window.workspace;

        if old_workspace == new_workspace {
            println!(
                "DEBUG: Window {:?} already in target workspace {}, no move needed",
                hwnd.0, new_workspace
            );
            return Ok(()); // Already in target workspace
        }

        println!(
            "DEBUG: Moving window {:?} from workspace {} to workspace {}",
            hwnd.0, old_workspace, new_workspace
        );

        // Remove window from current workspace
        let mut window_to_move = None;
        let mut source_monitor_idx = 0;

        let mut monitors = self.monitors.lock().unwrap();
        println!("DEBUG: Searching for window in monitors to remove");
        for (m_idx, monitor) in monitors.iter_mut().enumerate() {
            if let Some(workspace) = monitor.get_workspace_mut(old_workspace) {
                println!(
                    "DEBUG: Checking monitor {} workspace {} for window",
                    m_idx, old_workspace
                );
                if let Some(window) = workspace.remove_window(hwnd) {
                    window_to_move = Some(window);
                    source_monitor_idx = m_idx;
                    println!(
                        "DEBUG: Found and removed window from monitor {} workspace {}",
                        m_idx, old_workspace
                    );
                    break;
                }
            }
        }

        if let Some(mut window) = window_to_move {
            // Update window's workspace
            window.workspace = new_workspace;
            println!("DEBUG: Updated window workspace to {}", new_workspace);

            // Keep window on same monitor (find target workspace on same monitor)
            if let Some(monitor) = monitors.get_mut(source_monitor_idx) {
                if let Some(workspace) = monitor.get_workspace_mut(new_workspace) {
                    workspace.add_window(window.clone());
                    println!(
                        "DEBUG: Added window to target workspace {} on monitor {}",
                        new_workspace, source_monitor_idx
                    );
                } else {
                    println!(
                        "DEBUG: Failed to find target workspace {} on monitor {}",
                        new_workspace, source_monitor_idx
                    );
                }
            } else {
                println!(
                    "DEBUG: Failed to access source monitor {}",
                    source_monitor_idx
                );
            }

            println!(
                "DEBUG: Successfully moved window to workspace {}",
                new_workspace
            );

            // Re-tile the source workspace immediately after removing the window
            if old_workspace == self.active_workspace_global {
                println!("DEBUG: Source workspace is active, re-tiling after window removal");
                // Source workspace is currently active, so tile it
                let tiler = DwindleTiler::default();
                if let Some(monitor) = monitors.get_mut(source_monitor_idx) {
                    let workspace_idx = (old_workspace - 1) as usize;
                    if !monitor.workspaces[workspace_idx].windows.is_empty() {
                        println!(
                            "DEBUG: Tiling {} windows in source workspace {}",
                            monitor.workspaces[workspace_idx].windows.len(),
                            old_workspace
                        );
                        let monitor_copy = monitor.clone();
                        let windows = &mut monitor.workspaces[workspace_idx].windows;
                        tiler.tile_windows(&monitor_copy, windows);
                    } else {
                        println!(
                            "DEBUG: Source workspace {} is now empty, no tiling needed",
                            old_workspace
                        );
                    }
                }

                // Apply the new positions immediately
                println!("DEBUG: Applying new positions to remaining windows in source workspace");
                for monitor in monitors.iter() {
                    if monitor.active_workspace == old_workspace {
                        let active_workspace = monitor.get_active_workspace();
                        for win in &active_workspace.windows {
                            println!(
                                "DEBUG: Setting position for window {:?} to {:?}",
                                win.hwnd, win.rect
                            );
                            self.set_window_position(HWND(win.hwnd as _), &win.rect);
                        }
                    }
                }
            } else {
                println!(
                    "DEBUG: Source workspace {} is not active, skipping immediate re-tiling",
                    old_workspace
                );
            }

            // Switch to the target workspace to show the moved window
            drop(monitors);
            println!(
                "DEBUG: Switching to target workspace {} to show moved window",
                new_workspace
            );
            self.switch_workspace_with_windows(new_workspace)?;
            println!("DEBUG: Window move to workspace completed successfully");
            Ok(())
        } else {
            println!("DEBUG: Window {:?} not found in any workspace", hwnd.0);
            Err("Window not found".to_string())
        }
    }

    pub fn move_window_to_workspace_follow(&mut self, new_workspace: u8) -> Result<(), String> {
        // Move already switches to the target workspace
        self.move_window_to_workspace(new_workspace)
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
                if window.is_tiled {
                    self.set_window_position(HWND(window.hwnd as _), &window.rect);
                }
            }
        }
    }

    pub fn toggle_window_tiling(&self, hwnd: HWND) -> Result<(), String> {
        println!("DEBUG: Toggling tiling for window {:?}", hwnd.0);
        let mut monitors = self.monitors.lock().unwrap();
        let mut found = false;
        let mut is_now_tiled = false;

        for monitor in monitors.iter_mut() {
            for workspace in &mut monitor.workspaces {
                if let Some(window) = workspace.get_window_mut(hwnd) {
                    window.is_tiled = !window.is_tiled;
                    is_now_tiled = window.is_tiled;
                    found = true;

                    if !window.is_tiled {
                        // If it's now floating, restore its original rect
                        window.rect = window.original_rect;
                    }
                    break;
                }
            }
            if found {
                break;
            }
        }

        if !found {
            return Err("Window not found".to_string());
        }

        println!(
            "DEBUG: Window {:?} is now {}",
            hwnd.0,
            if is_now_tiled { "tiled" } else { "floating" }
        );

        // Re-tile active workspaces
        drop(monitors);
        self.tile_active_workspaces();
        self.apply_window_positions();

        Ok(())
    }

    fn set_window_position(&self, hwnd: HWND, rect: &RECT) {
        unsafe {
            // Restore the window if it's maximized, as SetWindowPos doesn't work on maximized windows
            if IsZoomed(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_RESTORE);
            }
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
        println!("DEBUG: Moving focus in direction {:?}", direction);

        let focused = self.get_focused_window();
        println!(
            "DEBUG: Currently focused window: {:?}",
            focused.as_ref().map(|w| w.hwnd)
        );

        // Find all windows in active workspace on all monitors
        let mut active_windows: Vec<(Window, RECT)> = Vec::new();
        let monitors = self.monitors.lock().unwrap();

        println!("DEBUG: Gathering active windows from all monitors");
        for (monitor_idx, monitor) in monitors.iter().enumerate() {
            let active_workspace = monitor.get_active_workspace();
            println!(
                "DEBUG: Monitor {} active workspace has {} windows",
                monitor_idx,
                active_workspace.windows.len()
            );
            for window in &active_workspace.windows {
                if window.is_tiled {
                    active_windows.push((window.clone(), window.rect));
                    println!(
                        "DEBUG: Active window: hwnd={:?}, rect={:?}",
                        window.hwnd, window.rect
                    );
                }
            }
        }

        if active_windows.is_empty() {
            println!("DEBUG: No active windows found, cannot move focus");
            return Ok(()); // No windows to focus
        }

        println!("DEBUG: Total active windows: {}", active_windows.len());

        let target = if let Some(focused) = focused {
            // Find window to move focus to based on direction
            println!("DEBUG: Finding next focus from current focused window");
            self.find_next_focus(&focused, direction, &active_windows)
        } else {
            // No window focused, focus the first window
            println!("DEBUG: No window currently focused, focusing first window");
            active_windows.first().map(|(w, _)| w.clone())
        };

        if let Some(target_window) = target {
            println!(
                "DEBUG: Setting focus to target window {:?}",
                target_window.hwnd
            );
            self.set_window_focus(HWND(target_window.hwnd as _));
            println!("DEBUG: Focus moved successfully");
        } else {
            println!("DEBUG: No suitable target window found for focus movement");
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
        println!(
            "DEBUG: Finding next focus from window {:?} with rect {:?}",
            focused.hwnd, focused_rect
        );

        let candidates: Vec<&(Window, RECT)> = windows
            .iter()
            .filter(|(w, _)| w.hwnd != focused.hwnd)
            .collect();

        println!(
            "DEBUG: Found {} candidate windows (excluding focused)",
            candidates.len()
        );

        if candidates.is_empty() {
            println!("DEBUG: No candidate windows available");
            return None;
        }

        // Find the best candidate based on direction
        let result = match direction {
            FocusDirection::Left => {
                println!("DEBUG: Looking for window to the left of focused window");
                let left_candidates: Vec<_> = candidates
                    .iter()
                    .filter(|(_, rect)| rect.right < focused_rect.left)
                    .collect();
                println!("DEBUG: {} windows found to the left", left_candidates.len());
                left_candidates
                    .iter()
                    .min_by_key(|(_, rect)| focused_rect.left - rect.right)
                    .map(|(w, _)| {
                        println!("DEBUG: Selected left candidate: hwnd={:?}", w.hwnd);
                        w.clone()
                    })
            }
            FocusDirection::Right => {
                println!("DEBUG: Looking for window to the right of focused window");
                let right_candidates: Vec<_> = candidates
                    .iter()
                    .filter(|(_, rect)| rect.left > focused_rect.right)
                    .collect();
                println!(
                    "DEBUG: {} windows found to the right",
                    right_candidates.len()
                );
                right_candidates
                    .iter()
                    .min_by_key(|(_, rect)| rect.left - focused_rect.right)
                    .map(|(w, _)| {
                        println!("DEBUG: Selected right candidate: hwnd={:?}", w.hwnd);
                        w.clone()
                    })
            }
            FocusDirection::Up => {
                println!("DEBUG: Looking for window above focused window");
                let up_candidates: Vec<_> = candidates
                    .iter()
                    .filter(|(_, rect)| rect.bottom < focused_rect.top)
                    .collect();
                println!("DEBUG: {} windows found above", up_candidates.len());
                up_candidates
                    .iter()
                    .min_by_key(|(_, rect)| focused_rect.top - rect.bottom)
                    .map(|(w, _)| {
                        println!("DEBUG: Selected up candidate: hwnd={:?}", w.hwnd);
                        w.clone()
                    })
            }
            FocusDirection::Down => {
                println!("DEBUG: Looking for window below focused window");
                let down_candidates: Vec<_> = candidates
                    .iter()
                    .filter(|(_, rect)| rect.top > focused_rect.bottom)
                    .collect();
                println!("DEBUG: {} windows found below", down_candidates.len());
                down_candidates
                    .iter()
                    .min_by_key(|(_, rect)| rect.top - focused_rect.bottom)
                    .map(|(w, _)| {
                        println!("DEBUG: Selected down candidate: hwnd={:?}", w.hwnd);
                        w.clone()
                    })
            }
        };

        if result.is_none() {
            println!(
                "DEBUG: No suitable window found in direction {:?}",
                direction
            );
        }

        result
    }

    fn set_window_focus(&self, hwnd: HWND) {
        use windows::Win32::UI::WindowsAndMessaging::*;

        println!("DEBUG: Setting focus to window {:?}", hwnd.0);
        unsafe {
            let result = SetForegroundWindow(hwnd);
            if result.as_bool() {
                println!("DEBUG: Successfully set focus to window {:?}", hwnd.0);
            } else {
                println!("DEBUG: Failed to set focus to window {:?}", hwnd.0);
            }
        }
    }

    pub fn move_window(&self, direction: FocusDirection) -> Result<(), String> {
        println!("DEBUG: Moving window in direction {:?}", direction);

        // Find all windows in active workspace on all monitors first
        let mut active_windows: Vec<(Window, RECT)> = Vec::new();
        {
            let monitors = self.monitors.lock().unwrap();
            println!("DEBUG: Gathering active windows for moving");
            for (monitor_idx, monitor) in monitors.iter().enumerate() {
                let active_workspace = monitor.get_active_workspace();
                println!(
                    "DEBUG: Monitor {} has {} windows in active workspace",
                    monitor_idx,
                    active_workspace.windows.len()
                );
                for window in &active_workspace.windows {
                    if window.is_tiled {
                        active_windows.push((window.clone(), window.rect));
                        println!(
                            "DEBUG: Window for moving: hwnd={:?}, rect={:?}",
                            window.hwnd, window.rect
                        );
                    }
                }
            }
        }

        if active_windows.is_empty() {
            println!("DEBUG: No active windows found to move");
            return Ok(()); // No windows to move
        }

        println!(
            "DEBUG: Total windows available for moving: {}",
            active_windows.len()
        );

        // Find the focused window in our active windows list
        let focused_hwnd = unsafe {
            use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
            GetForegroundWindow()
        };

        println!("DEBUG: Current foreground window: {:?}", focused_hwnd.0);

        let focused = active_windows
            .iter()
            .find(|(window, _)| window.hwnd == focused_hwnd.0 as isize)
            .map(|(window, _)| window.clone());

        println!(
            "DEBUG: Focused window in active list: {:?}",
            focused.as_ref().map(|w| w.hwnd)
        );

        if focused.is_none() {
            println!("DEBUG: Focused window not found in active workspace windows");
            return Err("Focused window not found in active workspace".to_string());
        }

        let focused = focused.unwrap();
        println!(
            "DEBUG: Moving focused window: hwnd={:?}, current rect={:?}",
            focused.hwnd, focused.rect
        );

        // Find target window to swap with
        println!(
            "DEBUG: Finding target window to swap with in direction {:?}",
            direction
        );
        let target = self.find_next_focus(&focused, direction, &active_windows);

        println!(
            "DEBUG: Target window for swap: {:?}",
            target.as_ref().map(|w| w.hwnd)
        );

        if let Some(target_window) = target {
            println!(
                "DEBUG: Swapping positions of windows {:?} and {:?}",
                focused.hwnd, target_window.hwnd
            );
            let swap_result =
                self.swap_window_positions(HWND(focused.hwnd as _), HWND(target_window.hwnd as _));
            match swap_result {
                Ok(()) => {
                    // Re-apply window positions after swap
                    println!("DEBUG: Re-applying window positions after successful swap");
                    self.apply_window_positions();
                    println!("DEBUG: Window positions swapped successfully");
                    // Keep focus on the moved window
                    println!("DEBUG: Restoring focus to moved window {:?}", focused.hwnd);
                    self.set_window_focus(HWND(focused.hwnd as _));
                    println!("DEBUG: Focus restored to moved window");
                }
                Err(err) => {
                    println!("DEBUG: Failed to swap window positions: {}", err);
                }
            }
        } else {
            println!("DEBUG: No suitable target window found to swap with");
        }

        Ok(())
    }

    fn swap_window_positions(&self, hwnd1: HWND, hwnd2: HWND) -> Result<(), String> {
        println!(
            "DEBUG: Swapping positions of windows {:?} and {:?}",
            hwnd1.0, hwnd2.0
        );
        let mut monitors = self.monitors.lock().unwrap();

        // Find both windows and swap their rects
        let mut window1_info: Option<(usize, usize, RECT)> = None;
        let mut window2_info: Option<(usize, usize, RECT)> = None;

        println!("DEBUG: Searching for windows in active workspaces");
        for (monitor_idx, monitor) in monitors.iter().enumerate() {
            let workspace_idx = (monitor.active_workspace - 1) as usize;
            println!(
                "DEBUG: Checking monitor {} active workspace {} (index {})",
                monitor_idx, monitor.active_workspace, workspace_idx
            );
            for (win_idx, window) in monitor.workspaces[workspace_idx].windows.iter().enumerate() {
                if window.hwnd == hwnd1.0 as isize {
                    window1_info = Some((monitor_idx, win_idx, window.rect));
                    println!(
                        "DEBUG: Found window1 at monitor {}, window index {}, rect {:?}",
                        monitor_idx, win_idx, window.rect
                    );
                }
                if window.hwnd == hwnd2.0 as isize {
                    window2_info = Some((monitor_idx, win_idx, window.rect));
                    println!(
                        "DEBUG: Found window2 at monitor {}, window index {}, rect {:?}",
                        monitor_idx, win_idx, window.rect
                    );
                }
            }
        }

        match (window1_info, window2_info) {
            (Some((m1, w1, rect1)), Some((m2, w2, rect2))) => {
                println!("DEBUG: Both windows found, proceeding with swap");
                // Store workspace indices to avoid borrowing issues
                let ws1_idx = (monitors[m1].active_workspace - 1) as usize;
                let ws2_idx = (monitors[m2].active_workspace - 1) as usize;

                println!(
                    "DEBUG: Swapping rects: window1 gets {:?}, window2 gets {:?}",
                    rect2, rect1
                );
                // Swap the rects
                monitors[m1].workspaces[ws1_idx].windows[w1].rect = rect2;
                monitors[m2].workspaces[ws2_idx].windows[w2].rect = rect1;
                println!("DEBUG: Window position swap completed successfully");
                Ok(())
            }
            (None, None) => {
                println!("DEBUG: Could not find either window to swap (both missing)");
                Err("Could not find both windows to swap".to_string())
            }
            (Some(_), None) => {
                println!("DEBUG: Could not find window2 to swap with");
                Err("Could not find both windows to swap".to_string())
            }
            (None, Some(_)) => {
                println!("DEBUG: Could not find window1 to swap");
                Err("Could not find both windows to swap".to_string())
            }
        }
    }

    pub fn print_workspace_status(&self) {
        let monitors = self.monitors.lock().unwrap();
        for (m_idx, monitor) in monitors.iter().enumerate() {
            println!("Monitor {}:", m_idx);
            for ws in 1..=9 {
                if let Some(workspace) = monitor.get_workspace(ws) {
                    let count = workspace.windows.len();
                    let active = if monitor.active_workspace == ws {
                        " (active)"
                    } else {
                        ""
                    };
                    println!("  Workspace {}: {} windows{}", ws, count, active);
                }
            }
        }
    }

    pub fn close_focused_window(&mut self) -> Result<(), String> {
        // Get currently focused window
        let focused = self.get_focused_window();

        if focused.is_none() {
            return Err("No focused window".to_string());
        }

        let focused_window = focused.unwrap();
        let hwnd = HWND(focused_window.hwnd as *mut std::ffi::c_void);

        println!("Closing window {:?}", hwnd.0);

        // Remove window from workspace tracking
        let removed = self.remove_window(hwnd);

        if removed.is_none() {
            return Err("Window not found in workspace manager".to_string());
        }

        // Close the actual window
        crate::windows_lib::close_window(hwnd)?;

        // Re-tile active workspace
        self.tile_active_workspaces();
        self.apply_window_positions();

        println!("Window closed and workspace re-tiled");
        Ok(())
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
