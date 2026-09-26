use std::sync::Arc;
use qtrs_core::signal::Signal;
use qtrs_gui::geometry::primitives::Point;

#[cfg(windows)]
pub mod win32_menu;
pub mod dbus_menu;
pub mod cocoa_menu;

#[cfg(windows)]
pub use win32_menu::{Win32Menu, Win32MenuItem};

pub use dbus_menu::{DBusMenu, DBusMenuItem, DBusMenuLayoutNode, DBusMenuPropValue};
pub use cocoa_menu::{CocoaMenu, CocoaMenuItem};

pub trait PlatformMenuItem: Send + Sync {
    fn id(&self) -> u32;
    fn text(&self) -> String;
    fn set_text(&mut self, text: &str);

    fn is_separator(&self) -> bool;

    fn is_checkable(&self) -> bool;
    fn is_checked(&self) -> bool;
    fn set_checked(&mut self, checked: bool);

    fn is_enabled(&self) -> bool;
    fn set_enabled(&mut self, enabled: bool);

    fn activated(&self) -> &Signal<()>;
}

pub trait PlatformMenu: Send + Sync {
    fn add_action(&mut self, id: u32, text: &str) -> Arc<dyn PlatformMenuItem>;
    fn add_checkable(&mut self, id: u32, text: &str, checked: bool) -> Arc<dyn PlatformMenuItem>;
    fn add_separator(&mut self);
    fn add_submenu(&mut self, text: &str, submenu: Box<dyn PlatformMenu>);
    fn show_popup(&self, screen_pos: Point);
    fn dismiss(&self);
    fn native_handle(&self) -> isize {
        0
    }
}
