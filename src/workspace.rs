use windows::Win32::Foundation::{HWND, RECT};

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
    pub hwnd: isize,
    pub workspace: u8,
    pub monitor: usize,
    pub rect: RECT,
    pub is_focused: bool,
    pub original_rect: RECT, // For restoring from fullscreen/hidden state
}

impl Window {
    pub fn new(hwnd: isize, workspace: u8, monitor: usize, rect: RECT) -> Self {
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
        let pos = self
            .windows
            .iter()
            .position(|w| w.hwnd == hwnd.0 as isize)?;
        Some(self.windows.remove(pos))
    }

    pub fn get_window(&self, hwnd: HWND) -> Option<&Window> {
        self.windows.iter().find(|w| w.hwnd == hwnd.0 as isize)
    }

    pub fn get_window_mut(&mut self, hwnd: HWND) -> Option<&mut Window> {
        self.windows.iter_mut().find(|w| w.hwnd == hwnd.0 as isize)
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
