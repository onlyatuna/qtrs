use std::sync::{Arc, Mutex};
use qtrs_gui::geometry::primitives::{Point, Rect, RectF};
use crate::widget::{EmptyWidget, WidgetRef};
use qtrs_gui::paint::{Painter, Pixmap};
use qtrs_platform::{PlatformWindow, WindowFlags, WindowSystemEvent, WindowSystemEventHandler, platform};
use qtrs_core::object::{ObjectId, ObjectData, QObject, register_qobject, unregister_qobject};
use qtrs_core::event::{Event, EventKind};

use crate::hit_test::EventTreeDispatcher;

pub struct Window {
    object_data: ObjectData,
    platform_window: Box<dyn PlatformWindow>,
    root_widget: WidgetRef,
    backing_store: Pixmap,
    geometry: Rect,
}

impl Window {
    pub fn new(
        title: &str,
        geometry: Rect,
        flags: WindowFlags,
    ) -> Result<Self, &'static str> {
        let p = platform();
        let mut platform_win = p.create_window(title, geometry, flags)?;
        let dpr = p.primary_screen().device_pixel_ratio();
        let physical_w = ((geometry.width.max(1) as f32) * dpr).round() as u32;
        let physical_h = ((geometry.height.max(1) as f32) * dpr).round() as u32;
        let backing_store = Pixmap::with_dpr(physical_w, physical_h, dpr)
            .ok_or("Failed to create top-level window offscreen Pixmap backing store")?;
        let window_id = ObjectId::next();

        let root_widget: WidgetRef = std::rc::Rc::new(std::cell::RefCell::new(Box::new(EmptyWidget::with_geometry(
            Rect::new(0, 0, geometry.width, geometry.height),
        ))));
        root_widget.borrow_mut().set_window_id(Some(window_id));

        let root_clone = std::rc::Rc::clone(&root_widget);
        let shared_root = Arc::new(Mutex::new(root_clone));
        let handler = WindowEventHandler {
            root: shared_root,
            dispatcher: EventTreeDispatcher::new(),
        };
        platform_win.set_event_handler(Box::new(handler));

        let win = Self {
            object_data: ObjectData::new(window_id),
            platform_window: platform_win,
            root_widget,
            backing_store,
            geometry,
        };

        crate::application::Application::register_window(window_id);
        Ok(win)
    }

    /// Registers this window for QObject ID-based dispatch.
    ///
    /// # Safety
    /// Keep the window alive and unmoved on its registration thread until it is unregistered.
    /// Do not access it through aliases while registry callbacks run.
    pub unsafe fn register(&mut self) {
        // SAFETY: delegated to this method's caller contract.
        unsafe { register_qobject(self) };
    }

    pub fn id(&self) -> ObjectId {
        self.object_data.id
    }

    pub fn root_widget(&self) -> WidgetRef {
        std::rc::Rc::clone(&self.root_widget)
    }

    pub fn geometry(&self) -> Rect {
        self.geometry
    }

    pub fn set_geometry(&mut self, rect: Rect) {
        self.geometry = rect;
        self.platform_window.set_geometry(rect);

        let dpr = platform().primary_screen().device_pixel_ratio();
        let physical_w = ((rect.width.max(1) as f32) * dpr).round() as u32;
        let physical_h = ((rect.height.max(1) as f32) * dpr).round() as u32;
        if self.backing_store.physical_width() != physical_w || self.backing_store.physical_height() != physical_h {
            if let Some(new_pixmap) = Pixmap::with_dpr(physical_w, physical_h, dpr) {
                self.backing_store = new_pixmap;
            }
        }

        self.root_widget.borrow_mut().set_geometry(Rect::new(0, 0, rect.width, rect.height));
    }

    pub fn show(&mut self) {
        self.platform_window.show();
        self.render_and_present();
    }

    pub fn hide(&mut self) {
        self.platform_window.hide();
    }

    pub fn set_stays_on_top(&mut self, enabled: bool) {
        self.platform_window.set_stays_on_top(enabled);
    }

    pub fn set_click_through(&mut self, enabled: bool) {
        self.platform_window.set_click_through(enabled);
    }

    pub fn start_system_drag(&self) {
        self.platform_window.start_system_drag();
    }
    pub fn set_backdrop(&mut self, backdrop: qtrs_platform::backdrop::BackdropType, dark_mode: bool) -> bool {
        self.platform_window.set_backdrop(backdrop, dark_mode)
    }

    pub fn set_ime_focus(&mut self, pos: Point) {
        self.platform_window.set_ime_focus(pos);
    }

    pub fn enable_drop_target(&mut self, enabled: bool) -> bool {
        self.platform_window.enable_drop_target(enabled)
    }


    pub fn native_handle(&self) -> isize {
        self.platform_window.native_handle()
    }

    pub fn render_and_present(&mut self) {
        let root_geom = Rect::new(0, 0, self.geometry.width, self.geometry.height);
        let dirty = collect_dirty_region(&self.root_widget, Point::new(0, 0))
            .unwrap_or(root_geom)
            .intersected(&root_geom);

        if dirty.is_empty() {
            return;
        }

        self.backing_store.fill(qtrs_gui::tiny_skia::Color::TRANSPARENT);

        {
            let mut painter = Painter::begin(&mut self.backing_store);
            painter.set_clip_rect(RectF::new(
                dirty.x as f32,
                dirty.y as f32,
                dirty.width as f32,
                dirty.height as f32,
            ));
            render_widget_recursive(&self.root_widget, &mut painter, root_geom);
        }

        let _ = self.platform_window.present_dirty(&mut self.backing_store, 1.0, dirty);
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        crate::application::Application::unregister_window(self.object_data.id);
        // SAFETY: an unsafe registration caller must ensure no callbacks remain active at drop.
        unsafe { unregister_qobject(self.object_data.id) };
    }
}

