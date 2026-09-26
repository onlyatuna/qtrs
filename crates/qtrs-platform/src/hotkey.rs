use bitflags::bitflags;

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct HotkeyModifiers: u32 {
        const ALT = 0x0001;
        const CONTROL = 0x0002;
        const SHIFT = 0x0004;
        const WIN = 0x0008;
        const NO_REPEAT = 0x4000;
    }
}

pub trait PlatformHotkeyManager: Send + Sync {
    fn register_hotkey(
        &mut self,
        id: u32,
        modifiers: HotkeyModifiers,
        vk: u32,
    ) -> Result<(), &'static str>;

    fn unregister_hotkey(&mut self, id: u32) -> Result<(), &'static str>;
}

#[cfg(windows)]
pub mod win32_hotkey {
    use super::*;
    use std::collections::HashSet;
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS,
    };

    pub struct Win32HotkeyManager {
        hwnd: HWND,
        registered_ids: HashSet<u32>,
    }

    unsafe impl Send for Win32HotkeyManager {}
    unsafe impl Sync for Win32HotkeyManager {}

    impl Win32HotkeyManager {
        pub fn new(hwnd: HWND) -> Self {
            Self {
                hwnd,
                registered_ids: HashSet::new(),
            }
        }

        pub fn registered_ids(&self) -> &HashSet<u32> {
            &self.registered_ids
        }
    }

    impl PlatformHotkeyManager for Win32HotkeyManager {
        fn register_hotkey(
            &mut self,
            id: u32,
            modifiers: HotkeyModifiers,
            vk: u32,
        ) -> Result<(), &'static str> {
            let win_mod: HOT_KEY_MODIFIERS = modifiers.bits();
            let success = unsafe { RegisterHotKey(self.hwnd, id as i32, win_mod, vk) };
            if success != 0 {
                self.registered_ids.insert(id);
                Ok(())
            } else {
                Err("RegisterHotKey failed")
            }
        }

        fn unregister_hotkey(&mut self, id: u32) -> Result<(), &'static str> {
            let success = unsafe { UnregisterHotKey(self.hwnd, id as i32) };
            self.registered_ids.remove(&id);
            if success != 0 {
                Ok(())
            } else {
                Err("UnregisterHotKey failed")
            }
        }
    }

    impl Drop for Win32HotkeyManager {
        fn drop(&mut self) {
            let ids: Vec<u32> = self.registered_ids.iter().copied().collect();
            for id in ids {
                let _ = self.unregister_hotkey(id);
            }
        }
    }
}

#[cfg(windows)]
pub use win32_hotkey::Win32HotkeyManager;

#[derive(Debug, Default)]
pub struct GenericHotkeyManager {
    registered_ids: std::collections::HashSet<u32>,
}

impl GenericHotkeyManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn registered_ids(&self) -> &std::collections::HashSet<u32> {
        &self.registered_ids
    }
}

impl PlatformHotkeyManager for GenericHotkeyManager {
    fn register_hotkey(
        &mut self,
        id: u32,
        _modifiers: HotkeyModifiers,
        _vk: u32,
    ) -> Result<(), &'static str> {
        self.registered_ids.insert(id);
        Ok(())
    }

    fn unregister_hotkey(&mut self, id: u32) -> Result<(), &'static str> {
        self.registered_ids.remove(&id);
        Ok(())
    }
}
