//! High-level workspace management and state coordination.
//!
//! The [`WorkspaceManager`] is the central coordinator for all window management
//! operations. It handles:
//! - Window tracking across workspaces and monitors
//! - Workspace switching with window visibility management
//! - Tiling layout application
//! - Focus management and window decorations
//! - Monitor hot-plugging

use super::workspace::{Monitor, Window};
use crate::statusbar::{STATUSBAR_MAX_WORKSPACES, StatusBar};
use crate::tiling::DwindleTiler;
use crate::windows_lib::{
    get_accent_color, hide_window_from_taskbar, reset_window_decorations, set_window_border_color,
    set_window_transparency, show_window_in_taskbar,
};
use log::{debug, error, info, warn};
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, IsZoomed, SW_RESTORE, SWP_NOACTIVATE, SWP_NOZORDER, SetWindowPos,
    ShowWindow,
};

/// Converts an isize window handle to HWND.
#[inline]
fn hwnd_from_isize(val: isize) -> HWND {
    HWND(val as *mut std::ffi::c_void)
}

/// Central coordinator for window and workspace management.
///
/// Manages all monitors, workspaces, and windows. Provides high-level
/// operations for workspace switching, window movement, and tiling.
pub struct WorkspaceManager {
    monitors: Vec<Monitor>,
    active_workspace_global: u8, // All monitors share the same active workspace
    last_reenumerate: Instant,
    statusbar: Option<StatusBar>,
    statusbar_visible: bool,
    last_focused_hwnd: Option<isize>,
    last_window_alpha: HashMap<isize, u8>,
    positioning_windows: HashSet<isize>, // Windows currently being positioned by us
    last_update_positions: Instant,      // Debounce update_window_positions calls
}

impl WorkspaceManager {
    /// Creates a new workspace manager with default state.
    pub fn new() -> Self {
        WorkspaceManager {
            monitors: Vec::new(),
            active_workspace_global: 1,
            last_reenumerate: Instant::now() - Duration::from_secs(60),
            statusbar: None,
            statusbar_visible: true,
            last_focused_hwnd: None,
            last_window_alpha: HashMap::new(),
            positioning_windows: HashSet::new(),
            last_update_positions: Instant::now() - Duration::from_secs(60),
        }
    }

    /// Sets the status bar instance for workspace indicator updates.
    pub fn set_statusbar(&mut self, statusbar: StatusBar) {
        self.statusbar = Some(statusbar);
    }

    /// Updates the status bar to reflect the current workspace.
    pub fn update_statusbar(&mut self) {
        let workspace_num = self.active_workspace_global;
        let mut occupied_6_9 = 0u8;
        for ws in 6..=9 {
            if self.get_workspace_window_count(ws) > 0 {
                occupied_6_9 |= 1 << (ws - 6);
            }
        }
        if let Some(statusbar) = self.statusbar.as_mut() {
            statusbar.update_indicator(workspace_num, STATUSBAR_MAX_WORKSPACES, occupied_6_9);
        }
    }

    /// Shows or hides the status bar.
    pub fn toggle_statusbar(&mut self, visible: bool) {
        self.statusbar_visible = visible;
        if let Some(statusbar) = self.statusbar.as_mut() {
            if visible {
                statusbar.show();
                self.update_statusbar();
            } else {
                statusbar.hide();
            }
        }
    }

    /// Toggles the status bar visibility.
    pub fn invert_statusbar_visibility(&mut self) {
        let desired = !self.statusbar_visible;
        self.toggle_statusbar(desired);
    }

    /// Updates window decorations (border color, transparency) based on focus state.
    pub fn update_decorations(&mut self) {
        let focused_hwnd = unsafe { GetForegroundWindow() };

        // If focus hasn't changed, we can still update if needed, but usually once is enough
        self.last_focused_hwnd = Some(focused_hwnd.0 as isize);

        let accent_color = match get_accent_color() {
            Ok(color) => color,
            Err(e) => {
                error!("Failed to read accent color: {}", e);
                return;
            }
        };

        let managed_hwnds = self.get_all_managed_hwnds();
        let managed_set: HashSet<isize> = managed_hwnds.iter().copied().collect();
        let unfocused_alpha: u8 = 245;

        for hwnd_val in &managed_hwnds {
            let hwnd = HWND(*hwnd_val as _);
            let desired_alpha = if hwnd == focused_hwnd {
                255
            } else {
                unfocused_alpha
            };
            let previous_alpha = self.last_window_alpha.get(hwnd_val).copied();

            if hwnd == focused_hwnd {
                if let Err(e) = set_window_border_color(hwnd, accent_color) {
                    error!("Failed to set window border color: {}", e);
                }
            } else if previous_alpha != Some(desired_alpha)
                && let Err(e) = reset_window_decorations(hwnd)
            {
                error!("Failed to reset window decorations: {}", e);
            }

            if previous_alpha != Some(desired_alpha) {
                if let Err(e) = set_window_transparency(hwnd, desired_alpha) {
                    error!("Failed to set window transparency: {}", e);
                } else {
                    self.last_window_alpha.insert(*hwnd_val, desired_alpha);
                }
            }
        }

        self.last_window_alpha
            .retain(|hwnd, _| managed_set.contains(hwnd));
    }

    /// Sets the list of monitors for the workspace manager.
    pub fn set_monitors(&mut self, monitors: Vec<Monitor>) {
        debug!("Setting {} monitors", monitors.len());
        for (i, monitor) in monitors.iter().enumerate() {
            debug!(
                "Monitor {}: hmonitor={:?}, rect={:?}, active_workspace={}",
                i, monitor.hmonitor, monitor.rect, monitor.active_workspace
            );
        }
        self.monitors = monitors;
        debug!("Monitors set successfully");
    }

    /// Returns the currently active workspace number (1-9).
    pub fn get_active_workspace(&self) -> u8 {
        self.active_workspace_global
    }

    /// Returns all window handles managed by Megatile across all workspaces.
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

    /// Determines which monitor a window belongs to.
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

