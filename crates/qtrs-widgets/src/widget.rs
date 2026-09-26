use crate::accessibility::{AccessibleAction, AccessibleRole};
use crate::focus::FocusPolicy;
use crate::layout::Layout;
pub use crate::size_policy::{Policy, QSizePolicy};
use qtrs_core::event::{Event, EventKind, FocusReason};
use qtrs_core::event_loop::post_event_to_thread;
use qtrs_core::object::{ObjectData, ObjectId, QObject, ThreadId};
use qtrs_gui::geometry::primitives::{Point, Rect, RectF, Size};
use qtrs_gui::paint::Painter;
use std::cell::RefCell;
use std::rc::{Rc, Weak};

pub type WidgetRef = Rc<RefCell<Box<dyn Widget>>>;
pub type WidgetWeak = Weak<RefCell<Box<dyn Widget>>>;

pub trait Widget: QObject + 'static {
    fn id(&self) -> ObjectId;

    fn geometry(&self) -> Rect;

    fn set_geometry(&mut self, rect: Rect);

    fn size_hint(&self) -> Size {
        Size::new(100, 30)
    }

    fn minimum_size(&self) -> Size {
        Size::new(0, 0)
    }

    fn minimum_size_hint(&self) -> Size {
        self.minimum_size()
    }

    fn maximum_size(&self) -> Size {
        Size::new(16777215, 16777215)
    }

    fn size_policy(&self) -> QSizePolicy {
        QSizePolicy::default()
    }

    fn set_size_policy(&mut self, _policy: QSizePolicy) {}

    fn is_visible(&self) -> bool;

    fn set_visible(&mut self, visible: bool);

    fn is_enabled(&self) -> bool;

    fn set_enabled(&mut self, enabled: bool);

    fn update(&mut self);

    fn dirty_rect(&self) -> Option<Rect>;

    fn clear_dirty(&mut self);

    fn layout(&self) -> Option<&dyn Layout>;

    fn layout_mut(&mut self) -> Option<&mut Box<dyn Layout>>;

    fn set_layout(&mut self, layout: Box<dyn Layout>);

    fn parent_widget(&self) -> Option<WidgetWeak>;

    fn set_parent_widget(&mut self, parent: Option<WidgetWeak>);

    fn window_id(&self) -> Option<ObjectId>;

    fn set_window_id(&mut self, window_id: Option<ObjectId>);

    fn children(&self) -> Vec<WidgetRef>;

    fn add_child(&mut self, child: WidgetRef);

    fn remove_child(&mut self, child_id: ObjectId);

    /// Virtual paint event handler (`QWidget::paintEvent` equivalent).
    ///
    /// Called when the widget needs to repaint its content.
    /// The painter is pre-translated to the widget's local coordinates `(0, 0)`.
    fn paint_event(&mut self, _painter: &mut Painter) {}

    fn mouse_press_event(&mut self, _pos: Point, _button: u32, _modifiers: u32) {}

    fn mouse_release_event(&mut self, _pos: Point, _button: u32, _modifiers: u32) {}

    fn mouse_move_event(&mut self, _pos: Point) {}

    fn enter_event(&mut self, _pos: Point) {}

    fn leave_event(&mut self) {}

    fn wheel_event(&mut self, _pos: Point, _delta_y: i32, _modifiers: u32) {}

    fn resize_event(&mut self, _new_size: Size, _old_size: Size) {}

    fn focus_policy(&self) -> FocusPolicy {
        FocusPolicy::NoFocus
    }

    fn set_focus_policy(&mut self, _policy: FocusPolicy) {}

    fn has_focus(&self) -> bool {
        false
    }

    fn set_has_focus(&mut self, _focus: bool) {}

    fn accessible_role(&self) -> AccessibleRole {
        AccessibleRole::Custom
    }

    fn accessible_name(&self) -> String {
        QObject::object_name(self).unwrap_or_default().to_owned()
    }

    fn accessible_value(&self) -> Option<String> {
        None
    }

    fn accessible_actions(&self) -> Vec<AccessibleAction> {
        Vec::new()
    }
    fn perform_accessible_action(&mut self, _action: AccessibleAction) -> bool {
        false
    }

    fn focus_in_event(&mut self, _reason: FocusReason) {}

    fn focus_out_event(&mut self, _reason: FocusReason) {}

    fn key_press_event(&mut self, _key: u32, _modifiers: u32, _is_repeat: bool) {}

    fn key_release_event(&mut self, _key: u32, _modifiers: u32) {}

    fn as_any(&self) -> &dyn std::any::Any;

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

