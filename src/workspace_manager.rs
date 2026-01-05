use super::workspace::{Monitor, Window};
use crate::tiling::DwindleTiler;
use crate::windows_lib::{hide_window_from_taskbar, show_window_in_taskbar};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::UI::WindowsAndMessaging::{
    IsZoomed, SW_RESTORE, SWP_NOACTIVATE, SWP_NOZORDER, SetWindowPos, ShowWindow,
};

pub struct WorkspaceManager {
    monitors: Vec<Monitor>,
    active_workspace_global: u8, // All monitors share the same active workspace
    last_reenumerate: Instant,
}

impl WorkspaceManager {
    pub fn new() -> Self {
        WorkspaceManager {
            monitors: Vec::new(),
            active_workspace_global: 1,
            last_reenumerate: Instant::now() - Duration::from_secs(60),
        }
    }

    pub fn set_monitors(&mut self, monitors: Vec<Monitor>) {
        println!("DEBUG: Setting {} monitors", monitors.len());
        for (i, monitor) in monitors.iter().enumerate() {
            println!(
                "DEBUG: Monitor {}: hmonitor={:?}, rect={:?}, active_workspace={}",
                i, monitor.hmonitor, monitor.rect, monitor.active_workspace
            );
        }
        self.monitors = monitors;
        println!("DEBUG: Monitors set successfully");
    }

    pub fn get_active_workspace(&self) -> u8 {
        self.active_workspace_global
    }

    pub fn get_all_managed_hwnds(&self) -> Vec<isize> {
        let mut hwnds = Vec::new();
        for monitor in self.monitors.iter() {
            for workspace in &monitor.workspaces {
                for window in &workspace.windows {
                    hwnds.push(window.hwnd);
                }
            }
        }
        hwnds
    }

    pub fn get_monitor_for_window(&self, hwnd: HWND) -> Option<usize> {
        use windows::Win32::Graphics::Gdi::{MONITOR_DEFAULTTONEAREST, MonitorFromWindow};

        let hmonitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };

        for (i, monitor) in self.monitors.iter().enumerate() {
            if monitor.hmonitor == hmonitor.0 as isize {
                return Some(i);
            }
        }

        // Fallback to containment check if hmonitor doesn't match
        for (i, monitor) in self.monitors.iter().enumerate() {
            if let Ok(rect) = crate::windows_lib::get_window_rect(hwnd)
                && rect.left >= monitor.rect.left
                && rect.top >= monitor.rect.top
                && rect.right <= monitor.rect.right
                && rect.bottom <= monitor.rect.bottom
            {
                return Some(i);
            }
        }

