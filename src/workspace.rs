//! Core data structures for window and workspace management.
//!
//! This module defines the fundamental types used throughout Megatile:
//! - [`Window`] - Represents a managed window
//! - [`Workspace`] - A collection of windows with layout state
//! - [`Monitor`] - A physical display with multiple workspaces

use windows::Win32::Foundation::{HWND, RECT};

/// Represents a window managed by Megatile.
///
/// Each window tracks its position, workspace assignment, tiling state,
/// focus status, and process information for app-specific handling.
#[derive(Debug, Clone)]
pub struct Window {
    pub hwnd: isize,
    pub workspace: u8,
    pub monitor: usize,
    pub rect: RECT,
    pub is_focused: bool,
    pub is_tiled: bool,
    pub original_rect: RECT, // For restoring from fullscreen/hidden state
    pub is_fullscreen: bool,
    pub process_name: Option<String>, // Process name (e.g., "Zoom.exe") for app-specific rules
    pub is_hidden_by_workspace: bool, // True when intentionally hidden due to workspace switching
}

impl Window {
    /// Creates a new window with the given handle, workspace, monitor, initial position, and process name.
    pub fn new(
        hwnd: isize,
        workspace: u8,
        monitor: usize,
        rect: RECT,
        process_name: Option<String>,
    ) -> Self {
        Window {
            hwnd,
            workspace,
            monitor,
            rect,
            is_focused: false,
            is_tiled: true,
            original_rect: rect,
            is_fullscreen: false,
            process_name,
            is_hidden_by_workspace: false, // New windows start visible (added to active workspace)
        }
    }
}

/// A virtual workspace containing windows and their layout state.
///
/// Each workspace maintains its own collection of windows and remembers
/// which window was last focused for seamless workspace switching.
#[derive(Debug, Clone)]
pub struct Workspace {
    /// Windows assigned to this workspace.
    pub windows: Vec<Window>,
    /// Handle of the last focused window in this workspace.
    pub focused_window_hwnd: Option<isize>,
    /// The tiling layout tree for this workspace.
    pub layout_tree: Option<crate::tiling::Tile>,
}

impl Workspace {
    /// Creates an empty workspace.
    pub fn new() -> Self {
        Workspace {
            windows: Vec::new(),
            focused_window_hwnd: None,
            layout_tree: None,
        }
    }

    /// Adds a window to this workspace, setting it as focused if no window is focused.
    pub fn add_window(&mut self, window: Window) {
        if self.focused_window_hwnd.is_none() && window.is_tiled {
            self.focused_window_hwnd = Some(window.hwnd);
        }
        self.windows.push(window);

        // Clear the layout tree when windows change - force fresh layout calculation
        self.layout_tree = None;
    }

    /// Removes a window by handle, returning it if found.
    pub fn remove_window(&mut self, hwnd: HWND) -> Option<Window> {
        let hwnd_val = hwnd.0 as isize;
        let pos = self.windows.iter().position(|w| w.hwnd == hwnd_val)?;

        if self.focused_window_hwnd == Some(hwnd_val) {
            self.focused_window_hwnd = None;
        }

        let removed = self.windows.remove(pos);

        // If we removed the focused window, try to focus another one
        if self.focused_window_hwnd.is_none()
            && let Some(first_tiled) = self.windows.iter().find(|w| w.is_tiled)
        {
            self.focused_window_hwnd = Some(first_tiled.hwnd);
        }

        // Clear the layout tree when windows change - force fresh layout calculation
        self.layout_tree = None;

        Some(removed)
    }

    /// Returns a reference to the window with the given handle.
    pub fn get_window(&self, hwnd: HWND) -> Option<&Window> {
        self.windows.iter().find(|w| w.hwnd == hwnd.0 as isize)
    }

    /// Returns a mutable reference to the window with the given handle.
    pub fn get_window_mut(&mut self, hwnd: HWND) -> Option<&mut Window> {
        self.windows.iter_mut().find(|w| w.hwnd == hwnd.0 as isize)
    }

    /// Returns the count of tiled windows only.
    pub fn window_count(&self) -> usize {
        self.windows.iter().filter(|w| w.is_tiled).count()
    }
}

/// Represents a physical monitor with multiple workspaces.
///
/// Each monitor has 9 workspaces (1-9), with one active at a time.
/// All monitors share the same active workspace number for synchronized switching.
#[derive(Debug, Clone)]
pub struct Monitor {
    /// Windows HMONITOR handle as isize.
    pub hmonitor: isize,
    /// Monitor screen bounds.
    pub rect: RECT,
    /// Array of 9 workspaces (indices 0-8 map to workspaces 1-9).
    pub workspaces: [Workspace; 9],
    /// Currently active workspace number (1-9).
    pub active_workspace: u8,
}

impl Monitor {
    /// Creates a new monitor with empty workspaces.
    pub fn new(hmonitor: isize, rect: RECT) -> Self {
        Monitor {
            hmonitor,
            rect,
            workspaces: std::array::from_fn(|_| Workspace::new()),
            active_workspace: 1,
        }
    }

    /// Returns a reference to the active workspace.
    pub fn get_active_workspace(&self) -> &Workspace {
        &self.workspaces[(self.active_workspace - 1) as usize]
    }

    /// Returns a workspace by number (1-9).
    pub fn get_workspace(&self, workspace_num: u8) -> Option<&Workspace> {
        if !(1..=9).contains(&workspace_num) {
            return None;
        }
        Some(&self.workspaces[(workspace_num - 1) as usize])
    }

    /// Returns a mutable workspace by number (1-9).
    pub fn get_workspace_mut(&mut self, workspace_num: u8) -> Option<&mut Workspace> {
        if !(1..=9).contains(&workspace_num) {
            return None;
        }
        Some(&mut self.workspaces[(workspace_num - 1) as usize])
    }

    /// Sets the active workspace. Returns false if the workspace number is invalid.
    pub fn set_active_workspace(&mut self, workspace_num: u8) -> bool {
        if !(1..=9).contains(&workspace_num) {
            return false;
        }
        self.active_workspace = workspace_num;
        true
    }

    /// Adds a window to the appropriate workspace based on its workspace field.
    pub fn add_window(&mut self, window: Window) {
        if let Some(workspace) = self.get_workspace_mut(window.workspace) {
            workspace.add_window(window);
        }
    }

    /// Removes a window from any workspace on this monitor.
    pub fn remove_window(&mut self, hwnd: HWND) -> Option<Window> {
        for workspace in &mut self.workspaces {
            if let Some(window) = workspace.remove_window(hwnd) {
                return Some(window);
            }
        }
        None
    }

    /// Finds a window by handle across all workspaces.
    pub fn get_window(&self, hwnd: HWND) -> Option<&Window> {
        for workspace in &self.workspaces {
            if let Some(window) = workspace.get_window(hwnd) {
                return Some(window);
            }
        }
        None
    }
}