    /// Finds the monitor in the specified direction from the given monitor.
    ///
    /// Returns the index of the adjacent monitor in the specified direction,
    /// or None if no monitor exists in that direction.
    pub fn find_monitor_in_direction(
        &self,
        monitor_idx: usize,
        direction: FocusDirection,
    ) -> Option<usize> {
        let current_monitor = self.monitors.get(monitor_idx)?;
        let current_rect = current_monitor.rect;

        // Calculate center point of current monitor
        let current_center_x = (current_rect.left + current_rect.right) / 2;
        let current_center_y = (current_rect.top + current_rect.bottom) / 2;

        let mut candidates: Vec<(usize, i32)> = Vec::new();

        for (i, monitor) in self.monitors.iter().enumerate() {
            if i == monitor_idx {
                continue; // Skip the current monitor
            }

            let monitor_rect = monitor.rect;
            let monitor_center_x = (monitor_rect.left + monitor_rect.right) / 2;
            let monitor_center_y = (monitor_rect.top + monitor_rect.bottom) / 2;

            let matches_direction = match direction {
                FocusDirection::Left => {
                    // Monitor is to the left if its center X is less than current center X
                    monitor_center_x < current_center_x
                }
                FocusDirection::Right => {
                    // Monitor is to the right if its center X is greater than current center X
                    monitor_center_x > current_center_x
                }
                FocusDirection::Up => {
                    // Monitor is above if its center Y is less than current center Y
                    monitor_center_y < current_center_y
                }
                FocusDirection::Down => {
                    // Monitor is below if its center Y is greater than current center Y
                    monitor_center_y > current_center_y
                }
            };

            if matches_direction {
                // Calculate distance (Manhattan distance for simplicity)
                let dx = (monitor_center_x - current_center_x).abs();
                let dy = (monitor_center_y - current_center_y).abs();
                let distance = dx + dy;
                candidates.push((i, distance));
            }
        }

        // Return the closest monitor in the specified direction
        candidates
            .iter()
            .min_by_key(|(_, distance)| *distance)
            .map(|(idx, _)| *idx)
    }

    /// Adds a window to the workspace manager.
    pub fn add_window(&mut self, window: Window) {
        debug!(
            "Adding window {:?} to workspace {} on monitor {}",
            window.hwnd, window.workspace, window.monitor
        );
        if let Some(monitor) = self.monitors.get_mut(window.monitor) {
            debug!(
                "Monitor {} found, adding window to workspace {}",
                window.monitor, window.workspace
            );
            monitor.add_window(window);
            self.update_statusbar();
            self.update_decorations();
            debug!("Window added successfully");
        } else {
            warn!("Monitor {} not found, cannot add window", window.monitor);
        }
    }

    /// Removes a window from tracking without re-tiling.
    pub fn remove_window(&mut self, hwnd: HWND) -> Option<Window> {
        debug!("Removing window {:?}", hwnd.0);
        self.last_window_alpha.remove(&(hwnd.0 as isize));
        for (monitor_idx, monitor) in self.monitors.iter_mut().enumerate() {
            debug!("Checking monitor {} for window {:?}", monitor_idx, hwnd.0);
            if let Some(window) = monitor.remove_window(hwnd) {
                debug!(
                    "Found and removed window {:?} from monitor {}",
                    window.hwnd, monitor_idx
                );
                return Some(window);
            }
        }
        debug!("Window {:?} not found", hwnd.0);
        None
    }

    /// Removes a window and re-tiles the affected workspace.
    pub fn remove_window_with_tiling(&mut self, hwnd: HWND) -> Option<Window> {
        debug!("Removing window with tiling update: {:?}", hwnd.0);
        let removed_window = self.remove_window(hwnd);

        if let Some(ref window) = removed_window {
            debug!(
                "Window {:?} from workspace {} removed, re-tiling affected workspaces",
                window.hwnd, window.workspace
            );
            // Re-tile the workspace that had the window removed
            self.tile_active_workspaces();
            self.apply_window_positions();
            self.update_statusbar();
            self.update_decorations();
            debug!("Re-tiling completed after window removal");
        } else {
            debug!("No window removed, skipping re-tiling");
        }

        removed_window
    }

    /// Finds a window by handle across all monitors and workspaces.
    pub fn get_window(&self, hwnd: HWND) -> Option<Window> {
        for monitor in self.monitors.iter() {
            if let Some(window) = monitor.get_window(hwnd) {
                return Some(window.clone());
            }
        }
        None
    }

    /// Checks if a window is in any active workspace across all monitors.
    ///
    /// Used to determine if a hidden window is expected (inactive workspace) or a zombie (hidden in active workspace).
    pub fn is_window_in_active_workspace(&self, hwnd: HWND) -> bool {
        let hwnd_val = hwnd.0 as isize;
        for monitor in self.monitors.iter() {
            let active_workspace = monitor.get_active_workspace();
            if active_workspace.windows.iter().any(|w| w.hwnd == hwnd_val) {
                return true;
            }
        }
        false
    }