        None
    }

    pub fn add_window(&mut self, window: Window) {
        println!(
            "DEBUG: Adding window {:?} to workspace {} on monitor {}",
            window.hwnd, window.workspace, window.monitor
        );
        if let Some(monitor) = self.monitors.get_mut(window.monitor) {
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

    pub fn remove_window(&mut self, hwnd: HWND) -> Option<Window> {
        println!("DEBUG: Removing window {:?}", hwnd.0);
        for (monitor_idx, monitor) in self.monitors.iter_mut().enumerate() {
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
        println!("DEBUG: Window {:?} not found", hwnd.0);
        None
    }

    pub fn remove_window_with_tiling(&mut self, hwnd: HWND) -> Option<Window> {
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
        for monitor in self.monitors.iter() {
            if let Some(window) = monitor.get_window(hwnd) {
                return Some(window.clone());
            }
        }
        None
    }

    pub fn get_active_workspace_windows(&self, monitor_index: usize) -> Vec<Window> {
        if let Some(monitor) = self.monitors.get(monitor_index) {
            monitor.get_active_workspace().windows.clone()
        } else {
            Vec::new()
        }
    }

    pub fn reenumerate_monitors(&mut self) -> Result<(), String> {
        // Prevent redundant re-enumerations within 500ms
        if self.last_reenumerate.elapsed() < Duration::from_millis(500) {
            return Ok(());
        }
        self.last_reenumerate = Instant::now();

        println!("Re-enumerating monitors...");

        // Get current monitor info
        let monitor_infos = crate::windows_lib::enumerate_monitors();
        println!("Found {} monitor(s)", monitor_infos.len());

        let mut new_monitors: Vec<Monitor> = Vec::new();

        for (i, info) in monitor_infos.iter().enumerate() {
            println!("  Monitor {}: {:?}", i, info.rect);

            // Try to preserve workspace data from existing monitor by matching hmonitor
            let existing_workspace_data = if let Some(old_monitor) =
                self.monitors.iter().find(|m| m.hmonitor == info.hmonitor)
            {
                old_monitor.workspaces.clone()
            } else {
                std::array::from_fn(|_| crate::workspace::Workspace::new())
            };

            let mut monitor = Monitor::new(info.hmonitor, info.rect);
            monitor.workspaces = existing_workspace_data;
            monitor.active_workspace = self.active_workspace_global;
            new_monitors.push(monitor);
        }

        // Update monitors
        self.monitors = new_monitors;

        // Re-tile active workspace on all monitors
        self.tile_active_workspaces();
        self.apply_window_positions();

        println!("Monitor re-enumeration complete");
        Ok(())
    }

    pub fn check_monitor_changes(&mut self) -> bool {
        let current_infos = crate::windows_lib::enumerate_monitors();
        if current_infos.len() != self.monitors.len() {
            return true;
        }

        for (i, info) in current_infos.iter().enumerate() {
            if info.hmonitor != self.monitors[i].hmonitor
                || info.rect.left != self.monitors[i].rect.left
                || info.rect.top != self.monitors[i].rect.top
                || info.rect.right != self.monitors[i].rect.right
                || info.rect.bottom != self.monitors[i].rect.bottom
            {
                return true;
            }
        }

        false
    }

    pub fn get_workspace_window_count(&self, workspace_num: u8) -> usize {
        let mut count = 0;
        for monitor in self.monitors.iter() {
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

        // Capture currently focused window for the old workspace before switching away
        if let Some(focused) = self.get_focused_window() {
            println!(
                "DEBUG: Current focus is window {:?} in workspace {}",
                focused.hwnd, focused.workspace
            );
            if focused.workspace == old_workspace {
                for monitor in self.monitors.iter_mut() {
                    if let Some(workspace) = monitor.get_workspace_mut(old_workspace)
                        && workspace.get_window(HWND(focused.hwnd as _)).is_some()
                    {
                        workspace.focused_window_hwnd = Some(focused.hwnd);
                        println!(
                            "DEBUG: Saved focus target {:?} for old workspace {}",
                            focused.hwnd, old_workspace
                        );
                    }
                }
            }
        }

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

        // Exit fullscreen on all windows in old workspace
        self.exit_fullscreen_workspace(old_workspace);

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
        for (i, monitor) in self.monitors.iter_mut().enumerate() {
            println!(
                "DEBUG: Setting monitor {} active workspace to {}",
                i, new_workspace
            );
            monitor.set_active_workspace(new_workspace);
        }

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

        // Restore focus for the new workspace
        println!("DEBUG: Restoring focus for workspace {}", new_workspace);
        let mut focus_target = None;
        for monitor in self.monitors.iter() {
            if let Some(workspace) = monitor.get_workspace(new_workspace) {
                if let Some(hwnd) = workspace.focused_window_hwnd {
                    focus_target = Some(HWND(hwnd as _));
                    println!(
                        "DEBUG: Found remembered focus target {:?} for workspace {}",
                        hwnd, new_workspace
                    );
                    break;
                }
                // If no remembered focus, try the first tiled window
                if let Some(first_window) = workspace.windows.iter().find(|w| w.is_tiled) {
                    focus_target = Some(HWND(first_window.hwnd as _));
                    println!(
                        "DEBUG: No remembered focus, using first tiled window {:?} for workspace {}",
                        first_window.hwnd, new_workspace
                    );
                    break;
                }
            }
        }

        if let Some(hwnd) = focus_target {
            println!(
                "DEBUG: Auto-focusing window {:?} after workspace switch",
                hwnd.0
            );
            self.set_window_focus(hwnd);
        } else {
            println!("DEBUG: No window to focus in workspace {}", new_workspace);
        }

        println!("DEBUG: Workspace switch completed successfully");
        Ok(())
    }

    fn hide_workspace_windows(&self, workspace_num: u8) -> Result<(), String> {
        let mut total_hidden = 0;
        let mut failed_count = 0;

        println!("DEBUG: Hiding windows for workspace {}", workspace_num);

        for (monitor_idx, monitor) in self.monitors.iter().enumerate() {
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
        let mut total_shown = 0;
        let mut failed_count = 0;

        println!("DEBUG: Showing windows for workspace {}", workspace_num);

        for (monitor_idx, monitor) in self.monitors.iter().enumerate() {
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
        let mut should_switch = false;
        let mut result = Err("Window not found".to_string());

        println!("DEBUG: Searching for window in monitors to remove");
        for (m_idx, monitor) in self.monitors.iter_mut().enumerate() {
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
            if let Some(monitor) = self.monitors.get_mut(source_monitor_idx) {
                if let Some(workspace) = monitor.get_workspace_mut(new_workspace) {
                    let hwnd_val = window.hwnd;
                    workspace.add_window(window.clone());
                    workspace.focused_window_hwnd = Some(hwnd_val); // Ensure moved window is focused
                    println!(
                        "DEBUG: Added window to target workspace {} on monitor {} and set as focus target",
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
                if let Some(monitor) = self.monitors.get_mut(source_monitor_idx) {
                    let workspace_idx = (old_workspace - 1) as usize;
                    if !monitor.workspaces[workspace_idx].windows.is_empty() {
                        println!(
                            "DEBUG: Tiling {} windows in source workspace {}",
                            monitor.workspaces[workspace_idx].windows.len(),
                            old_workspace
                        );
                        let monitor_copy = monitor.clone();
                        let workspace = &mut monitor.workspaces[workspace_idx];
                        let layout_tree = &mut workspace.layout_tree;
                        let windows = &mut workspace.windows;
                        tiler.tile_windows(&monitor_copy, layout_tree, windows);
                    } else {
                        println!(
                            "DEBUG: Source workspace {} is now empty, no tiling needed",
                            old_workspace
                        );
                    }
                }

                // Apply the new positions immediately
                println!("DEBUG: Applying new positions to remaining windows in source workspace");
                for monitor in self.monitors.iter() {
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

            should_switch = true;
            result = Ok(());
        } else {
            println!("DEBUG: Window {:?} not found in any workspace", hwnd.0);
            result = Err("Window not found".to_string());
        }

        if should_switch {
            println!(
                "DEBUG: Switching to target workspace {} to show moved window",
                new_workspace
            );
            self.switch_workspace_with_windows(new_workspace)?;
            println!("DEBUG: Window move to workspace completed successfully");
        }

        result
    }

    pub fn move_window_to_workspace_follow(&mut self, new_workspace: u8) -> Result<(), String> {
        // Move already switches to the target workspace
        self.move_window_to_workspace(new_workspace)
    }

    pub fn tile_active_workspaces(&mut self) {
        let tiler = DwindleTiler::default();
        for monitor in self.monitors.iter_mut() {
            let workspace_idx = (monitor.active_workspace - 1) as usize;

            if !monitor.workspaces[workspace_idx].windows.is_empty() {
                // Create a copy of the monitor for reading
                let monitor_copy = monitor.clone();
                let workspace = &mut monitor.workspaces[workspace_idx];
                let layout_tree = &mut workspace.layout_tree;
                let windows = &mut workspace.windows;
                tiler.tile_windows(&monitor_copy, layout_tree, windows);
            }
        }
    }

    pub fn apply_window_positions(&self) {
        for monitor in self.monitors.iter() {
            let active_workspace = monitor.get_active_workspace();

            for window in &active_workspace.windows {
                if window.is_tiled {
                    self.set_window_position(HWND(window.hwnd as _), &window.rect);
                }
            }
        }
    }

    pub fn toggle_window_tiling(&mut self, hwnd: HWND) -> Result<(), String> {
        println!("DEBUG: Toggling tiling for window {:?}", hwnd.0);
        let mut found = false;
        let mut is_now_tiled = false;
        let mut rect_to_restore = None;

        for monitor in self.monitors.iter_mut() {
            for workspace in &mut monitor.workspaces {
                if let Some(window) = workspace.get_window_mut(hwnd) {
                    window.is_tiled = !window.is_tiled;
                    is_now_tiled = window.is_tiled;
                    found = true;

                    if !window.is_tiled {
                        // If it's now floating, restore its original rect
                        window.rect = window.original_rect;
                        rect_to_restore = Some(window.original_rect);
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

        if let Some(rect) = rect_to_restore {
            self.set_window_position(hwnd, &rect);
        }

        println!(
            "DEBUG: Window {:?} is now {}",
            hwnd.0,
            if is_now_tiled { "tiled" } else { "floating" }
        );

        // Re-tile active workspaces
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

    pub fn move_focus(&mut self, direction: FocusDirection) -> Result<(), String> {
        println!("DEBUG: Moving focus in direction {:?}", direction);

        let focused = self.get_focused_window();
        println!(
            "DEBUG: Currently focused window: {:?}",
            focused.as_ref().map(|w| w.hwnd)
        );

        // Find all windows in active workspace on all monitors first
        let mut active_windows: Vec<(Window, RECT)> = Vec::new();
        println!("DEBUG: Gathering active windows from all monitors");
        for (monitor_idx, monitor) in self.monitors.iter().enumerate() {
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
        let focused_center_x = (focused_rect.left + focused_rect.right) / 2;
        let focused_center_y = (focused_rect.top + focused_rect.bottom) / 2;

        println!(
            "DEBUG: Finding next focus from window {:?} with rect {:?}",
            focused.hwnd, focused_rect
        );

        let candidates: Vec<&(Window, RECT)> = windows
            .iter()
            .filter(|(w, _)| w.hwnd != focused.hwnd)
            .collect();

        if candidates.is_empty() {
            return None;
        }

        let filtered_candidates: Vec<_> = candidates
            .iter()
            .filter(|(_, rect)| match direction {
                FocusDirection::Left => rect.right <= focused_rect.left,
                FocusDirection::Right => rect.left >= focused_rect.right,
                FocusDirection::Up => rect.bottom <= focused_rect.top,
                FocusDirection::Down => rect.top >= focused_rect.bottom,
            })
            .collect();

        println!(
            "DEBUG: {} windows found in direction {:?}",
            filtered_candidates.len(),
            direction
        );

        filtered_candidates
            .iter()
            .min_by_key(|(_, rect)| {
                let rect_center_x = (rect.left + rect.right) / 2;
                let rect_center_y = (rect.top + rect.bottom) / 2;

                let (dist_primary, dist_secondary) = match direction {
                    FocusDirection::Left => (
                        focused_rect.left - rect.right,
                        (focused_center_y - rect_center_y).abs(),
                    ),
                    FocusDirection::Right => (
                        rect.left - focused_rect.right,
                        (focused_center_y - rect_center_y).abs(),
                    ),
                    FocusDirection::Up => (
                        focused_rect.top - rect.bottom,
                        (focused_center_x - rect_center_x).abs(),
                    ),
                    FocusDirection::Down => (
                        rect.top - focused_rect.bottom,
                        (focused_center_x - rect_center_x).abs(),
                    ),
                };

                // Prioritize primary distance, then secondary
                // Use a large multiplier for primary distance to ensure it's the main factor
                dist_primary * 1000 + dist_secondary
            })
            .map(|(w, _)| w.clone())
    }

    pub fn set_window_focus(&mut self, hwnd: HWND) {
        use windows::Win32::UI::WindowsAndMessaging::*;

        println!("DEBUG: Setting focus to window {:?}", hwnd.0);

        // Update focus memory in the workspace
        let mut found = false;
        for monitor in self.monitors.iter_mut() {
            for workspace in &mut monitor.workspaces {
                if workspace.get_window(hwnd).is_some() {
                    workspace.focused_window_hwnd = Some(hwnd.0 as isize);
                    found = true;
                    break;
                }
            }
            if found {
                break;
            }
        }

        unsafe {
            let result = SetForegroundWindow(hwnd);
            if result.as_bool() {
                println!("DEBUG: Successfully set focus to window {:?}", hwnd.0);
            } else {
                println!("DEBUG: Failed to set focus to window {:?}", hwnd.0);
            }
        }
    }

    pub fn move_window(&mut self, direction: FocusDirection) -> Result<(), String> {
        println!("DEBUG: Moving window in direction {:?}", direction);

        // Find all windows in active workspace on all monitors first
        let mut active_windows: Vec<(Window, RECT)> = Vec::new();
        println!("DEBUG: Gathering active windows for moving");
        for (monitor_idx, monitor) in self.monitors.iter().enumerate() {
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

    fn swap_window_positions(&mut self, hwnd1: HWND, hwnd2: HWND) -> Result<(), String> {
        println!(
            "DEBUG: Swapping positions of windows {:?} and {:?}",
            hwnd1.0, hwnd2.0
        );
        // Find both windows and swap their rects
        let mut window1_info: Option<(usize, usize, RECT)> = None;
        let mut window2_info: Option<(usize, usize, RECT)> = None;

        println!("DEBUG: Searching for windows in active workspaces");
        for (monitor_idx, monitor) in self.monitors.iter().enumerate() {
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
                let ws1_idx = (self.monitors[m1].active_workspace - 1) as usize;
                let ws2_idx = (self.monitors[m2].active_workspace - 1) as usize;

                println!(
                    "DEBUG: Swapping rects: window1 gets {:?}, window2 gets {:?}",
                    rect2, rect1
                );
                // Swap the rects
                self.monitors[m1].workspaces[ws1_idx].windows[w1].rect = rect2;
                self.monitors[m2].workspaces[ws2_idx].windows[w2].rect = rect1;

                // IMPORTANT: Also swap the HWNDs in the layout tree if it exists
                if m1 == m2 && ws1_idx == ws2_idx {
                    if let Some(ref mut layout_tree) =
                        self.monitors[m1].workspaces[ws1_idx].layout_tree
                    {
                        Self::swap_hwnds_in_tree(layout_tree, hwnd1.0 as isize, hwnd2.0 as isize);
                    }
                } else {
                    // If moving across monitors/workspaces, just clear the trees to be safe
                    // and let them re-generate on next tile call.
                    self.monitors[m1].workspaces[ws1_idx].layout_tree = None;
                    self.monitors[m2].workspaces[ws2_idx].layout_tree = None;
                }

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

    pub fn update_window_positions(&mut self) {
        // Get monitor rects first
        let monitor_rects: Vec<RECT> = self.monitors.iter().map(|m| m.rect).collect();
        let mut moves: Vec<(isize, usize, usize)> = Vec::new(); // (hwnd, old_monitor_idx, new_monitor_idx)
        let mut any_tiled_moved = false;

        for monitor_idx in 0..self.monitors.len() {
            // To avoid borrowing issues, we'll iterate through indices
            for ws_idx in 0..self.monitors[monitor_idx].workspaces.len() {
                for win_idx in 0..self.monitors[monitor_idx].workspaces[ws_idx].windows.len() {
                    let hwnd = HWND(
                        self.monitors[monitor_idx].workspaces[ws_idx].windows[win_idx].hwnd as _,
                    );

                    if let Ok(current_rect) = crate::windows_lib::get_window_rect(hwnd) {
                        let window =
                            &mut self.monitors[monitor_idx].workspaces[ws_idx].windows[win_idx];

                        let moved_from_last_known = window.rect.left != current_rect.left
                            || window.rect.top != current_rect.top
                            || window.rect.right != current_rect.right
                            || window.rect.bottom != current_rect.bottom;

                        if moved_from_last_known {
                            // Update original_rect whenever the window moves from its last known position
                            // This captures the user's "preferred" position
                            if !window.is_fullscreen {
                                window.original_rect = current_rect;
                            }

                            if !window.is_tiled {
                                // If it's floating, also update its current tracking rect
                                window.rect = current_rect;
                            } else {
                                // Tiled window moved, will need to re-tile
                                any_tiled_moved = true;
                            }
                        }

                        // Also check if monitor changed
                        let center_x = (current_rect.left + current_rect.right) / 2;
                        let center_y = (current_rect.top + current_rect.bottom) / 2;

                        let new_monitor_idx = monitor_rects
                            .iter()
                            .enumerate()
                            .find(|(_, rect)| {
                                center_x >= rect.left
                                    && center_x <= rect.right
                                    && center_y >= rect.top
                                    && center_y <= rect.bottom
                            })
                            .map(|(i, _)| i);

                        if let Some(new_idx) = new_monitor_idx
                            && new_idx != window.monitor
                        {
                            moves.push((window.hwnd, window.monitor, new_idx));
                            window.monitor = new_idx;
                        }
                    }
                }
            }
        }

        // Apply moves
        for (hwnd, _old_monitor_idx, new_monitor_idx) in moves {
            if let Some(window) = self.remove_window(HWND(hwnd as _)) {
                if let Some(new_monitor) = self.monitors.get_mut(new_monitor_idx) {
                    let ws_idx = (window.workspace - 1) as usize;
                    new_monitor.workspaces[ws_idx].add_window(window);
                }
            }
        }

        // If any tiled window moved, sort windows by position and re-tile
        if any_tiled_moved {
            for monitor in self.monitors.iter_mut() {
                let ws_idx = (monitor.active_workspace - 1) as usize;
                monitor.workspaces[ws_idx]
                    .windows
                    .sort_by_key(|w| (w.rect.left, w.rect.top));
            }
            self.tile_active_workspaces();
            self.apply_window_positions();
        }
    }

    pub fn print_workspace_status(&self) {
        for (m_idx, monitor) in self.monitors.iter().enumerate() {
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

        // Focus the next window in the workspace
        let active_workspace_num = self.active_workspace_global;
        let mut next_focus = None;
        for monitor in self.monitors.iter() {
            if let Some(workspace) = monitor.get_workspace(active_workspace_num)
                && let Some(hwnd) = workspace.focused_window_hwnd
            {
                next_focus = Some(HWND(hwnd as _));
                break;
            }
        }

        if let Some(hwnd) = next_focus {
            println!("DEBUG: Auto-focusing next window {:?} after close", hwnd.0);
            self.set_window_focus(hwnd);
        }

        println!("Window closed and workspace re-tiled");
        Ok(())
    }

    pub fn toggle_fullscreen(&mut self) -> Result<(), String> {
        // Get currently focused window
        let focused = self.get_focused_window();

        if focused.is_none() {
            return Err("No focused window".to_string());
        }

        let focused_hwnd = HWND(focused.unwrap().hwnd as _);
        let mut handled = false;

        // Find and update the window in workspace
        for monitor in self.monitors.iter_mut() {
            let monitor_rect = monitor.rect;
            if let Some(workspace) = monitor.get_workspace_mut(monitor.active_workspace)
                && let Some(window) = workspace.get_window_mut(focused_hwnd)
            {
                if window.is_fullscreen {
                    // Restore from fullscreen
                    println!("Restoring window {:?} from fullscreen", focused_hwnd);
                    crate::windows_lib::restore_window_from_fullscreen(
                        focused_hwnd,
                        window.original_rect,
                    )?;
                    window.is_fullscreen = false;
                    window.is_tiled = true;
                } else {
                    // Set to fullscreen
                    println!("Setting window {:?} to fullscreen", focused_hwnd);
                    window.original_rect = window.rect; // Store current position
                    crate::windows_lib::set_window_fullscreen(focused_hwnd, monitor_rect)?;
                    window.is_fullscreen = true;
                    window.is_tiled = false;
                }
                handled = true;
                break;
            }
        }

        if handled {
            self.tile_active_workspaces();
            self.apply_window_positions();
            Ok(())
        } else {
            Err("Window not found in active workspace".to_string())
        }
    }

    pub fn exit_fullscreen_all(&mut self) {
        for monitor in self.monitors.iter_mut() {
            for workspace in &mut monitor.workspaces {
                for window in &mut workspace.windows {
                    if window.is_fullscreen {
                        crate::windows_lib::restore_window_from_fullscreen(
                            HWND(window.hwnd as _),
                            window.original_rect,
                        )
                        .ok();
                        window.is_fullscreen = false;
                    }
                }
            }
        }
    }

    fn exit_fullscreen_workspace(&mut self, workspace_num: u8) {
        for monitor in self.monitors.iter_mut() {
            if let Some(workspace) = monitor.get_workspace_mut(workspace_num) {
                for window in &mut workspace.windows {
                    if window.is_fullscreen {
                        crate::windows_lib::restore_window_from_fullscreen(
                            HWND(window.hwnd as _),
                            window.original_rect,
                        )
                        .ok();
                        window.is_fullscreen = false;
                    }
                }
            }
        }
    }

    pub fn resize_focused_window(
        &mut self,
        direction: ResizeDirection,
        amount: f32,
    ) -> Result<(), String> {
        let focused = self.get_focused_window();
        if focused.is_none() {
            return Err("No focused window".to_string());
        }
        let focused_window = focused.unwrap();

        // Find the workspace and monitor for the focused window
        for monitor in self.monitors.iter_mut() {
            if let Some(workspace) = monitor.get_workspace_mut(monitor.active_workspace) {
                if let Some(layout_tree) = workspace.layout_tree.as_mut() {
                    // Find the ancestor tile with matching split direction
                    let target_direction = match direction {
                        ResizeDirection::Horizontal => crate::tiling::SplitDirection::Vertical,
                        ResizeDirection::Vertical => crate::tiling::SplitDirection::Horizontal,
                    };

                    if let Some(target_tile) = Self::find_ancestor_with_direction(
                        layout_tree,
                        focused_window.hwnd,
                        target_direction,
                    ) {
                        // Adjust the split ratio
                        target_tile.split_ratio =
                            (target_tile.split_ratio + amount).clamp(0.1, 0.9);

                        // Re-apply tiling with updated ratios
                        self.tile_active_workspaces();
                        self.apply_window_positions();
                        return Ok(());
                    }
                }
            }
        }

        Err("No suitable ancestor found for resizing in this direction".to_string())
    }

    fn find_ancestor_with_direction<'a>(
        tile: &'a mut crate::tiling::Tile,
        hwnd: isize,
        target_direction: crate::tiling::SplitDirection,
    ) -> Option<&'a mut crate::tiling::Tile> {
        // Check if any child contains the window and has a deeper ancestor matching the direction
        let mut search_deeper = false;
        if let Some(ref children) = tile.children {
            if Self::tree_contains_window(&children.0, hwnd) {
                if Self::has_ancestor_with_direction(&children.0, hwnd, target_direction) {
                    search_deeper = true;
                }
            } else if Self::tree_contains_window(&children.1, hwnd) {
                if Self::has_ancestor_with_direction(&children.1, hwnd, target_direction) {
                    search_deeper = true;
                }
            }
        }

        if search_deeper {
            let children = tile.children.as_mut().unwrap();
            let child_to_search = if Self::tree_contains_window(&children.0, hwnd) {
                &mut children.0
            } else {
                &mut children.1
            };
            return Self::find_ancestor_with_direction(child_to_search, hwnd, target_direction);
        }

        // If no deeper ancestor found, check if this one matches
        if tile.split_direction == Some(target_direction) && Self::tree_contains_window(tile, hwnd)
        {
            return Some(tile);
        }

        None
    }

    fn has_ancestor_with_direction(
        tile: &crate::tiling::Tile,
        hwnd: isize,
        target_direction: crate::tiling::SplitDirection,
    ) -> bool {
        if let Some(ref children) = tile.children {
            if Self::tree_contains_window(&children.0, hwnd) {
                if Self::has_ancestor_with_direction(&children.0, hwnd, target_direction) {
                    return true;
                }
            } else if Self::tree_contains_window(&children.1, hwnd) {
                if Self::has_ancestor_with_direction(&children.1, hwnd, target_direction) {
                    return true;
                }
            }

            if tile.split_direction == Some(target_direction) {
                return true;
            }
        }
        false
    }

    fn find_parent_tile(
        tile: &mut crate::tiling::Tile,
        hwnd: isize,
    ) -> Option<&mut crate::tiling::Tile> {
        // Check if any child is a parent
        let mut search_deeper = false;
        if let Some(ref children) = tile.children {
            if Self::has_parent_in_subtree(&children.0, hwnd)
                || Self::has_parent_in_subtree(&children.1, hwnd)
            {
                search_deeper = true;
            }
        }

        if search_deeper {
            let children = tile.children.as_mut().unwrap();
            let child_to_search = if Self::tree_contains_window(&children.0, hwnd) {
                &mut children.0
            } else {
                &mut children.1
            };
            return Self::find_parent_tile(child_to_search, hwnd);
        }

        // If no child is a parent, but this tile contains the window, then this is the parent
        if tile.windows.contains(&hwnd) && tile.children.is_some() {
            return Some(tile);
        }

        None
    }

    fn has_parent_in_subtree(tile: &crate::tiling::Tile, hwnd: isize) -> bool {
        if let Some(ref children) = tile.children {
            if Self::has_parent_in_subtree(&children.0, hwnd)
                || Self::has_parent_in_subtree(&children.1, hwnd)
            {
                return true;
            }
            if tile.windows.contains(&hwnd) {
                return true;
            }
        }
        false
    }

    fn tree_contains_window(tile: &crate::tiling::Tile, hwnd: isize) -> bool {
        if tile.is_leaf() {
            tile.windows.contains(&hwnd)
        } else if let Some(ref children) = tile.children {
            Self::tree_contains_window(&children.0, hwnd)
                || Self::tree_contains_window(&children.1, hwnd)
        } else {
            false
        }
    }

    pub fn flip_focused_region(&mut self) -> Result<(), String> {
        let focused = self.get_focused_window();
        if focused.is_none() {
            return Err("No focused window".to_string());
        }
        let focused_window = focused.unwrap();

        // Find the workspace and monitor for the focused window
        for monitor in self.monitors.iter_mut() {
            if let Some(workspace) = monitor.get_workspace_mut(monitor.active_workspace) {
                if let Some(layout_tree) = workspace.layout_tree.as_mut() {
                    // Find the tile containing the focused window
                    if let Some(parent_tile) =
                        Self::find_parent_tile(layout_tree, focused_window.hwnd)
                    {
                        // Flip the split direction
                        parent_tile.split_direction = match parent_tile.split_direction {
                            Some(crate::tiling::SplitDirection::Horizontal) => {
                                Some(crate::tiling::SplitDirection::Vertical)
                            }
                            Some(crate::tiling::SplitDirection::Vertical) => {
                                Some(crate::tiling::SplitDirection::Horizontal)
                            }
                            None => None,
                        };

                        // Re-apply tiling with flipped direction
                        self.tile_active_workspaces();
                        self.apply_window_positions();
                        return Ok(());
                    }
                }
            }
        }

        Err("Focused window not found in layout tree".to_string())
    }

    fn swap_hwnds_in_tree(tile: &mut crate::tiling::Tile, hwnd1: isize, hwnd2: isize) {
        // Update windows list in the current tile (both leaf and intermediate)
        for hwnd in &mut tile.windows {
            if *hwnd == hwnd1 {
                *hwnd = hwnd2;
            } else if *hwnd == hwnd2 {
                *hwnd = hwnd1;
            }
        }

        // Recurse into children
        if let Some(ref mut children) = tile.children {
            Self::swap_hwnds_in_tree(&mut children.0, hwnd1, hwnd2);
            Self::swap_hwnds_in_tree(&mut children.1, hwnd1, hwnd2);
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

#[derive(Debug, Clone, Copy)]
pub enum ResizeDirection {
    Horizontal,
    Vertical,
}

impl Default for WorkspaceManager {
    fn default() -> Self {
        Self::new()
    }
}
