// Backdrop materials module

/// Backdrop materials corresponding to Windows 11 / Windows 10 DWM visual styles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackdropType {
    /// Default window background (opaque or system-controlled).
    None,
    /// Windows 11 Mica material for main windows (`DWMSBT_MAINWINDOW`).
    Mica,
    /// Windows 11 Mica Alt material for tabbed windows (`DWMSBT_TABBEDWINDOW`).
    MicaAlt,
    /// Windows 11 / Windows 10 Desktop Acrylic blur effect (`DWMSBT_TRANSIENTWINDOW` or `ACCENT_ENABLE_ACRYLICBLURBEHIND`).
    Acrylic,
    /// Windows 7 / 10 Classic DWM Aero Glass blur behind (`DwmEnableBlurBehindWindow`).
    BlurBehind,
}

/// Applies the specified backdrop material effect to the window.
#[cfg(windows)]
pub fn set_window_backdrop(hwnd: windows_sys::Win32::Foundation::HWND, backdrop: BackdropType, dark_mode: bool) -> bool {
    use windows_sys::Win32::Graphics::Dwm::{
        DwmEnableBlurBehindWindow, DwmSetWindowAttribute, DWMWA_SYSTEMBACKDROP_TYPE,
        DWMWA_USE_IMMERSIVE_DARK_MODE, DWM_BB_ENABLE, DWM_BLURBEHIND,
    };
// unused import removed

    if hwnd.is_null() {
        return false;
    }

    unsafe {
        // First, configure dark mode preference if requested
        let dark: i32 = if dark_mode { 1 } else { 0 };
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE as u32,
            &dark as *const _ as *const _,
            std::mem::size_of::<i32>() as u32,
        );

        match backdrop {
            BackdropType::None => {
                let none_type: u32 = 1; // DWMSBT_NONE
                let _ = DwmSetWindowAttribute(
                    hwnd,
                    DWMWA_SYSTEMBACKDROP_TYPE as u32,
                    &none_type as *const _ as *const _,
                    std::mem::size_of::<u32>() as u32,
                );
                // Also disable blur behind if active
                let bb = DWM_BLURBEHIND {
                    dwFlags: DWM_BB_ENABLE,
                    fEnable: 0,
                    hRgnBlur: std::ptr::null_mut(),
                    fTransitionOnMaximized: 0,
                };
                let _ = DwmEnableBlurBehindWindow(hwnd, &bb);
                true
            }
            BackdropType::Mica => {
                // DWMSBT_MAINWINDOW = 2
                let mica_type: u32 = 2;
                let hr = DwmSetWindowAttribute(
                    hwnd,
                    DWMWA_SYSTEMBACKDROP_TYPE as u32,
                    &mica_type as *const _ as *const _,
                    std::mem::size_of::<u32>() as u32,
                );
                hr == 0
            }
            BackdropType::MicaAlt => {
                // DWMSBT_TABBEDWINDOW = 4
                let mica_alt_type: u32 = 4;
                let hr = DwmSetWindowAttribute(
                    hwnd,
                    DWMWA_SYSTEMBACKDROP_TYPE as u32,
                    &mica_alt_type as *const _ as *const _,
                    std::mem::size_of::<u32>() as u32,
                );
                hr == 0
            }
            BackdropType::Acrylic => {
                // Try modern Win11 22H2 DWMSBT_TRANSIENTWINDOW (Acrylic = 3)
                let acrylic_type: u32 = 3;
                let hr = DwmSetWindowAttribute(
                    hwnd,
                    DWMWA_SYSTEMBACKDROP_TYPE as u32,
                    &acrylic_type as *const _ as *const _,
                    std::mem::size_of::<u32>() as u32,
                );
                if hr == 0 {
                    return true;
                }

                // Fallback to Win10 SetWindowCompositionAttribute (Acrylic)
                set_win10_acrylic(hwnd, if dark_mode { 0x99202020 } else { 0x99f0f0f0 })
            }
            BackdropType::BlurBehind => {
                let bb = DWM_BLURBEHIND {
                    dwFlags: DWM_BB_ENABLE,
                    fEnable: 1,
                    hRgnBlur: std::ptr::null_mut(),
                    fTransitionOnMaximized: 0,
                };
                DwmEnableBlurBehindWindow(hwnd, &bb) == 0
            }
        }
    }
}

#[cfg(windows)]
fn set_win10_acrylic(hwnd: windows_sys::Win32::Foundation::HWND, gradient_color: u32) -> bool {
    use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryA};

    #[repr(C)]
    struct AccentPolicy {
        accent_state: u32,
        accent_flags: u32,
        gradient_color: u32,
        animation_id: u32,
    }

    #[repr(C)]
    struct WindowCompositionAttributeData {
        attribute: u32,
        data: *mut AccentPolicy,
        size_of_data: usize,
    }

    type SetWindowCompositionAttributeFn = unsafe extern "system" fn(
        windows_sys::Win32::Foundation::HWND,
        *mut WindowCompositionAttributeData,
    ) -> i32;
    unsafe {
        let user32 = LoadLibraryA(b"user32.dll\0".as_ptr());
        if user32.is_null() {
            return false;
        }
        let fn_ptr = GetProcAddress(user32, b"SetWindowCompositionAttribute\0".as_ptr());
        if let Some(set_wca) = fn_ptr {
            let set_wca: SetWindowCompositionAttributeFn = std::mem::transmute(set_wca);
            let mut policy = AccentPolicy {
                accent_state: 4, // ACCENT_ENABLE_ACRYLICBLURBEHIND
                accent_flags: 2, // Draw gradient color
                gradient_color,
                animation_id: 0,
            };
            let mut data = WindowCompositionAttributeData {
                attribute: 19, // WCA_ACCENT_POLICY
                data: &mut policy,
                size_of_data: std::mem::size_of::<AccentPolicy>(),
            };
            set_wca(hwnd, &mut data) != 0
        } else {
            false
        }
    }
}

#[cfg(not(windows))]
pub fn set_window_backdrop(_handle: isize, _backdrop: BackdropType, _dark_mode: bool) -> bool {
    false
}