    /// Re-enumerates monitors and updates workspace assignments.
    ///
    /// Called when monitor configuration changes (hot-plug, resolution change).
    pub fn reenumerate_monitors(&mut self) -> Result<(), String> {
        // Prevent redundant re-enumerations within 500ms
        if self.last_reenumerate.elapsed() < Duration::from_millis(500) {
            return Ok(());
        }
        self.last_reenumerate = Instant::now();

        info!("Re-enumerating monitors...");

        // Get current monitor info
        let monitor_infos = crate::windows_lib::enumerate_monitors();
        info!("Found {} monitor(s)", monitor_infos.len());

        let mut new_monitors: Vec<Monitor> = Vec::new();

        for (i, info) in monitor_infos.iter().enumerate() {
            debug!("Monitor {}: {:?}", i, info.rect);

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

        info!("Monitor re-enumeration complete");
        Ok(())
    }

    /// Checks if monitor configuration has changed.
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

    /// Returns the total window count for a workspace across all monitors.
    pub fn get_workspace_window_count(&self, workspace_num: u8) -> usize {
        let mut count = 0;
        for monitor in self.monitors.iter() {
            if let Some(workspace) = monitor.get_workspace(workspace_num) {
                count += workspace.windows.len();
            }
        }
        count
    }

    /// Switches to a different workspace, hiding/showing windows as needed.
    pub fn switch_workspace_with_windows(&mut self, new_workspace: u8) -> Result<(), String> {
        if !(1..=9).contains(&new_workspace) {
            warn!("Invalid workspace number requested: {}", new_workspace);
            return Err("Invalid workspace number".to_string());
        }

        let old_workspace = self.active_workspace_global;

        if old_workspace == new_workspace {
            debug!(
                "Workspace switch requested to same workspace {}, no action needed",
                new_workspace
            );
            return Ok(()); // No change needed
        }

        debug!(
            "Switching from workspace {} to {}",
            old_workspace, new_workspace
        );

        // Capture currently focused window for the old workspace before switching away
        if let Some(focused) = self.get_focused_window() {
            debug!(
                "Current focus is window {:?} in workspace {}",
                focused.hwnd, focused.workspace
            );
            if focused.workspace == old_workspace {
                for monitor in self.monitors.iter_mut() {
                    if let Some(workspace) = monitor.get_workspace_mut(old_workspace)
                        && workspace.get_window(HWND(focused.hwnd as _)).is_some()
                    {
                        workspace.focused_window_hwnd = Some(focused.hwnd);
                        debug!(
                            "Saved focus target {:?} for old workspace {}",
                            focused.hwnd, old_workspace
                        );
                    }
                }
            }
        }

        // Count windows in old workspace before switching
        let old_workspace_window_count = self.get_workspace_window_count(old_workspace);
        debug!(
            "Old workspace {} has {} windows",
            old_workspace, old_workspace_window_count
        );

        // Count windows in new workspace
        let new_workspace_window_count = self.get_workspace_window_count(new_workspace);
        debug!(
            "New workspace {} has {} windows",
            new_workspace, new_workspace_window_count
        );

        // Re-tile the old workspace before hiding windows (in case windows changed)
        debug!(
            "Re-tiling old workspace {} before hiding windows",
            old_workspace
        );
        self.tile_active_workspaces();

        // Exit fullscreen on all windows in old workspace
        self.exit_fullscreen_workspace(old_workspace);

        // Hide windows from old workspace
        debug!("Hiding windows from workspace {}", old_workspace);
        self.hide_workspace_windows(old_workspace)?;

        // Show windows from new workspace
        debug!("Showing windows from workspace {}", new_workspace);
        self.show_workspace_windows(new_workspace)?;

        // Update active workspace IMMEDIATELY after hide/show, before tiling
        debug!("Updating active workspace global to {}", new_workspace);
        self.active_workspace_global = new_workspace;

        // Update all monitors to reflect the new active workspace
        debug!("Updating active workspace on all monitors");
        for (i, monitor) in self.monitors.iter_mut().enumerate() {
            debug!(
                "Setting monitor {} active workspace to {}",
                i, new_workspace
            );
            monitor.set_active_workspace(new_workspace);
        }

        // Now tile the new workspace with correct active workspace state
        debug!("Tiling new workspace {} with updated state", new_workspace);
        self.tile_active_workspaces();

        // Apply window positions immediately
        debug!(
            "Applying window positions for new workspace {}",
            new_workspace
        );
        self.apply_window_positions();

        // Restore fullscreen state for windows that were previously fullscreen
        debug!(
            "Restoring fullscreen windows in workspace {}",
            new_workspace
        );
        self.restore_fullscreen_workspace(new_workspace);

        // Restore focus for the new workspace
        debug!("Restoring focus for workspace {}", new_workspace);
        let mut focus_target = None;
        for monitor in self.monitors.iter() {
            if let Some(workspace) = monitor.get_workspace(new_workspace) {
                if let Some(hwnd) = workspace.focused_window_hwnd {
                    focus_target = Some(hwnd_from_isize(hwnd));
                    debug!(
                        "Found remembered focus target {:?} for workspace {}",
                        hwnd, new_workspace
                    );
                    break;
                }
                // If no remembered focus, try the first tiled window
                if let Some(first_window) = workspace.windows.iter().find(|w| w.is_tiled) {
                    focus_target = Some(hwnd_from_isize(first_window.hwnd));
                    debug!(
                        "No remembered focus, using first tiled window {:?} for workspace {}",
                        first_window.hwnd, new_workspace
                    );
                    break;
                }
            }
        }

        if let Some(hwnd) = focus_target {
            debug!("Auto-focusing window {:?} after workspace switch", hwnd.0);
            self.set_window_focus(hwnd);
        } else {
            debug!("No window to focus in workspace {}", new_workspace);
        }

        self.update_statusbar();
        self.update_decorations();

        debug!("Workspace switch completed successfully");
        Ok(())
    }

    /// Sets visibility for all windows in a workspace (hide=true or show=false).
    fn set_workspace_windows_visibility(
        &mut self,
        workspace_num: u8,
        hide: bool,
    ) -> Result<(), String> {
        let action = if hide { "Hiding" } else { "Showing" };
        debug!("{} windows for workspace {}", action, workspace_num);

        let mut success_count = 0;
        let mut failed_count = 0;

        // MUTABLE iteration: Need to update is_hidden_by_workspace flag after hiding/showing
        for (monitor_idx, monitor) in self.monitors.iter_mut().enumerate() {
            if let Some(workspace) = monitor.get_workspace_mut(workspace_num) {
                debug!(
                    "Monitor {} has {} windows in workspace {}",
                    monitor_idx,
                    workspace.windows.len(),
                    workspace_num
                );
                for window in &mut workspace.windows {
                    let hwnd = hwnd_from_isize(window.hwnd);
                    let result = if hide {
                        hide_window_from_taskbar(hwnd)
                    } else {
                        show_window_in_taskbar(hwnd)
                    };
                    match result {
                        Ok(()) => {
                            success_count += 1;
                            // Track workspace hiding state to prevent cleanup from removing these windows
                            window.is_hidden_by_workspace = hide;
                            debug!("Window {:?} is_hidden_by_workspace = {}", window.hwnd, hide);
                        }
                        Err(e) => {
                            error!(
                                "Failed to {} window {:?}: {}",
                                action.to_lowercase(),
                                window.hwnd,
                                e
                            );
                            failed_count += 1;
                        }
                    }
                }
            }
        }

        debug!(
            "{} {} windows, {} failed",
            action, success_count, failed_count
        );
        Ok(())
    }

    /// Hides all windows in a workspace from the taskbar.
    fn hide_workspace_windows(&mut self, workspace_num: u8) -> Result<(), String> {
        self.set_workspace_windows_visibility(workspace_num, true)
    }

    /// Shows all windows in a workspace in the taskbar.
    fn show_workspace_windows(&mut self, workspace_num: u8) -> Result<(), String> {
        self.set_workspace_windows_visibility(workspace_num, false)
    }

    /// Moves the focused window to another workspace.
    pub fn move_window_to_workspace(&mut self, new_workspace: u8) -> Result<(), String> {
        if !(1..=9).contains(&new_workspace) {
            warn!(
                "Invalid workspace number {} requested for window move",
                new_workspace
            );
            return Err("Invalid workspace number".to_string());
        }

        // Get currently focused window
        let focused = self.get_focused_window();

        if focused.is_none() {
            warn!("No focused window found for moving");
            return Err("No focused window".to_string());
        }

        let focused_window = focused.unwrap();
        let hwnd = HWND(focused_window.hwnd as *mut std::ffi::c_void);

        let old_workspace = focused_window.workspace;

        if old_workspace == new_workspace {
            debug!(
                "Window {:?} already in target workspace {}, no move needed",
                hwnd.0, new_workspace
            );
            return Ok(()); // Already in target workspace
        }

        debug!(
            "Moving window {:?} from workspace {} to workspace {}",
            hwnd.0, old_workspace, new_workspace
        );

        // Remove window from current workspace
        let mut window_to_move = None;
        let mut source_monitor_idx = 0;
        let mut should_switch = false;
        let mut _result = Err("Window not found".to_string());

        debug!("Searching for window in monitors to remove");
        for (m_idx, monitor) in self.monitors.iter_mut().enumerate() {
            if let Some(workspace) = monitor.get_workspace_mut(old_workspace) {
                debug!(
                    "Checking monitor {} workspace {} for window",
                    m_idx, old_workspace
                );
                if let Some(window) = workspace.remove_window(hwnd) {
                    window_to_move = Some(window);
                    source_monitor_idx = m_idx;
                    debug!(
                        "Found and removed window from monitor {} workspace {}",
                        m_idx, old_workspace
                    );
                    break;
                }
            }
        }

        if let Some(mut window) = window_to_move {
            // Update window's workspace
            window.workspace = new_workspace;
            debug!("Updated window workspace to {}", new_workspace);

            // Keep window on same monitor (find target workspace on same monitor)
            if let Some(monitor) = self.monitors.get_mut(source_monitor_idx) {
                if let Some(workspace) = monitor.get_workspace_mut(new_workspace) {
                    let hwnd_val = window.hwnd;
                    workspace.add_window(window.clone());
                    workspace.focused_window_hwnd = Some(hwnd_val); // Ensure moved window is focused
                    debug!(
                        "Added window to target workspace {} on monitor {} and set as focus target",
                        new_workspace, source_monitor_idx
                    );
                } else {
                    warn!(
                        "Failed to find target workspace {} on monitor {}",
                        new_workspace, source_monitor_idx
                    );
                }
            } else {
                warn!("Failed to access source monitor {}", source_monitor_idx);
            }

            debug!("Successfully moved window to workspace {}", new_workspace);

            // Re-tile the source workspace immediately after removing the window
            if old_workspace == self.active_workspace_global {
                debug!("Source workspace is active, re-tiling after window removal");
                // Source workspace is currently active, so tile it
                let tiler = DwindleTiler::default();
                if let Some(monitor) = self.monitors.get_mut(source_monitor_idx) {
                    let workspace_idx = (old_workspace - 1) as usize;
                    if !monitor.workspaces[workspace_idx].windows.is_empty() {
                        debug!(
                            "Tiling {} windows in source workspace {}",
                            monitor.workspaces[workspace_idx].windows.len(),
                            old_workspace
                        );
                        let monitor_copy = monitor.clone();
                        let workspace = &mut monitor.workspaces[workspace_idx];
                        let layout_tree = &mut workspace.layout_tree;
                        let windows = &mut workspace.windows;
                        tiler.tile_windows(&monitor_copy, layout_tree, windows);
                    } else {
                        debug!(
                            "Source workspace {} is now empty, no tiling needed",
                            old_workspace
                        );
                    }
                }

                // Apply the new positions immediately
                debug!("Applying new positions to remaining windows in source workspace");

                // Collect windows to position to avoid borrow checker issues
                let mut windows_to_position: Vec<(isize, RECT)> = Vec::new();
                for monitor in self.monitors.iter() {
                    if monitor.active_workspace == old_workspace {
                        let active_workspace = monitor.get_active_workspace();
                        for win in &active_workspace.windows {
                            debug!(
                                "Setting position for window {:?} to {:?}",
                                win.hwnd, win.rect
                            );
                            windows_to_position.push((win.hwnd, win.rect));
                        }
                    }
                }

                // Now position them
                for (hwnd, rect) in windows_to_position {
                    self.set_window_position(hwnd_from_isize(hwnd), &rect);
                }
            } else {
                debug!(
                    "Source workspace {} is not active, skipping immediate re-tiling",
                    old_workspace
                );
            }

            should_switch = true;
            _result = Ok(());
        } else {
            warn!("Window {:?} not found in any workspace", hwnd.0);
            _result = Err("Window not found".to_string());
        }

        if should_switch {
            debug!(
                "Switching to target workspace {} to show moved window",
                new_workspace
            );
            self.switch_workspace_with_windows(new_workspace)?;
            debug!("Window move to workspace completed successfully");
        }

        _result
    }

    /// Moves the focused window to an adjacent monitor in the specified direction.
    ///
    /// If no monitor exists in the specified direction, this function returns Ok(())
    /// without moving the window (no-op behavior).
    pub fn move_window_to_monitor(&mut self, direction: FocusDirection) -> Result<(), String> {
        debug!("Moving window to monitor in direction {:?}", direction);

        // Get currently focused window
        let focused = self.get_focused_window();

        if focused.is_none() {
            warn!("No focused window found for moving to monitor");
            return Err("No focused window".to_string());
        }

        let focused_window = focused.unwrap();
        let hwnd = HWND(focused_window.hwnd as *mut std::ffi::c_void);
        let source_monitor_idx = focused_window.monitor;
        let current_workspace = focused_window.workspace;

        debug!(
            "Moving window {:?} from monitor {} to monitor in direction {:?}",
            hwnd.0, source_monitor_idx, direction
        );

        // Find target monitor in the specified direction
        let target_monitor_idx = match self.find_monitor_in_direction(source_monitor_idx, direction)
        {
            Some(idx) => idx,
            None => {
                debug!(
                    "No monitor found in direction {:?} from monitor {}",
                    direction, source_monitor_idx
                );
                return Ok(()); // No monitor in that direction, no-op
            }
        };

        if source_monitor_idx == target_monitor_idx {
            debug!(
                "Window {:?} already on target monitor {}, no move needed",
                hwnd.0, target_monitor_idx
            );
            return Ok(()); // Already on target monitor
        }

        debug!(
            "Target monitor {} found, moving window from monitor {}",
            target_monitor_idx, source_monitor_idx
        );

        // Remove window from current monitor/workspace
        let mut window_to_move = None;

        if let Some(monitor) = self.monitors.get_mut(source_monitor_idx)
            && let Some(workspace) = monitor.get_workspace_mut(current_workspace)
            && let Some(window) = workspace.remove_window(hwnd)
        {
            window_to_move = Some(window);
            debug!(
                "Removed window from monitor {} workspace {}",
                source_monitor_idx, current_workspace
            );
        }

        if let Some(mut window) = window_to_move {
            // Update window's monitor index
            window.monitor = target_monitor_idx;
            debug!("Updated window monitor to {}", target_monitor_idx);

            // Add window to target monitor's active workspace (same workspace number)
            if let Some(target_monitor) = self.monitors.get_mut(target_monitor_idx) {
                if let Some(target_workspace) = target_monitor.get_workspace_mut(current_workspace)
                {
                    let hwnd_val = window.hwnd;
                    target_workspace.add_window(window.clone());
                    target_workspace.focused_window_hwnd = Some(hwnd_val); // Ensure moved window is focused
                    debug!(
                        "Added window to monitor {} workspace {}",
                        target_monitor_idx, current_workspace
                    );
                } else {
                    return Err(format!(
                        "Failed to find workspace {} on target monitor {}",
                        current_workspace, target_monitor_idx
                    ));
                }
            } else {
                return Err(format!(
                    "Failed to access target monitor {}",
                    target_monitor_idx
                ));
            }

            // Re-tile both source and target monitors' active workspaces
            debug!("Re-tiling source and target monitors");
            self.tile_active_workspaces();
            self.apply_window_positions();

            // Keep focus on the moved window
            debug!("Restoring focus to moved window {:?}", hwnd.0);
            self.set_window_focus(hwnd);
            debug!("Window moved to monitor successfully");

            Ok(())
        } else {
            Err("Failed to remove window from source monitor".to_string())
        }
    }

    /// Applies tiling layout to all active workspaces on all monitors.
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

    /// Applies calculated positions to all tiled windows.
    pub fn apply_window_positions(&mut self) {
        // Collect windows to position first to avoid borrow checker issues
        let mut windows_to_position: Vec<(isize, RECT)> = Vec::new();

        for monitor in self.monitors.iter() {
            let active_workspace = monitor.get_active_workspace();

            for window in &active_workspace.windows {
                if window.is_tiled {
                    windows_to_position.push((window.hwnd, window.rect));
                }
            }
        }

        // Update all window rects FIRST to match target positions
        // This prevents update_window_positions from thinking they moved
        for (hwnd_val, target_rect) in &windows_to_position {
            for monitor in self.monitors.iter_mut() {
                for workspace in &mut monitor.workspaces {
                    if let Some(window) = workspace.get_window_mut(hwnd_from_isize(*hwnd_val)) {
                        window.rect = *target_rect;
                        break;
                    }
                }
            }
        }

        // Now position them
        for (hwnd, rect) in windows_to_position {
            self.set_window_position(hwnd_from_isize(hwnd), &rect);
        }

        // Clear positioning set after a brief moment to allow events to settle
        // We do this immediately since we've already updated window.rect to match
        self.positioning_windows.clear();
    }

    /// Toggles a window between tiled and floating state.
    pub fn toggle_window_tiling(&mut self, hwnd: HWND) -> Result<(), String> {
        debug!("Toggling tiling for window {:?}", hwnd.0);
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

        debug!(
            "Window {:?} is now {}",
            hwnd.0,
            if is_now_tiled { "tiled" } else { "floating" }
        );

        // Re-tile active workspaces
        self.tile_active_workspaces();
        self.apply_window_positions();

        Ok(())
    }

    /// Sets a window's position and size, accounting for DWM invisible borders.
    fn set_window_position(&mut self, hwnd: HWND, rect: &RECT) {
        let hwnd_val = hwnd.0 as isize;

        // Mark this window as being positioned by us
        self.positioning_windows.insert(hwnd_val);

        unsafe {
            // Restore the window if it's maximized, as SetWindowPos doesn't work on maximized windows
            if IsZoomed(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_RESTORE);
            }

            // Adjust for DWM invisible borders so the visible area matches our target
            let adjusted_rect = crate::windows_lib::adjust_rect_for_dwm_borders(hwnd, rect);

            SetWindowPos(
                hwnd,
                None,
                adjusted_rect.left,
                adjusted_rect.top,
                adjusted_rect.right - adjusted_rect.left,
                adjusted_rect.bottom - adjusted_rect.top,
                SWP_NOZORDER | SWP_NOACTIVATE,
            )
            .ok();
        }

        // Remove from positioning set after a brief delay to catch follow-up events
        // We'll clean this up in the next update cycle
    }