impl QObject for Window {
    fn object_data(&self) -> &ObjectData {
        &self.object_data
    }

    fn object_data_mut(&mut self) -> &mut ObjectData {
        &mut self.object_data
    }

    fn as_qobject_any(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }

    fn as_qobject_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }

    fn event(&mut self, event: &mut Event) -> bool {
        match &event.kind {
            EventKind::UpdateRequest => {
                self.render_and_present();
                true
            }
            EventKind::DpiChanged { dpi_x, .. } => {
                let new_dpr = (*dpi_x as f32) / 96.0;
                let physical_w = ((self.geometry.width.max(1) as f32) * new_dpr).round() as u32;
                let physical_h = ((self.geometry.height.max(1) as f32) * new_dpr).round() as u32;
                if let Some(new_pixmap) = Pixmap::with_dpr(physical_w, physical_h, new_dpr) {
                    self.backing_store = new_pixmap;
                    // Invalidate and re-layout root widget tree
                    let mut root = self.root_widget.borrow_mut();
                    let root_w = self.geometry.width;
                    let root_h = self.geometry.height;
                    root.set_geometry(Rect::new(0, 0, root_w, root_h));
                    if let Some(layout) = root.layout_mut() {
                        layout.update_layout();
                    }
                    root.update();
                    drop(root);
                    self.render_and_present();
                }
                true
            }
            _ => false,
        }
    }
}

fn render_widget_recursive(
    widget_ref: &WidgetRef,
    painter: &mut Painter,
    viewport_rect: Rect,
) {
    let mut widget = widget_ref.borrow_mut();
    if !widget.is_visible() {
        return;
    }

    let geom = widget.geometry();
    painter.save();
    painter.translate(geom.x as f32, geom.y as f32);

    widget.paint_event(painter);

    let children = widget.children();
    drop(widget);

    for child in children {
        render_widget_recursive(&child, painter, viewport_rect);
    }

    painter.restore();
}

/// Recursively collects and unifies dirty rectangles across the widget tree.
pub fn collect_dirty_region(widget_ref: &WidgetRef, offset: Point) -> Option<Rect> {
    let mut w = widget_ref.borrow_mut();
    let geom = w.geometry();
    let current_offset = Point::new(offset.x + geom.x, offset.y + geom.y);

    let mut dirty_union = w.dirty_rect().map(|d| {
        Rect::new(current_offset.x + d.x, current_offset.y + d.y, d.width, d.height)
    });
    w.clear_dirty();

    let children = w.children();
    drop(w);

    for child in children {
        if let Some(child_dirty) = collect_dirty_region(&child, current_offset) {
            dirty_union = match dirty_union {
                Some(u) => Some(u.united(&child_dirty)),
                None => Some(child_dirty),
            };
        }
    }

    dirty_union
}

struct WindowEventHandler {
    root: Arc<Mutex<WidgetRef>>,
    dispatcher: EventTreeDispatcher,
}

unsafe impl Send for WindowEventHandler {}
unsafe impl Sync for WindowEventHandler {}

