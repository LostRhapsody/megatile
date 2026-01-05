# Megatile TODO

NOTE: We're working on LINUX right now, so all cargo checks will fail. Work on the fixes below and we'll test in Windows later. Good luck.

1. Resolve CodeRabbit comments
2. the transparency effect is super glitchy and flickers every few seconds, make it less transparent and stop the flickering
3. the status bar should have rounded corners, be floating (not touching the top of the screen, like a 2 pixel gap on the top), and be a simple pill with active workspaces in it.
    3-1. When first started, it will display 5 circles, the first being 'active'. When switching workspaces, the active dot changes. You can add up to 9 active workspaces, but we display 5 as default.
    3-2. The bar is slim and minimal, using the window's user's accent colors in different hues. More subtle for the background and grey-er for in-active workspace dots, normal accent color saturation for active dot.
    3-3. The status bar should only be a few "pixels" tall in total. Should have nice space around each dot, so width will be maybe 20-30 pixels.
    3-4. The window tiles will need to sit BELOW the bar, to prevent overlap.

## CodeRabbit findings:

main.rs line 431
Refactor to eliminate special-case handling.

This special-case handling for ToggleStatusBar creates architectural inconsistency. The variable name wm_lock is misleading since it's just a mutable reference, not a lock. Consider moving statusbar_visible into WorkspaceManager to allow uniform handling through handle_action like all other hotkeys.

statusbar.rs line 8
Missing Drop implementation causes window handle leak.

When StatusBar is dropped, the underlying window handle (HWND) is not destroyed, leading to a resource leak. Implement the Drop trait to call DestroyWindow.

 pub struct StatusBar {
     hwnd: HWND,
 }
+
+impl Drop for StatusBar {
+    fn drop(&mut self) {
+        unsafe {
+            let _ = DestroyWindow(self.hwnd);
+        }
+    }
+}

statusbar.rs line 25
Avoid unwrap() to prevent potential panic.

GetModuleHandleW could theoretically fail. Since the constructor already returns Result, propagate this error instead of panicking.

-                Some(GetModuleHandleW(None).unwrap().into()),
+                Some(GetModuleHandleW(None).map_err(|e| format!("Failed to get module handle: {}", e))?.into()),

windows_lib.rs line 341
Add error handling and remove unused variable.

This function has several concerns:

Error handling inconsistency: Unlike other functions in this file (e.g., get_window_rect, close_window), errors from DwmGetColorizationColor are silently ignored. If the call fails, the function returns 0, which may be indistinguishable from a valid black color.

Unused variable: The opaque parameter is retrieved but never used.

-pub fn get_accent_color() -> u32 {
+/// Gets the Windows accent color and converts it to COLORREF format (0x00BBGGRR).
+pub fn get_accent_color() -> Result<u32, String> {
     let mut color = 0u32;
-    let mut opaque = BOOL(0);
     unsafe {
-        let _ = DwmGetColorizationColor(&mut color, &mut opaque);
+        DwmGetColorizationColor(&mut color, std::ptr::null_mut())
+            .map_err(|e| format!("Failed to get accent color: {}", e))?;
     }
     // color is 0xAARRGGBB. Convert to 0x00BBGGRR (COLORREF format)
     let r = (color >> 16) & 0xFF;
     let g = (color >> 8) & 0xFF;
     let b = color & 0xFF;
-    (b << 16) | (g << 8) | r
+    Ok((b << 16) | (g << 8) | r)
 }


windows_lib.rs line 354
Replace hardcoded size with size_of and add error handling.

Issues identified:

Hardcoded size: Line 360 uses 4 instead of std::mem::size_of::<u32>(). This is inconsistent with line 111 in the same file where size_of is used.

Missing error handling: Errors from DwmSetWindowAttribute are silently ignored, making debugging difficult if the operation fails.

Missing documentation: The expected color format (COLORREF: 0x00BBGGRR) should be documented.

+/// Sets the window border color.
+/// 
+/// # Arguments
+/// * `color` - Color in COLORREF format (0x00BBGGRR)
-pub fn set_window_border_color(hwnd: HWND, color: u32) {
+pub fn set_window_border_color(hwnd: HWND, color: u32) -> Result<(), String> {
     unsafe {
-        let _ = DwmSetWindowAttribute(
+        DwmSetWindowAttribute(
             hwnd,
             DWMWA_BORDER_COLOR,
             &color as *const _ as *const std::ffi::c_void,
-            4,
-        );
+            std::mem::size_of::<u32>() as u32,
+        )
+        .map_err(|e| format!("Failed to set window border color: {}", e))?;
+        Ok(())
     }
 }

windows_lib.rs line 365
Add error handling and document parameters.

Concerns:

Multiple ignored errors: This function silently ignores errors from SetWindowLongW, SetLayeredWindowAttributes, and SetWindowPos. If any of these operations fail, the window could be left in an inconsistent state (e.g., style flag set but transparency not applied).

Undocumented parameter: Line 380 uses COLORREF(0) as the color key parameter. This should be documented to clarify that it's unused when LWA_ALPHA is specified.

+/// Sets the window transparency level.
+/// 
+/// # Arguments
+/// * `alpha` - Transparency level (0 = fully transparent, 255 = fully opaque)
-pub fn set_window_transparency(hwnd: HWND, alpha: u8) {
+pub fn set_window_transparency(hwnd: HWND, alpha: u8) -> Result<(), String> {
     unsafe {
         let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE);
         if alpha == 255 {
-            let _ = SetWindowLongW(
+            SetWindowLongW(
                 hwnd,
                 GWL_EXSTYLE,
                 (ex_style as u32 & !WS_EX_LAYERED.0) as i32,
             );
         } else {
-            let _ = SetWindowLongW(
+            SetWindowLongW(
                 hwnd,
                 GWL_EXSTYLE,
                 (ex_style as u32 | WS_EX_LAYERED.0) as i32,
             );
-            let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), alpha, LWA_ALPHA);
+            // COLORREF(0) is unused when LWA_ALPHA flag is set
+            SetLayeredWindowAttributes(hwnd, COLORREF(0), alpha, LWA_ALPHA)
+                .map_err(|e| format!("Failed to set layered window attributes: {}", e))?;
         }
         // Force frame update
-        let _ = SetWindowPos(
+        SetWindowPos(
             hwnd,
             None,
             0,
             0,
             0,
             0,
             SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED | SWP_NOACTIVATE,
-        );
+        )
+        .map_err(|e| format!("Failed to update window frame: {}", e))?;
+        Ok(())
     }
 }