pub struct WidgetBase {
    pub object_data: ObjectData,
    pub geometry: Rect,
    pub visible: bool,
    pub enabled: bool,
    pub dirty: Option<Rect>,
    pub parent: Option<WidgetWeak>,
    pub window_id: Option<ObjectId>,
    pub children: Vec<WidgetRef>,
    pub layout: Option<Box<dyn Layout>>,
    pub focus_policy: FocusPolicy,
    pub has_focus: bool,
    pub size_policy: QSizePolicy,
}

impl WidgetBase {
    pub fn new() -> Self {
        let id = ObjectId::next();
        Self {
            object_data: ObjectData::new(id),
            geometry: Rect::new(0, 0, 100, 30),
            visible: true,
            enabled: true,
            dirty: Some(Rect::new(0, 0, 100, 30)),
            parent: None,
            window_id: None,
            children: Vec::new(),
            layout: None,
            focus_policy: FocusPolicy::NoFocus,
            has_focus: false,
            size_policy: QSizePolicy::default(),
        }
    }

    pub fn with_geometry(geometry: Rect) -> Self {
        let mut base = Self::new();
        base.geometry = geometry;
        base.dirty = Some(Rect::new(0, 0, geometry.width, geometry.height));
        base
    }

    pub fn add_child(&mut self, child: WidgetRef) {
        let child_id = child.borrow().id();
        self.children.retain(|c| c.borrow().id() != child_id);
        if let Some(win_id) = self.window_id {
            child.borrow_mut().set_window_id(Some(win_id));
        }
        self.children.push(child);
    }

    pub fn remove_child(&mut self, child_id: ObjectId) {
        self.children.retain(|c| c.borrow().id() != child_id);
    }
}

impl Default for WidgetBase {
    fn default() -> Self {
        Self::new()
    }
}

pub type PaintHandler = Box<dyn FnMut(&mut Painter) + 'static>;

pub struct EmptyWidget {
    pub base: WidgetBase,
    pub background_color: Option<qtrs_gui::tiny_skia::Color>,
    pub paint_handler: Option<PaintHandler>,
}

pub type CustomWidget = EmptyWidget;

impl EmptyWidget {
    pub fn new() -> Self {
        Self {
            base: WidgetBase::new(),
            background_color: None,
            paint_handler: None,
        }
    }

    pub fn with_geometry(geometry: Rect) -> Self {
        Self {
            base: WidgetBase::with_geometry(geometry),
            background_color: None,
            paint_handler: None,
        }
    }

    pub fn set_background_color(&mut self, color: Option<qtrs_gui::tiny_skia::Color>) {
        self.background_color = color;
        self.update();
    }

    pub fn set_paint_handler<F>(&mut self, handler: F)
    where
        F: FnMut(&mut Painter) + 'static,
    {
        self.paint_handler = Some(Box::new(handler));
        self.update();
    }

    pub fn with_paint_handler<F>(mut self, handler: F) -> Self
    where
        F: FnMut(&mut Painter) + 'static,
    {
        self.paint_handler = Some(Box::new(handler));
        self
    }
}

impl Default for EmptyWidget {
    fn default() -> Self {
        Self::new()
    }
}

impl QObject for EmptyWidget {
    fn object_data(&self) -> &ObjectData {
        &self.base.object_data
    }

    fn object_data_mut(&mut self) -> &mut ObjectData {
        &mut self.base.object_data
    }

    fn as_qobject_any(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }

    fn as_qobject_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }

    fn event(&mut self, event: &mut Event) -> bool {
        match &event.kind {
            EventKind::MouseMove { x, y } => {
                self.mouse_move_event(Point::new(*x, *y));
                true
            }
            EventKind::Enter { x, y } => {
                self.enter_event(Point::new(*x, *y));
                true
            }
            EventKind::Leave => {
                self.leave_event();
                true
            }
            EventKind::MouseButtonPress { x, y, button } => {
                self.mouse_press_event(Point::new(*x, *y), *button, 0);
                true
            }
            EventKind::MouseButtonRelease { x, y, button } => {
                self.mouse_release_event(Point::new(*x, *y), *button, 0);
                true
            }
            EventKind::Wheel {
                x,
                y,
                angle_delta_y,
                modifiers,
                ..
            } => {
                self.wheel_event(Point::new(*x, *y), *angle_delta_y, *modifiers);
                true
            }
            EventKind::Resize {
                width,
                height,
                old_width,
                old_height,
            } => {
                self.resize_event(
                    Size::new(*width, *height),
                    Size::new(*old_width, *old_height),
                );
                true
            }
            EventKind::FocusIn { reason } => {
                self.focus_in_event(*reason);
                true
            }
            EventKind::FocusOut { reason } => {
                self.focus_out_event(*reason);
                true
            }
            EventKind::KeyPress {
                key,
                modifiers,
                is_repeat,
            } => {
                self.key_press_event(*key, *modifiers, *is_repeat);
                true
            }
            EventKind::KeyRelease { key, modifiers } => {
                self.key_release_event(*key, *modifiers);
                true
            }
            _ => false,
        }
    }
}

