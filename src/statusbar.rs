use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;

pub struct StatusBar {
    hwnd: HWND,
}

impl StatusBar {
    pub fn new(owner_hwnd: HWND) -> Result<Self, String> {
        unsafe {
            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE(WS_EX_TOPMOST.0 | WS_EX_TOOLWINDOW.0 | WS_EX_NOACTIVATE.0),
                w!("STATIC"),
                w!(""),
                WINDOW_STYLE(WS_POPUP.0 | WS_VISIBLE.0 | 0x1), // 0x1 is SS_CENTER
                0,
                0,
                0,
                0,
                Some(owner_hwnd),
                Some(HMENU::default()),
                Some(GetModuleHandleW(None).unwrap().into()),
                None,
            );

            let hwnd = match hwnd {
                Ok(h) => h,
                Err(e) => return Err(format!("Failed to create status bar window: {}", e)),
            };

            Ok(StatusBar { hwnd })
        }
    }

    pub fn set_text(&self, text: &str) {
        unsafe {
            let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
            SetWindowTextW(self.hwnd, windows::core::PCWSTR(wide.as_ptr())).ok();
        }
    }

    pub fn set_position(&self, x: i32, y: i32, width: i32, height: i32) {
        unsafe {
            let _ = SetWindowPos(
                self.hwnd,
                Some(HWND_TOPMOST),
                x,
                y,
                width,
                height,
                SWP_NOACTIVATE,
            );
        }
    }

    pub fn set_font(&self, hfont: HFONT) {
        unsafe {
            let _ = SendMessageW(
                self.hwnd,
                WM_SETFONT,
                Some(WPARAM(hfont.0 as usize)),
                Some(LPARAM(1)),
            );
        }
    }

    pub fn show(&self) {
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_SHOW);
        }
    }

    pub fn hide(&self) {
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_HIDE);
        }
    }

    pub fn update_full_status(
        &self,
        workspace_num: u8,
        window_count: usize,
        tiling_algorithm: &str,
    ) {
        let text = format!(
            " Workspace {} | {} Windows | {} ",
            workspace_num, window_count, tiling_algorithm
        );
        self.set_text(&text);
    }

    pub fn get_hwnd(&self) -> HWND {
        self.hwnd
    }
}