impl WindowSystemEventHandler for WindowEventHandler {
    fn handle_window_event(&mut self, event: WindowSystemEvent) {
        let root = self.root.lock().unwrap().clone();
        match event {
            WindowSystemEvent::MouseMove { pos, .. } => {
                let mut ev = Event::new_spontaneous(EventKind::MouseMove {
                    x: pos.x,
                    y: pos.y,
                });
                self.dispatcher.dispatch_event(&root, &mut ev);
            }
            WindowSystemEvent::MouseLeave => {
                self.dispatcher.handle_mouse_leave();
            }
            WindowSystemEvent::MousePress { pos, button, modifiers: _, .. } => {
                let btn = match button {
                    qtrs_platform::MouseButton::Left => 1,
                    qtrs_platform::MouseButton::Right => 2,
                    qtrs_platform::MouseButton::Middle => 3,
                    _ => 0,
                };
                let mut ev = Event::new_spontaneous(EventKind::MouseButtonPress {
                    x: pos.x,
                    y: pos.y,
                    button: btn,
                });
                self.dispatcher.dispatch_event(&root, &mut ev);
            }
            WindowSystemEvent::MouseRelease { pos, button, .. } => {
                let btn = match button {
                    qtrs_platform::MouseButton::Left => 1,
                    qtrs_platform::MouseButton::Right => 2,
                    qtrs_platform::MouseButton::Middle => 3,
                    _ => 0,
                };
                let mut ev = Event::new_spontaneous(EventKind::MouseButtonRelease {
                    x: pos.x,
                    y: pos.y,
                    button: btn,
                });
                self.dispatcher.dispatch_event(&root, &mut ev);
            }
            WindowSystemEvent::Wheel { pos, delta, modifiers, .. } => {
                let mut ev = Event::new_spontaneous(EventKind::Wheel {
                    x: pos.x,
                    y: pos.y,
                    pixel_delta_x: delta.x,
                    pixel_delta_y: delta.y,
                    angle_delta_x: 0,
                    angle_delta_y: delta.y,
                    modifiers: modifiers.bits(),
                });
                self.dispatcher.dispatch_event(&root, &mut ev);
            }
            WindowSystemEvent::Resize { size } => {
                let mut ev = Event::new_spontaneous(EventKind::Resize {
                    width: size.width,
                    height: size.height,
                    old_width: 0,
                    old_height: 0,
                });
                self.dispatcher.dispatch_event(&root, &mut ev);
            }
            WindowSystemEvent::KeyPress { key, modifiers, is_repeat } => {
                let mut ev = Event::new_spontaneous(EventKind::KeyPress {
                    key,
                    modifiers: modifiers.bits(),
                    is_repeat,
                });
                self.dispatcher.dispatch_event(&root, &mut ev);
            }
            WindowSystemEvent::KeyRelease { key, modifiers } => {
                let mut ev = Event::new_spontaneous(EventKind::KeyRelease {
                    key,
                    modifiers: modifiers.bits(),
                });
                self.dispatcher.dispatch_event(&root, &mut ev);
            }
            WindowSystemEvent::FocusIn => {
                let mut ev = Event::new_spontaneous(EventKind::FocusIn {
                    reason: qtrs_core::event::FocusReason::ActiveWindow,
                });
                self.dispatcher.dispatch_event(&root, &mut ev);
            }
            WindowSystemEvent::FocusOut => {
                let mut ev = Event::new_spontaneous(EventKind::FocusOut {
                    reason: qtrs_core::event::FocusReason::ActiveWindow,
                });
                self.dispatcher.dispatch_event(&root, &mut ev);
            }
            WindowSystemEvent::DpiChanged { dpi_x, dpi_y } => {
                let mut ev = Event::new_spontaneous(EventKind::DpiChanged { dpi_x, dpi_y });
                self.dispatcher.dispatch_event(&root, &mut ev);
            }
            WindowSystemEvent::InputMethod { commit_string, preedit_string, cursor_position } => {
                let mut ev = Event::new_spontaneous(EventKind::InputMethod {
                    commit_string,
                    preedit_string,
                    cursor_position,
                });
                self.dispatcher.dispatch_event(&root, &mut ev);
            }
            WindowSystemEvent::DragEnter { pos, formats, drop_action } => {
                let mut ev = Event::new_spontaneous(EventKind::DragEnter {
                    pos_x: pos.x,
                    pos_y: pos.y,
                    formats,
                    drop_action,
                });
                self.dispatcher.dispatch_event(&root, &mut ev);
            }
            WindowSystemEvent::DragMove { pos, drop_action } => {
                let mut ev = Event::new_spontaneous(EventKind::DragMove {
                    pos_x: pos.x,
                    pos_y: pos.y,
                    drop_action,
                });
                self.dispatcher.dispatch_event(&root, &mut ev);
            }
            WindowSystemEvent::DragLeave => {
                let mut ev = Event::new_spontaneous(EventKind::DragLeave);
                self.dispatcher.dispatch_event(&root, &mut ev);
            }
            WindowSystemEvent::Drop { pos, formats, data, drop_action } => {
                let mut ev = Event::new_spontaneous(EventKind::Drop {
                    pos_x: pos.x,
                    pos_y: pos.y,
                    formats,
                    data,
                    drop_action,
                });
                self.dispatcher.dispatch_event(&root, &mut ev);
            }
            _ => {}
        }
    }
}
