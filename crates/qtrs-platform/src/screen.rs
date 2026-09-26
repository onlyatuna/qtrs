use qtrs_gui::geometry::primitives::{Point, Rect};

pub trait PlatformScreen: Send + Sync {
    fn name(&self) -> String;
    fn geometry(&self) -> Rect;
    fn available_geometry(&self) -> Rect;
    fn is_primary(&self) -> bool;
    fn device_pixel_ratio(&self) -> f32;
}

#[cfg(windows)]
pub mod win32_screen {
    use super::*;
    use std::ptr;
    use windows_sys::core::BOOL;
    use windows_sys::Win32::Foundation::{LPARAM, POINT, RECT};
    use windows_sys::Win32::Graphics::Gdi::{
        EnumDisplayMonitors, GetMonitorInfoW, MonitorFromPoint, HDC, HMONITOR,
        MONITORINFO, MONITORINFOEXW, MONITOR_DEFAULTTONEAREST,
    };
    use windows_sys::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};

    #[derive(Debug, Clone)]
    pub struct Win32Screen {
        name: String,
        geometry: Rect,
        available_geometry: Rect,
        is_primary: bool,
        dpr: f32,
    }

    impl Win32Screen {
        pub fn from_hmonitor(h_monitor: HMONITOR) -> Option<Self> {
            if h_monitor.is_null() {
                return None;
            }

            unsafe {
                let mut info_ex: MONITORINFOEXW = std::mem::zeroed();
                info_ex.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;

                if GetMonitorInfoW(h_monitor, &mut info_ex as *mut _ as *mut MONITORINFO) == 0 {
                    return None;
                }

                let rc = info_ex.monitorInfo.rcMonitor;
                let rc_work = info_ex.monitorInfo.rcWork;

                let geometry = Rect::new(
                    rc.left,
                    rc.top,
                    (rc.right - rc.left).max(0),
                    (rc.bottom - rc.top).max(0),
                );

                let available_geometry = Rect::new(
                    rc_work.left,
                    rc_work.top,
                    (rc_work.right - rc_work.left).max(0),
                    (rc_work.bottom - rc_work.top).max(0),
                );

                let is_primary = (info_ex.monitorInfo.dwFlags & 1) != 0;

                let mut name_len = 0;
                while name_len < info_ex.szDevice.len() && info_ex.szDevice[name_len] != 0 {
                    name_len += 1;
                }
                let name = String::from_utf16_lossy(&info_ex.szDevice[..name_len]);

                let mut dpi_x = 96u32;
                let mut dpi_y = 96u32;
                let hr = GetDpiForMonitor(h_monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y);
                let dpr = if hr == 0 {
                    dpi_x as f32 / 96.0
                } else {
                    windows_sys::Win32::UI::HiDpi::GetDpiForSystem() as f32 / 96.0
                };

                Some(Self {
                    name,
                    geometry,
                    available_geometry,
                    is_primary,
                    dpr: dpr.max(1.0),
                })
            }
        }

        pub fn primary() -> Self {
            let pt = POINT { x: 0, y: 0 };
            let h_monitor = unsafe { MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST) };
            if let Some(screen) = Self::from_hmonitor(h_monitor) {
                return screen;
            }

            unsafe {
                use windows_sys::Win32::UI::WindowsAndMessaging::{
                    GetSystemMetrics, SystemParametersInfoW, SM_CXSCREEN, SM_CYSCREEN, SPI_GETWORKAREA,
                };
                let width = GetSystemMetrics(SM_CXSCREEN);
                let height = GetSystemMetrics(SM_CYSCREEN);
                let mut work_area: RECT = std::mem::zeroed();
                SystemParametersInfoW(
                    SPI_GETWORKAREA,
                    0,
                    &mut work_area as *mut _ as *mut _,
                    0,
                );
                let dpi = windows_sys::Win32::UI::HiDpi::GetDpiForSystem();
                let dpr = (dpi as f32 / 96.0).max(1.0);

                Self {
                    name: "Primary Monitor".to_string(),
                    geometry: Rect::new(0, 0, width, height),
                    available_geometry: Rect::new(
                        work_area.left,
                        work_area.top,
                        (work_area.right - work_area.left).max(0),
                        (work_area.bottom - work_area.top).max(0),
                    ),
                    is_primary: true,
                    dpr,
                }
            }
        }

        pub fn all_screens() -> Vec<Self> {
            let mut list = Vec::<Self>::new();
            unsafe {
                unsafe extern "system" fn monitor_enum_proc(
                    h_monitor: HMONITOR,
                    _hdc: HDC,
                    _rect: *mut RECT,
                    lparam: LPARAM,
                ) -> BOOL {
                    let list_ref = &mut *(lparam as *mut Vec<Win32Screen>);
                    if let Some(screen) = Win32Screen::from_hmonitor(h_monitor) {
                        if screen.is_primary {
                            list_ref.insert(0, screen);
                        } else {
                            list_ref.push(screen);
                        }
                    }
                    1
                }

                EnumDisplayMonitors(
                    ptr::null_mut(),
                    ptr::null(),
                    Some(monitor_enum_proc),
                    &mut list as *mut _ as LPARAM,
                );
            }

            if list.is_empty() {
                list.push(Self::primary());
            }

            list
        }

        pub fn screen_at(pos: Point) -> Option<Self> {
            let pt = POINT { x: pos.x, y: pos.y };
            let h_monitor = unsafe { MonitorFromPoint(pt, 0) };
            if !h_monitor.is_null() {
                Self::from_hmonitor(h_monitor)
            } else {
                None
            }
        }
    }

    impl PlatformScreen for Win32Screen {
        fn name(&self) -> String {
            self.name.clone()
        }

        fn geometry(&self) -> Rect {
            self.geometry
        }

        fn available_geometry(&self) -> Rect {
            self.available_geometry
        }

        fn is_primary(&self) -> bool {
            self.is_primary
        }

        fn device_pixel_ratio(&self) -> f32 {
            self.dpr
        }
    }
}

#[cfg(windows)]
pub use win32_screen::Win32Screen;

#[derive(Debug, Clone)]
pub struct GenericScreen {
    name: String,
    geometry: Rect,
    available_geometry: Rect,
    is_primary: bool,
    dpr: f32,
}

impl GenericScreen {
    pub fn new(
        name: impl Into<String>,
        geometry: Rect,
        available_geometry: Rect,
        is_primary: bool,
        dpr: f32,
    ) -> Self {
        Self {
            name: name.into(),
            geometry,
            available_geometry,
            is_primary,
            dpr,
        }
    }

    pub fn default_primary() -> Self {
        Self {
            name: "DefaultScreen".to_string(),
            geometry: Rect::new(0, 0, 1920, 1080),
            available_geometry: Rect::new(0, 0, 1920, 1040),
            is_primary: true,
            dpr: 1.0,
        }
    }
}

impl PlatformScreen for GenericScreen {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn geometry(&self) -> Rect {
        self.geometry
    }

    fn available_geometry(&self) -> Rect {
        self.available_geometry
    }

    fn is_primary(&self) -> bool {
        self.is_primary
    }

    fn device_pixel_ratio(&self) -> f32 {
        self.dpr
    }
}