    /// Returns the currently focused window if it's managed by Megatile.
    pub fn get_focused_window(&self) -> Option<Window> {
        use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

        unsafe {
            let hwnd = GetForegroundWindow();
            self.get_window(hwnd)
        }
    }

    /// Moves focus to the nearest window in the specified direction.
    pub fn move_focus(&mut self, direction: FocusDirection) -> Result<(), String> {
        debug!("Moving focus in direction {:?}", direction);

        let focused = self.get_focused_window();
        debug!(
            "Currently focused window: {:?}",
            focused.as_ref().map(|w| w.hwnd)
        );

        // Find all windows in active workspace on all monitors first
        let mut active_windows: Vec<(Window, RECT)> = Vec::new();
        debug!("Gathering active windows from all monitors");
        for (monitor_idx, monitor) in self.monitors.iter().enumerate() {
            let active_workspace = monitor.get_active_workspace();
            debug!(
                "Monitor {} active workspace has {} windows",
                monitor_idx,
                active_workspace.windows.len()
            );
            for window in &active_workspace.windows {
                // allow focusing on tiled or fullscreen windows
                if window.is_tiled || (window.is_fullscreen && !window.is_tiled) {
                    active_windows.push((window.clone(), window.rect));
                    debug!(
                        "Active window: hwnd={:?}, rect={:?}",
                        window.hwnd, window.rect
                    );
                }
            }
        }

        if active_windows.is_empty() {
            debug!("No active windows found, cannot move focus");
            return Ok(()); // No windows to focus
        }

        debug!("Total active windows: {}", active_windows.len());

        let target = if let Some(focused) = focused {
            // Find window to move focus to based on direction
            debug!("Finding next focus from current focused window");
            self.find_next_focus(&focused, direction, &active_windows)
        } else {
            // No window focused, focus the first window
            debug!("No window currently focused, focusing first window");
            active_windows.first().map(|(w, _)| w.clone())
        };

        if let Some(target_window) = target {
            debug!("Setting focus to target window {:?}", target_window.hwnd);
            self.set_window_focus(HWND(target_window.hwnd as _));
            debug!("Focus moved successfully");
        } else {
            debug!("No suitable target window found for focus movement");
        }

        Ok(())
    }