impl Widget for EmptyWidget {
    fn id(&self) -> ObjectId {
        self.base.object_data.id
    }

    fn geometry(&self) -> Rect {
        self.base.geometry
    }

    fn set_geometry(&mut self, rect: Rect) {
        if self.base.geometry != rect {
            let old_size = Size::new(self.base.geometry.width, self.base.geometry.height);
            let new_size = Size::new(rect.width, rect.height);
            self.base.geometry = rect;
            self.resize_event(new_size, old_size);

            if let Some(layout) = self.base.layout.as_mut() {
                layout.set_geometry(Rect::new(0, 0, rect.width, rect.height));
            }
            self.update();
        }
    }

    fn is_visible(&self) -> bool {
        self.base.visible
    }

    fn set_visible(&mut self, visible: bool) {
        if self.base.visible != visible {
            self.base.visible = visible;
            self.update();
        }
    }

    fn is_enabled(&self) -> bool {
        self.base.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.base.enabled = enabled;
    }

    fn update(&mut self) {
        self.base.dirty = Some(Rect::new(
            0,
            0,
            self.base.geometry.width,
            self.base.geometry.height,
        ));

        let target_receiver = self.base.window_id.unwrap_or(self.base.object_data.id);
        let _ = post_event_to_thread(
            ThreadId::current(),
            target_receiver,
            Event::new(EventKind::UpdateRequest),
        );
    }

    fn dirty_rect(&self) -> Option<Rect> {
        self.base.dirty
    }

    fn clear_dirty(&mut self) {
        self.base.dirty = None;
    }

    fn layout(&self) -> Option<&dyn Layout> {
        self.base.layout.as_deref()
    }

    fn layout_mut(&mut self) -> Option<&mut Box<dyn Layout>> {
        self.base.layout.as_mut()
    }

    fn set_layout(&mut self, mut layout: Box<dyn Layout>) {
        layout.set_geometry(Rect::new(
            0,
            0,
            self.base.geometry.width,
            self.base.geometry.height,
        ));
        self.base.layout = Some(layout);
        self.update();
    }

    fn parent_widget(&self) -> Option<WidgetWeak> {
        self.base.parent.clone()
    }

    fn set_parent_widget(&mut self, parent: Option<WidgetWeak>) {
        self.base.parent = parent;
    }

    fn window_id(&self) -> Option<ObjectId> {
        self.base.window_id
    }

    fn set_window_id(&mut self, window_id: Option<ObjectId>) {
        self.base.window_id = window_id;
        for child in &self.base.children {
            child.borrow_mut().set_window_id(window_id);
        }
    }

    fn children(&self) -> Vec<WidgetRef> {
        self.base.children.clone()
    }

    fn add_child(&mut self, child: WidgetRef) {
        let child_id = child.borrow().id();
        self.base.children.retain(|c| c.borrow().id() != child_id);
        if let Some(win_id) = self.base.window_id {
            child.borrow_mut().set_window_id(Some(win_id));
        }
        self.base.children.push(child);
        self.update();
    }

    fn remove_child(&mut self, child_id: ObjectId) {
        self.base.children.retain(|c| c.borrow().id() != child_id);
        self.update();
    }

    fn focus_policy(&self) -> FocusPolicy {
        self.base.focus_policy
    }

    fn set_focus_policy(&mut self, policy: FocusPolicy) {
        self.base.focus_policy = policy;
    }

    fn has_focus(&self) -> bool {
        self.base.has_focus
    }

    fn set_has_focus(&mut self, focus: bool) {
        self.base.has_focus = focus;
    }

    fn size_policy(&self) -> QSizePolicy {
        self.base.size_policy
    }

    fn set_size_policy(&mut self, policy: QSizePolicy) {
        self.base.size_policy = policy;
    }

    fn paint_event(&mut self, painter: &mut Painter) {
        if let Some(color) = self.background_color {
            let brush = qtrs_gui::paint::Brush::Color(color);
            painter.set_brush(brush);
            painter.set_pen(None);
            let rect_f = RectF::new(
                0.0,
                0.0,
                self.base.geometry.width as f32,
                self.base.geometry.height as f32,
            );
            painter.draw_rect(rect_f);
        }
        if let Some(handler) = &mut self.paint_handler {
            handler(painter);
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
