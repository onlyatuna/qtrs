use std::sync::atomic::{AtomicU8, Ordering};
use qtrs_core::signal::Signal;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ColorScheme {
    Unknown = 0,
    Light = 1,
    Dark = 2,
}

impl From<u8> for ColorScheme {
    fn from(val: u8) -> Self {
        match val {
            1 => ColorScheme::Light,
            2 => ColorScheme::Dark,
            _ => ColorScheme::Unknown,
        }
    }
}

pub trait PlatformTheme: Send + Sync {
    fn color_scheme(&self) -> ColorScheme;
    fn theme_changed(&self) -> &Signal<ColorScheme>;
    fn refresh(&self);
}

#[cfg(windows)]
pub mod win32_theme {
    use super::*;
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, KEY_READ,
        REG_DWORD,
    };

    pub struct Win32Theme {
        cached_scheme: AtomicU8,
        theme_changed_signal: Signal<ColorScheme>,
    }

    impl Default for Win32Theme {
        fn default() -> Self {
            let initial = Self::query_color_scheme();
            Self {
                cached_scheme: AtomicU8::new(initial as u8),
                theme_changed_signal: Signal::new(),
            }
        }
    }

    impl Win32Theme {
        pub fn new() -> Self {
            Self::default()
        }

        pub fn query_color_scheme() -> ColorScheme {
            let subkey: Vec<u16> = "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let value_name: Vec<u16> = "AppsUseLightTheme"
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();

            unsafe {
                let mut hkey: HKEY = std::ptr::null_mut();
                if RegOpenKeyExW(
                    HKEY_CURRENT_USER,
                    subkey.as_ptr(),
                    0,
                    KEY_READ,
                    &mut hkey,
                ) != 0
                {
                    return ColorScheme::Unknown;
                }

                let mut val_type: u32 = 0;
                let mut data: u32 = 0;
                let mut data_size: u32 = std::mem::size_of::<u32>() as u32;

                let status = RegQueryValueExW(
                    hkey,
                    value_name.as_ptr(),
                    std::ptr::null_mut(),
                    &mut val_type,
                    &mut data as *mut u32 as *mut u8,
                    &mut data_size,
                );

                RegCloseKey(hkey);

                if status == 0 && val_type == REG_DWORD {
                    if data == 0 {
                        ColorScheme::Dark
                    } else {
                        ColorScheme::Light
                    }
                } else {
                    ColorScheme::Unknown
                }
            }
        }
    }

    impl PlatformTheme for Win32Theme {
        fn color_scheme(&self) -> ColorScheme {
            ColorScheme::from(self.cached_scheme.load(Ordering::Acquire))
        }

        fn theme_changed(&self) -> &Signal<ColorScheme> {
            &self.theme_changed_signal
        }

        fn refresh(&self) {
            let current = Self::query_color_scheme();
            let previous = ColorScheme::from(self.cached_scheme.swap(current as u8, Ordering::AcqRel));
            if current != previous {
                self.theme_changed_signal.emit(&current);
            }
        }
    }
}

#[cfg(windows)]
pub use win32_theme::Win32Theme;

pub struct GenericTheme {
    cached_scheme: AtomicU8,
    theme_changed_signal: Signal<ColorScheme>,
}

impl Default for GenericTheme {
    fn default() -> Self {
        Self::new(ColorScheme::Light)
    }
}

impl GenericTheme {
    pub fn new(initial: ColorScheme) -> Self {
        Self {
            cached_scheme: AtomicU8::new(initial as u8),
            theme_changed_signal: Signal::new(),
        }
    }

    pub fn set_color_scheme(&self, scheme: ColorScheme) {
        let previous = ColorScheme::from(self.cached_scheme.swap(scheme as u8, Ordering::AcqRel));
        if scheme != previous {
            self.theme_changed_signal.emit(&scheme);
        }
    }
}

impl PlatformTheme for GenericTheme {
    fn color_scheme(&self) -> ColorScheme {
        ColorScheme::from(self.cached_scheme.load(Ordering::Acquire))
    }

    fn theme_changed(&self) -> &Signal<ColorScheme> {
        &self.theme_changed_signal
    }

    fn refresh(&self) {}
}
