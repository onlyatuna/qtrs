use qtrs_core::event::{Event, EventKind};
use qtrs_core::object::{ObjectData, ObjectId, QObject};
use qtrs_core::signal::Signal;
use qtrs_gui::geometry::primitives::{Rect, RectF, Size};
use qtrs_gui::paint::brush::Brush;
use qtrs_gui::paint::painter::{Painter, Pen};
use qtrs_gui::tiny_skia::Color;
use crate::focus::FocusPolicy;
use crate::layout::Layout;
use crate::size_policy::{Policy, QSizePolicy};
use crate::widget::{Widget, WidgetBase, WidgetRef, WidgetWeak};

/// Result code returned when a dialog finishes (`QDialog::DialogCode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DialogCode {
    #[default]
    Rejected = 0,
    Accepted = 1,
}

/// Modal or modeless dialog window with backdrop overlay and focus trapping (`QDialog`).
pub struct Dialog {
    base: WidgetBase,
    modal: bool,
    result: DialogCode,
    backdrop_color: Color,
    window_frame_color: Color,
    border_color: Color,
    border_radius: f32,

    pub finished: Signal<DialogCode>,
    pub accepted: Signal<()>,
    pub rejected: Signal<()>,
}

impl Dialog {
    /// Creates a new dialog.
    pub fn new() -> Self {
        let mut base = WidgetBase::new();
        base.focus_policy = FocusPolicy::StrongFocus;
        base.size_policy = QSizePolicy::new(Policy::Preferred, Policy::Preferred);
        base.geometry = Rect::new(50, 50, 320, 200);

        Self {
            base,
            modal: true,
            result: DialogCode::Rejected,
            backdrop_color: Color::from_rgba8(0, 0, 0, 100),
            window_frame_color: Color::from_rgba8(250, 250, 250, 255),
            border_color: Color::from_rgba8(200, 200, 200, 255),
            border_radius: 8.0,

            finished: Signal::new(),
            accepted: Signal::new(),
            rejected: Signal::new(),
        }
    }

    pub fn is_modal(&self) -> bool {
        self.modal
    }

    pub fn set_modal(&mut self, modal: bool) {
        self.modal = modal;
    }

    pub fn backdrop_color(&self) -> Color {
        self.backdrop_color
    }

    pub fn set_backdrop_color(&mut self, color: Color) {
        self.backdrop_color = color;
        self.update();
    }

    pub fn result(&self) -> DialogCode {
        self.result
    }

    pub fn accept(&mut self) {
        self.done(DialogCode::Accepted);
    }

    pub fn reject(&mut self) {
        self.done(DialogCode::Rejected);
    }

    pub fn done(&mut self, result: DialogCode) {
        self.result = result;
        self.set_visible(false);
        match result {
            DialogCode::Accepted => self.accepted.emit(&()),
            DialogCode::Rejected => self.rejected.emit(&()),
        }
        self.finished.emit(&result);
        self.update();
    }

    pub fn open(&mut self) {
        self.set_visible(true);
        self.update();
    }
}

impl Default for Dialog {
    fn default() -> Self {
        Self::new()
    }
}

impl QObject for Dialog {
    fn object_data(&self) -> &ObjectData {
        &self.base.object_data
    }

    fn object_data_mut(&mut self) -> &mut ObjectData {
        &mut self.base.object_data
    }

    fn event(&mut self, event: &mut Event) -> bool {
        match &event.kind {
            EventKind::KeyPress { key, .. } => {
                // Esc key rejects the dialog
                if *key == 0x1B || *key == 0x01000000 {
                    self.reject();
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    fn as_qobject_any(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }

    fn as_qobject_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
}

impl Widget for Dialog {
    fn id(&self) -> ObjectId {
        self.base.object_data.id
    }

    fn geometry(&self) -> Rect {
        self.base.geometry
    }

    fn set_geometry(&mut self, rect: Rect) {
        if self.base.geometry != rect {
            self.base.geometry = rect;
            if let Some(layout) = self.base.layout.as_mut() {
                layout.set_geometry(Rect::new(0, 0, rect.width, rect.height));
            }
            self.update();
        }
    }

    fn size_hint(&self) -> Size {
        Size::new(320, 200)
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
        self.update();
    }

    fn update(&mut self) {
        self.base.dirty = Some(self.base.geometry);
        let target_receiver = self.base.window_id.unwrap_or(self.base.object_data.id);
        let _ = qtrs_core::event_loop::post_event_to_thread(
            qtrs_core::object::ThreadId::current(),
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
        layout.set_geometry(Rect::new(0, 0, self.base.geometry.width, self.base.geometry.height));
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
        self.base.add_child(child);
    }

    fn remove_child(&mut self, child_id: ObjectId) {
        self.base.remove_child(child_id);
    }

    fn paint_event(&mut self, painter: &mut Painter) {
        let geom = self.base.geometry;
        let rect_f = RectF::new(0.0, 0.0, geom.width as f32, geom.height as f32);

        // Draw shadow / border and dialog body
        painter.set_brush(Brush::Color(self.window_frame_color));
        painter.set_pen(Pen::new(self.border_color, 1.0));
        painter.draw_rounded_rect(rect_f, self.border_radius, self.border_radius);
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