    /// Finds the next window to focus based on spatial position.
    fn find_next_focus(
        &self,
        focused: &Window,
        direction: FocusDirection,
        windows: &[(Window, RECT)],
    ) -> Option<Window> {
        let focused_rect = focused.rect;
        let focused_center_x = (focused_rect.left + focused_rect.right) / 2;
        let focused_center_y = (focused_rect.top + focused_rect.bottom) / 2;

        debug!(
            "Finding next focus from window {:?} with rect {:?}",
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

        debug!(
            "{} windows found in direction {:?}",
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

    /// Sets focus to a specific window.
    pub fn set_window_focus(&mut self, hwnd: HWND) {
        use windows::Win32::UI::WindowsAndMessaging::*;

        debug!("Setting focus to window {:?}", hwnd.0);

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
                debug!("Successfully set focus to window {:?}", hwnd.0);
            } else {
                warn!("Failed to set focus to window {:?}", hwnd.0);
            }
        }
    }

    /// Swaps the focused window with the window in the specified direction.
    pub fn move_window(&mut self, direction: FocusDirection) -> Result<(), String> {
        debug!("Moving window in direction {:?}", direction);

        // Find all windows in active workspace on all monitors first
        let mut active_windows: Vec<(Window, RECT)> = Vec::new();
        debug!("Gathering active windows for moving");
        for (monitor_idx, monitor) in self.monitors.iter().enumerate() {
            let active_workspace = monitor.get_active_workspace();
            debug!(
                "Monitor {} has {} windows in active workspace",
                monitor_idx,
                active_workspace.windows.len()
            );
            for window in &active_workspace.windows {
                if window.is_tiled {
                    active_windows.push((window.clone(), window.rect));
                    debug!(
                        "Window for moving: hwnd={:?}, rect={:?}",
                        window.hwnd, window.rect
                    );
                }
            }
        }

        if active_windows.is_empty() {
            debug!("No active windows found to move");
            return Ok(()); // No windows to move
        }

        debug!(
            "Total windows available for moving: {}",
            active_windows.len()
        );

        // Find the focused window in our active windows list
        let focused_hwnd = unsafe {
            use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
            GetForegroundWindow()
        };

        debug!("Current foreground window: {:?}", focused_hwnd.0);

        let focused = active_windows
            .iter()
            .find(|(window, _)| window.hwnd == focused_hwnd.0 as isize)
            .map(|(window, _)| window.clone());

        debug!(
            "Focused window in active list: {:?}",
            focused.as_ref().map(|w| w.hwnd)
        );

        if focused.is_none() {
            warn!("Focused window not found in active workspace windows");
            return Err("Focused window not found in active workspace".to_string());
        }

        let focused = focused.unwrap();
        debug!(
            "Moving focused window: hwnd={:?}, current rect={:?}",
            focused.hwnd, focused.rect
        );

        // Find target window to swap with
        debug!(
            "Finding target window to swap with in direction {:?}",
            direction
        );
        let target = self.find_next_focus(&focused, direction, &active_windows);

        debug!(
            "Target window for swap: {:?}",
            target.as_ref().map(|w| w.hwnd)
        );

        if let Some(target_window) = target {
            debug!(
                "Swapping positions of windows {:?} and {:?}",
                focused.hwnd, target_window.hwnd
            );
            let swap_result =
                self.swap_window_positions(HWND(focused.hwnd as _), HWND(target_window.hwnd as _));
            match swap_result {
                Ok(()) => {
                    // Re-apply window positions after swap
                    debug!("Re-applying window positions after successful swap");
                    self.apply_window_positions();
                    debug!("Window positions swapped successfully");
                    // Keep focus on the moved window
                    debug!("Restoring focus to moved window {:?}", focused.hwnd);
                    self.set_window_focus(HWND(focused.hwnd as _));
                    debug!("Focus restored to moved window");
                }
                Err(err) => {
                    error!("Failed to swap window positions: {}", err);
                }
            }
        } else {
            debug!("No suitable target window found to swap with");
        }

        Ok(())
    }

    /// Swaps the positions of two windows in the tiling layout.
    fn swap_window_positions(&mut self, hwnd1: HWND, hwnd2: HWND) -> Result<(), String> {
        debug!(
            "Swapping positions of windows {:?} and {:?}",
            hwnd1.0, hwnd2.0
        );
        // Find both windows and swap their rects
        let mut window1_info: Option<(usize, usize, RECT)> = None;
        let mut window2_info: Option<(usize, usize, RECT)> = None;

        debug!("Searching for windows in active workspaces");
        for (monitor_idx, monitor) in self.monitors.iter().enumerate() {
            let workspace_idx = (monitor.active_workspace - 1) as usize;
            debug!(
                "Checking monitor {} active workspace {} (index {})",
                monitor_idx, monitor.active_workspace, workspace_idx
            );
            for (win_idx, window) in monitor.workspaces[workspace_idx].windows.iter().enumerate() {
                if window.hwnd == hwnd1.0 as isize {
                    window1_info = Some((monitor_idx, win_idx, window.rect));
                    debug!(
                        "Found window1 at monitor {}, window index {}, rect {:?}",
                        monitor_idx, win_idx, window.rect
                    );
                }
                if window.hwnd == hwnd2.0 as isize {
                    window2_info = Some((monitor_idx, win_idx, window.rect));
                    debug!(
                        "Found window2 at monitor {}, window index {}, rect {:?}",
                        monitor_idx, win_idx, window.rect
                    );
                }
            }
        }

        match (window1_info, window2_info) {
            (Some((m1, w1, rect1)), Some((m2, w2, rect2))) => {
                debug!("Both windows found, proceeding with swap");
                // Store workspace indices to avoid borrowing issues
                let ws1_idx = (self.monitors[m1].active_workspace - 1) as usize;
                let ws2_idx = (self.monitors[m2].active_workspace - 1) as usize;

                debug!(
                    "Swapping rects: window1 gets {:?}, window2 gets {:?}",
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

                debug!("Window position swap completed successfully");
                Ok(())
            }
            (None, None) => {
                warn!("Could not find either window to swap (both missing)");
                Err("Could not find both windows to swap".to_string())
            }
            (Some(_), None) => {
                warn!("Could not find window2 to swap with");
                Err("Could not find both windows to swap".to_string())
            }
            (None, Some(_)) => {
                warn!("Could not find window1 to swap");
                Err("Could not find both windows to swap".to_string())
            }
        }
    }

    /// Checks if a window is currently being positioned by us.
    pub fn is_positioning_window(&self, hwnd: HWND) -> bool {
        self.positioning_windows.contains(&(hwnd.0 as isize))
    }

    /// Handles a window being minimized.
    pub fn handle_window_minimized(&mut self, hwnd: HWND) {
        debug!("Handling minimized window {:?}", hwnd.0);

        // Remove the window from tiling
        if let Some(removed) = self.remove_window(hwnd) {
            debug!(
                "Removed minimized window {:?} from workspace {}",
                removed.hwnd, removed.workspace
            );

            // Re-tile if it was in the active workspace
            if removed.workspace == self.active_workspace_global {
                self.tile_active_workspaces();
                self.apply_window_positions();
                self.update_statusbar();
                self.update_decorations();
                debug!("Re-tiled after window minimize");
            }
        } else {
            debug!("Minimized window {:?} was not in our tracking", hwnd.0);
        }
    }

    /// Handles a window being restored from minimized state.
    pub fn handle_window_restored(&mut self, hwnd: HWND) {
        debug!("Handling restored window {:?}", hwnd.0);

        // Check if it's a normal window
        if !crate::windows_lib::is_normal_window_hwnd(hwnd) {
            debug!("Window {:?} is not a normal window, ignoring", hwnd.0);
            return;
        }

        // Check if window is already tracked
        if self.get_window(hwnd).is_some() {
            debug!("Window {:?} is already tracked, ignoring", hwnd.0);
            return;
        }

        // Check if window is still minimized (shouldn't be, but verify)
        if crate::windows_lib::is_window_minimized(hwnd) {
            debug!("Window {:?} is still minimized, ignoring", hwnd.0);
            return;
        }

        debug!("Re-registering restored window {:?}", hwnd.0);

        // Get current window rect
        let rect = crate::windows_lib::get_window_rect(hwnd).unwrap_or_default();

        // Get active workspace and monitor
        let active_workspace = self.active_workspace_global;
        let monitor_index = self.get_monitor_for_window(hwnd).unwrap_or(0);

        // Get process name for app-specific filtering
        let process_name = crate::windows_lib::get_process_name_for_window(hwnd);

        // Create new window object
        let window = super::workspace::Window::new(
            hwnd.0 as isize,
            active_workspace,
            monitor_index,
            rect,
            process_name,
        );

        // Show in taskbar
        let _ = show_window_in_taskbar(hwnd);

        // Add window and re-tile
        self.add_window(window);
        self.tile_active_workspaces();
        self.apply_window_positions();

        debug!(
            "Successfully re-registered and tiled restored window {:?}",
            hwnd.0
        );
    }

    /// Removes invalid windows that have become hidden, minimized, or destroyed.
    ///
    /// This function performs "garbage collection" on windows that apps have hidden
    /// without properly destroying them (e.g., Zoom login splash screens, Steam popups).
    /// It checks:
    /// - Minimized state (missed by events)
    /// - Window visibility (hidden by app) - UNLESS window is intentionally hidden by workspace switching
    /// - DWM cloaked state (UWP apps)
    /// - Invalid geometry (zero-size, off-screen)
    /// - Invalid window handles
    pub fn cleanup_invalid_windows(&mut self) {
        let mut invalid_windows = Vec::new();

        // Find all invalid windows across all monitors and workspaces
        for monitor in self.monitors.iter() {
            for workspace in &monitor.workspaces {
                for window in &workspace.windows {
                    let hwnd = hwnd_from_isize(window.hwnd);

                    // Check if window is still valid using comprehensive validation
                    // Pass is_hidden_by_workspace to skip visibility check for intentionally hidden windows
                    if !crate::windows_lib::is_window_still_valid(
                        hwnd,
                        window.is_hidden_by_workspace,
                    ) {
                        debug!(
                            "Cleanup: found invalid window {:?} (process: {:?}, hidden_by_ws: {})",
                            hwnd.0, window.process_name, window.is_hidden_by_workspace
                        );
                        invalid_windows.push(hwnd);
                    }
                }
            }
        }

        // Remove all invalid windows and re-tile affected workspaces
        for hwnd in invalid_windows {
            debug!("Cleaning up zombie/invalid window {:?}", hwnd.0);
            self.remove_window_with_tiling(hwnd);
        }
    }

    /// Updates internal tracking when windows are moved externally.
    pub fn update_window_positions(&mut self) {
        // Debounce: Don't update more frequently than every 50ms
        if self.last_update_positions.elapsed() < Duration::from_millis(50) {
            return;
        }
        self.last_update_positions = Instant::now();

        // Get monitor rects first
        let monitor_rects: Vec<RECT> = self.monitors.iter().map(|m| m.rect).collect();
        let mut moves: Vec<(isize, usize, usize)> = Vec::new(); // (hwnd, old_monitor_idx, new_monitor_idx)
        let mut any_tiled_moved = false;

        // Movement threshold: only consider it moved if changed by more than this
        // Set higher to account for DWM border adjustments
        const MOVE_THRESHOLD: i32 = 50;

        for monitor_idx in 0..self.monitors.len() {
            // To avoid borrowing issues, we'll iterate through indices
            for ws_idx in 0..self.monitors[monitor_idx].workspaces.len() {
                for win_idx in 0..self.monitors[monitor_idx].workspaces[ws_idx].windows.len() {
                    let hwnd = HWND(
                        self.monitors[monitor_idx].workspaces[ws_idx].windows[win_idx].hwnd as _,
                    );

                    let hwnd_val = hwnd.0 as isize;

                    // Skip windows we're currently positioning
                    if self.positioning_windows.contains(&hwnd_val) {
                        continue;
                    }

                    if let Ok(current_rect) = crate::windows_lib::get_window_rect(hwnd) {
                        let window =
                            &mut self.monitors[monitor_idx].workspaces[ws_idx].windows[win_idx];

                        // Calculate movement distance
                        let left_diff = (window.rect.left - current_rect.left).abs();
                        let top_diff = (window.rect.top - current_rect.top).abs();
                        let right_diff = (window.rect.right - current_rect.right).abs();
                        let bottom_diff = (window.rect.bottom - current_rect.bottom).abs();

                        let moved_significantly = left_diff > MOVE_THRESHOLD
                            || top_diff > MOVE_THRESHOLD
                            || right_diff > MOVE_THRESHOLD
                            || bottom_diff > MOVE_THRESHOLD;

                        if moved_significantly {
                            debug!(
                                "Window {:?} moved significantly: left={}, top={}, right={}, bottom={}",
                                hwnd_val, left_diff, top_diff, right_diff, bottom_diff
                            );

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
                                debug!("Tiled window {:?} moved by user, will re-tile", hwnd_val);
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
            if let Some(window) = self.remove_window(hwnd_from_isize(hwnd))
                && let Some(new_monitor) = self.monitors.get_mut(new_monitor_idx)
            {
                let ws_idx = (window.workspace - 1) as usize;
                new_monitor.workspaces[ws_idx].add_window(window);
            }
        }

        // If any tiled window moved, sort windows by position and re-tile
        if any_tiled_moved {
            debug!("Re-tiling due to user-moved tiled windows");
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

    /// Prints debug information about workspace state.
    pub fn print_workspace_status(&self) {
        for (m_idx, monitor) in self.monitors.iter().enumerate() {
            debug!("Monitor {}:", m_idx);
            for ws in 1..=9 {
                if let Some(workspace) = monitor.get_workspace(ws) {
                    let count = workspace.windows.len();
                    let active = if monitor.active_workspace == ws {
                        " (active)"
                    } else {
                        ""
                    };
                    debug!("  Workspace {}: {} windows{}", ws, count, active);
                }
            }
        }
    }

    /// Closes the currently focused window.
    pub fn close_focused_window(&mut self) -> Result<(), String> {
        // Get currently focused window
        let focused = self.get_focused_window();

        if focused.is_none() {
            return Err("No focused window".to_string());
        }

        let focused_window = focused.unwrap();
        let hwnd = HWND(focused_window.hwnd as *mut std::ffi::c_void);

        info!("Closing window {:?}", hwnd.0);

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
                next_focus = Some(hwnd_from_isize(hwnd));
                break;
            }
        }

        if let Some(hwnd) = next_focus {
            debug!("Auto-focusing next window {:?} after close", hwnd.0);
            self.set_window_focus(hwnd);
        }

        self.update_statusbar();
        self.update_decorations();

        info!("Window closed and workspace re-tiled");
        Ok(())
    }

    /// Toggles fullscreen mode for the focused window.
    pub fn toggle_fullscreen(&mut self) -> Result<(), String> {
        // Get currently focused window
        let focused = self.get_focused_window();

        if focused.is_none() {
            return Err("No focused window".to_string());
        }

        let focused_hwnd = hwnd_from_isize(focused.unwrap().hwnd);
        let mut handled = false;

        // Find and update the window in workspace
        for monitor in self.monitors.iter_mut() {
            let monitor_rect = monitor.rect;
            if let Some(workspace) = monitor.get_workspace_mut(monitor.active_workspace)
                && let Some(window) = workspace.get_window_mut(focused_hwnd)
            {
                if window.is_fullscreen {
                    // Restore from fullscreen
                    info!("Restoring window {:?} from fullscreen", focused_hwnd);
                    crate::windows_lib::restore_window_from_fullscreen(
                        focused_hwnd,
                        window.original_rect,
                    )?;
                    window.is_fullscreen = false;
                    window.is_tiled = true;
                } else {
                    // Set to fullscreen
                    info!("Setting window {:?} to fullscreen", focused_hwnd);
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
            self.update_statusbar();
            self.update_decorations();
            Ok(())
        } else {
            Err("Window not found in active workspace".to_string())
        }
    }

    /// Exits fullscreen for all windows in a workspace.
    /// Note: This restores windows from fullscreen visually but preserves the is_fullscreen flag
    /// so that fullscreen state can be restored when switching back to this workspace.
    fn exit_fullscreen_workspace(&mut self, workspace_num: u8) {
        for monitor in self.monitors.iter_mut() {
            if let Some(workspace) = monitor.get_workspace_mut(workspace_num) {
                for window in &mut workspace.windows {
                    if window.is_fullscreen {
                        debug!(
                            "Exiting fullscreen for window {:?} in workspace {} (preserving flag)",
                            window.hwnd, workspace_num
                        );
                        if let Err(e) = crate::windows_lib::restore_window_from_fullscreen(
                            hwnd_from_isize(window.hwnd),
                            window.original_rect,
                        ) {
                            error!("Failed to restore window from fullscreen: {}", e);
                        }
                        // Keep is_fullscreen = true so we can restore it when switching back
                    }
                }
            }
        }
    }

    /// Restores fullscreen state for windows that were previously fullscreen.
    /// Called when switching TO a workspace to restore windows marked as fullscreen.
    fn restore_fullscreen_workspace(&mut self, workspace_num: u8) {
        for monitor in self.monitors.iter_mut() {
            let monitor_rect = monitor.rect;
            if let Some(workspace) = monitor.get_workspace_mut(workspace_num) {
                for window in &mut workspace.windows {
                    if window.is_fullscreen {
                        debug!(
                            "Restoring fullscreen for window {:?} in workspace {}",
                            window.hwnd, workspace_num
                        );
                        if let Err(e) = crate::windows_lib::set_window_fullscreen(
                            hwnd_from_isize(window.hwnd),
                            monitor_rect,
                        ) {
                            error!("Failed to set window fullscreen: {}", e);
                        }
                        // Flag is already true, no need to set it
                    }
                }
            }
        }
    }

    /// Resizes the focused window's tile region by adjusting split ratios.
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
            if let Some(workspace) = monitor.get_workspace_mut(monitor.active_workspace)
                && let Some(layout_tree) = workspace.layout_tree.as_mut()
            {
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
                    target_tile.split_ratio = (target_tile.split_ratio + amount).clamp(0.1, 0.9);

                    // Re-apply tiling with updated ratios
                    self.tile_active_workspaces();
                    self.apply_window_positions();
                    return Ok(());
                }
            }
        }

        Err("No suitable ancestor found for resizing in this direction".to_string())
    }

    fn find_ancestor_with_direction(
        tile: &mut crate::tiling::Tile,
        hwnd: isize,
        target_direction: crate::tiling::SplitDirection,
    ) -> Option<&mut crate::tiling::Tile> {
        // Check if any child contains the window and has a deeper ancestor matching the direction
        let mut search_deeper = false;
        if let Some(ref children) = tile.children {
            if Self::tree_contains_window(&children.0, hwnd) {
                if Self::has_ancestor_with_direction(&children.0, hwnd, target_direction) {
                    search_deeper = true;
                }
            } else if Self::tree_contains_window(&children.1, hwnd)
                && Self::has_ancestor_with_direction(&children.1, hwnd, target_direction)
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
            } else if Self::tree_contains_window(&children.1, hwnd)
                && Self::has_ancestor_with_direction(&children.1, hwnd, target_direction)
            {
                return true;
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
        if let Some(ref children) = tile.children
            && (Self::has_parent_in_subtree(&children.0, hwnd)
                || Self::has_parent_in_subtree(&children.1, hwnd))
        {
            search_deeper = true;
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

    /// Flips the split direction of the region containing the focused window.
    pub fn flip_focused_region(&mut self) -> Result<(), String> {
        let focused = self.get_focused_window();
        if focused.is_none() {
            return Err("No focused window".to_string());
        }
        let focused_window = focused.unwrap();

        // Find the workspace and monitor for the focused window
        for monitor in self.monitors.iter_mut() {
            if let Some(workspace) = monitor.get_workspace_mut(monitor.active_workspace)
                && let Some(layout_tree) = workspace.layout_tree.as_mut()
            {
                // Find the tile containing the focused window
                if let Some(parent_tile) = Self::find_parent_tile(layout_tree, focused_window.hwnd)
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

/// Direction for focus and window movement operations.
#[derive(Debug, Clone, Copy)]
pub enum FocusDirection {
    Left,
    Right,
    Up,
    Down,
}

/// Direction for window resize operations.
#[derive(Debug, Clone, Copy)]
pub enum ResizeDirection {
    /// Resize horizontally (affects vertical splits).
    Horizontal,
    /// Resize vertically (affects horizontal splits).
    Vertical,
}

impl Default for WorkspaceManager {
    fn default() -> Self {
        Self::new()
    }
}
