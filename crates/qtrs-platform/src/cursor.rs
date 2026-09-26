#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CursorShape {
    Arrow,
    UpArrow,
    Cross,
    Wait,
    IBeam,
    SizeVer,
    SizeHor,
    SizeBDiag,
    SizeFDiag,
    SizeAll,
    Blank,
    PointingHand,
    Busy,
}

pub trait PlatformCursor: Send + Sync {
    fn change_cursor(&mut self, shape: CursorShape);
}

#[cfg(windows)]
pub mod win32_cursor {
    use super::*;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        LoadCursorW, SetCursor, HCURSOR, IDC_APPSTARTING, IDC_ARROW, IDC_CROSS, IDC_HAND,
        IDC_IBEAM, IDC_NO, IDC_SIZEALL, IDC_SIZENESW, IDC_SIZENS, IDC_SIZENWSE, IDC_SIZEWE,
        IDC_UPARROW, IDC_WAIT,
    };

    pub struct Win32Cursor {
        current_shape: CursorShape,
    }

    impl Default for Win32Cursor {
        fn default() -> Self {
            Self::new()
        }
    }

    impl Win32Cursor {
        pub fn new() -> Self {
            Self {
                current_shape: CursorShape::Arrow,
            }
        }

        pub fn current_shape(&self) -> CursorShape {
            self.current_shape
        }

        pub fn set_shape(shape: CursorShape) {
            let idc = match shape {
                CursorShape::Arrow => IDC_ARROW,
                CursorShape::UpArrow => IDC_UPARROW,
                CursorShape::Cross => IDC_CROSS,
                CursorShape::Wait => IDC_WAIT,
                CursorShape::IBeam => IDC_IBEAM,
                CursorShape::SizeVer => IDC_SIZENS,
                CursorShape::SizeHor => IDC_SIZEWE,
                CursorShape::SizeBDiag => IDC_SIZENESW,
                CursorShape::SizeFDiag => IDC_SIZENWSE,
                CursorShape::SizeAll => IDC_SIZEALL,
                CursorShape::Blank => IDC_NO,
                CursorShape::PointingHand => IDC_HAND,
                CursorShape::Busy => IDC_APPSTARTING,
            };

            unsafe {
                let hcursor: HCURSOR = LoadCursorW(std::ptr::null_mut(), idc);
                if !hcursor.is_null() {
                    SetCursor(hcursor);
                }
            }
        }
    }

    impl PlatformCursor for Win32Cursor {
        fn change_cursor(&mut self, shape: CursorShape) {
            self.current_shape = shape;
            Self::set_shape(shape);
        }
    }
}

#[cfg(windows)]
pub use win32_cursor::Win32Cursor;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GenericCursor {
    current_shape: CursorShape,
}

impl Default for GenericCursor {
    fn default() -> Self {
        Self {
            current_shape: CursorShape::Arrow,
        }
    }
}

impl GenericCursor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn current_shape(&self) -> CursorShape {
        self.current_shape
    }
}

impl PlatformCursor for GenericCursor {
    fn change_cursor(&mut self, shape: CursorShape) {
        self.current_shape = shape;
    }
}
