use qtrs_core::object::{ObjectId, ObjectData, QObject};
use qtrs_core::event::Event;
use qtrs_gui::geometry::primitives::{PointF, Rect, Size};
use qtrs_gui::paint::{Painter, Pen};
use qtrs_gui::text::{Font, FontMetrics};
use qtrs_gui::tiny_skia::Color;
use crate::layout::Layout;
use crate::widget::{Widget, WidgetBase, WidgetRef, WidgetWeak};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Alignment {
    #[default]
    Left,
    Center,
    Right,
}

pub struct Label {
    pub base: WidgetBase,
    text: String,
    font: Font,
    color: Color,
    background_color: Option<Color>,
    alignment: Alignment,
}

impl Label {
    pub fn new(text: impl Into<String>) -> Self {
        let text_str = text.into();
        let font = Font::new("Segoe UI", 13.0);
        let mut base = WidgetBase::new();
        let metrics = FontMetrics::from_font(&font);
        let text_w = metrics.horizontal_advance(&text_str, &font).ceil() as i32 + 10;
        let text_h = metrics.height.ceil() as i32 + 6;
        base.geometry = Rect::new(0, 0, text_w.max(60), text_h.max(24));
        Self {
            base,
            text: text_str,
            font,
            color: Color::BLACK,
            background_color: None,
            alignment: Alignment::Left,
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.update();
    }

    pub fn color(&self) -> Color {
        self.color
    }

    pub fn set_color(&mut self, color: Color) {
        self.color = color;
        self.update();
    }

    pub fn font(&self) -> &Font {
        &self.font
    }

    pub fn set_font(&mut self, font: Font) {
        self.font = font;
        self.update();
    }

    pub fn set_background_color(&mut self, color: Option<Color>) {
        self.background_color = color;
        self.update();
    }

    pub fn alignment(&self) -> Alignment {
        self.alignment
    }

    pub fn set_alignment(&mut self, alignment: Alignment) {
        self.alignment = alignment;
        self.update();
    }
}

impl QObject for Label {
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

    fn event(&mut self, _event: &mut Event) -> bool {
        false
    }
}

impl Widget for Label {
    fn id(&self) -> ObjectId {
        self.base.object_data.id
    }

    fn geometry(&self) -> Rect {
        self.base.geometry
    }

    fn set_geometry(&mut self, rect: Rect) {
        if self.base.geometry != rect {
            self.base.geometry = rect;
            self.update();
        }
    }

    fn size_hint(&self) -> Size {
        let metrics = FontMetrics::from_font(&self.font);
        let text_w = metrics.horizontal_advance(&self.text, &self.font).ceil() as i32 + 10;
        let text_h = metrics.height.ceil() as i32 + 6;
        Size::new(text_w.max(40), text_h.max(20))
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
        self.base.dirty = Some(self.base.geometry);
        let target_receiver = self.base.window_id.unwrap_or(self.base.object_data.id);
        let _ = qtrs_core::event_loop::post_event_to_thread(
            qtrs_core::object::ThreadId::current(),
            target_receiver,
            Event::new(qtrs_core::event::EventKind::UpdateRequest),
        );
    }

    fn dirty_rect(&self) -> Option<Rect> {
        self.base.dirty
    }

    fn clear_dirty(&mut self) {
        self.base.dirty = None;
    }

    fn layout(&self) -> Option<&dyn Layout> {
        None
    }

    fn layout_mut(&mut self) -> Option<&mut Box<dyn Layout>> {
        None
    }

    fn set_layout(&mut self, _layout: Box<dyn Layout>) {}

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
    }

    fn children(&self) -> Vec<WidgetRef> {
        Vec::new()
    }

    fn add_child(&mut self, _child: WidgetRef) {}

    fn remove_child(&mut self, _child_id: ObjectId) {}

    fn paint_event(&mut self, painter: &mut Painter) {
        let geom = self.base.geometry;

        if let Some(bg) = self.background_color {
            painter.set_brush(qtrs_gui::paint::Brush::Color(bg));
            painter.set_pen(None);
            painter.draw_rect(qtrs_gui::geometry::primitives::RectF::new(
                0.0,
                0.0,
                geom.width as f32,
                geom.height as f32,
            ));
        }

        if self.text.is_empty() {
            return;
        }

        let metrics = FontMetrics::from_font(&self.font);
        let text_w = metrics.horizontal_advance(&self.text, &self.font);
        let text_h = metrics.height;
        let x = match self.alignment {
            Alignment::Left => 4.0,
            Alignment::Center => ((geom.width as f32 - text_w) / 2.0).max(0.0),
            Alignment::Right => (geom.width as f32 - text_w - 4.0).max(0.0),
        };

        let baseline_y = ((geom.height as f32 - text_h) / 2.0).max(0.0) + metrics.ascent;

        painter.set_pen(Pen::new(self.color, 1.0));
        painter.draw_text(PointF::new(x, baseline_y), &self.text, &self.font);
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
