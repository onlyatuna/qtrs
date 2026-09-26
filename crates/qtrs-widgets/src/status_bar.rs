use crate::widget::{Widget, WidgetBase};
use qtrs_core::event::{Event, EventKind};
use qtrs_core::object::{ObjectData, ObjectId, QObject};
use qtrs_core::signal::Signal;
use qtrs_gui::geometry::primitives::{Rect, Size};
use qtrs_gui::paint::Painter;
pub type QStatusBar = StatusBar;
pub struct StatusBar {
    base: WidgetBase,
    message: String,
    pub message_changed: Signal<String>,
}
impl StatusBar {
    pub fn new() -> Self {
        Self {
            base: WidgetBase::new(),
            message: String::new(),
            message_changed: Signal::new(),
        }
    }
    pub fn current_message(&self) -> &str {
        &self.message
    }
    pub fn show_message(&mut self, message: impl Into<String>) {
        self.message = message.into();
        self.message_changed.emit(&self.message);
        self.update()
    }
    pub fn clear_message(&mut self) {
        if !self.message.is_empty() {
            self.message.clear();
            self.message_changed.emit(&self.message);
            self.update()
        }
    }
}
impl Default for StatusBar {
    fn default() -> Self {
        Self::new()
    }
}
impl QObject for StatusBar {
    fn object_data(&self) -> &ObjectData {
        &self.base.object_data
    }
    fn object_data_mut(&mut self) -> &mut ObjectData {
        &mut self.base.object_data
    }
    fn event(&mut self, e: &mut Event) -> bool {
        matches!(e.kind, EventKind::UpdateRequest)
    }
}
impl Widget for StatusBar {
    fn id(&self) -> ObjectId {
        self.base.object_data.id
    }
    fn geometry(&self) -> Rect {
        self.base.geometry
    }
    fn set_geometry(&mut self, r: Rect) {
        self.base.geometry = r;
        self.update()
    }
    fn size_hint(&self) -> Size {
        Size::new(160, 24)
    }
    fn is_visible(&self) -> bool {
        self.base.visible
    }
    fn set_visible(&mut self, v: bool) {
        self.base.visible = v
    }
    fn is_enabled(&self) -> bool {
        self.base.enabled
    }
    fn set_enabled(&mut self, v: bool) {
        self.base.enabled = v
    }
    fn update(&mut self) {
        self.base.dirty = Some(Rect::new(
            0,
            0,
            self.base.geometry.width,
            self.base.geometry.height,
        ))
    }
    fn dirty_rect(&self) -> Option<Rect> {
        self.base.dirty
    }
    fn clear_dirty(&mut self) {
        self.base.dirty = None
    }
    fn layout(&self) -> Option<&dyn crate::layout::Layout> {
        None
    }
    fn layout_mut(&mut self) -> Option<&mut Box<dyn crate::layout::Layout>> {
        None
    }
    fn set_layout(&mut self, _: Box<dyn crate::layout::Layout>) {}
    fn parent_widget(&self) -> Option<crate::widget::WidgetWeak> {
        self.base.parent.clone()
    }
    fn set_parent_widget(&mut self, p: Option<crate::widget::WidgetWeak>) {
        self.base.parent = p
    }
    fn window_id(&self) -> Option<ObjectId> {
        self.base.window_id
    }
    fn set_window_id(&mut self, id: Option<ObjectId>) {
        self.base.window_id = id
    }
    fn children(&self) -> Vec<crate::widget::WidgetRef> {
        vec![]
    }
    fn add_child(&mut self, _: crate::widget::WidgetRef) {}
    fn remove_child(&mut self, _: ObjectId) {}
    fn paint_event(&mut self, _: &mut Painter) {}
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
