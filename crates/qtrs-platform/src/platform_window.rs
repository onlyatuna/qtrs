pub trait PlatformWindow: Send + Sync {
    fn show(&self);
    fn hide(&self);
    fn geometry(&self) -> Rect;
    fn set_geometry(&mut self, rect: Rect);
    fn set_stays_on_top(&mut self, enabled: bool);
    fn set_click_through(&mut self, enabled: bool);
    fn start_system_drag(&self);
    fn present(&mut self, pixmap: &mut Pixmap, opacity: f32) -> Result<(), &'static str>;
    fn present_dirty(
        &mut self,
        pixmap: &mut Pixmap,
        opacity: f32,
        _dirty: Rect,
    ) -> Result<(), &'static str> {
        self.present(pixmap, opacity)
    }
    fn set_event_handler(&mut self, handler: Box<dyn WindowSystemEventHandler>);
    fn native_handle(&self) -> isize;
    fn poll_events(&mut self) -> usize {
        0
    }
    fn set_backdrop(&mut self, _backdrop: crate::backdrop::BackdropType, _dark_mode: bool) -> bool {
        false
    }
    fn set_ime_focus(&mut self, _pos: qtrs_gui::geometry::primitives::Point) {}
    fn enable_drop_target(&mut self, _enabled: bool) -> bool {
        false
    }
}

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use qtrs_gui::geometry::primitives::Rect;
use qtrs_gui::paint::Pixmap;
use crate::window::WindowFlags;
use crate::window_system_interface::WindowSystemEventHandler;

pub struct GenericWindow {
    geometry: Rect,
    visible: AtomicBool,
    stays_on_top: bool,
    click_through: bool,
    handler: Option<Box<dyn WindowSystemEventHandler>>,
    pending_events: Mutex<Vec<crate::window_system_interface::WindowSystemEvent>>,
    last_pixmap: Mutex<Option<Pixmap>>,
    opacity: f32,
}

impl GenericWindow {
    pub fn new(_title: &str, rect: Rect, flags: WindowFlags) -> Self {
        Self {
            geometry: rect,
            visible: AtomicBool::new(false),
            stays_on_top: flags.contains(WindowFlags::STAYS_ON_TOP),
            click_through: flags.contains(WindowFlags::CLICK_THROUGH),
            handler: None,
            pending_events: Mutex::new(Vec::new()),
            last_pixmap: Mutex::new(None),
            opacity: 1.0,
        }
    }

    pub fn is_visible(&self) -> bool {
        self.visible.load(Ordering::Acquire)
    }

    pub fn opacity(&self) -> f32 {
        self.opacity
    }

    pub fn queue_event(&self, event: crate::window_system_interface::WindowSystemEvent) {
        self.pending_events.lock().unwrap().push(event);
    }
}

impl PlatformWindow for GenericWindow {
    fn show(&self) {
        self.visible.store(true, Ordering::Release);
    }

    fn hide(&self) {
        self.visible.store(false, Ordering::Release);
    }

    fn geometry(&self) -> Rect {
        self.geometry
    }

    fn set_geometry(&mut self, rect: Rect) {
        self.geometry = rect;
    }

    fn set_stays_on_top(&mut self, enabled: bool) {
        self.stays_on_top = enabled;
    }

    fn set_click_through(&mut self, enabled: bool) {
        self.click_through = enabled;
    }

    fn start_system_drag(&self) {}

    fn present(&mut self, pixmap: &mut Pixmap, opacity: f32) -> Result<(), &'static str> {
        let mut guard = self.last_pixmap.lock().unwrap();
        *guard = Some((*pixmap).clone());
        self.opacity = opacity;
        Ok(())
    }

    fn set_event_handler(&mut self, handler: Box<dyn WindowSystemEventHandler>) {
        self.handler = Some(handler);
    }

    fn native_handle(&self) -> isize {
        0
    }

    fn poll_events(&mut self) -> usize {
        let events = std::mem::take(&mut *self.pending_events.lock().unwrap());
        let count = events.len();
        if let Some(handler) = self.handler.as_mut() {
            for event in events {
                handler.handle_window_event(event);
            }
        }
        count
    }
}
