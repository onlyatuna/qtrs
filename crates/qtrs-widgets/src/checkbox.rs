use qtrs_core::event::{Event, EventKind, FocusReason};
use qtrs_core::object::{ObjectData, ObjectId, QObject};
use qtrs_core::signal::Signal;
use qtrs_gui::geometry::primitives::{Point, PointF, Rect, RectF, Size};
use qtrs_gui::paint::brush::Brush;
use qtrs_gui::paint::painter::{Painter, Pen};
use qtrs_gui::text::{Font, FontMetrics};
use qtrs_gui::tiny_skia::Color;
use crate::focus::FocusPolicy;
use crate::layout::Layout;
use crate::size_policy::{Policy, QSizePolicy};
use crate::widget::{Widget, WidgetBase, WidgetRef, WidgetWeak};

/// Tri-state check state for CheckBox (`Qt::CheckState`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CheckState {
    #[default]
    Unchecked,
    PartiallyChecked,
    Checked,
}

/// Checkbox control with text label (`QCheckBox`).
pub struct CheckBox {
    base: WidgetBase,
    text: String,
    state: CheckState,
    tristate: bool,
    font: Font,

    // Colors
    text_color: Color,
    box_bg_color: Color,
    box_border_color: Color,
    check_color: Color,
    focus_ring_color: Color,

    // Signals
    pub state_changed: Signal<CheckState>,
    pub toggled: Signal<bool>,
}

impl CheckBox {
    /// Creates a new checkbox with specified label text.
    pub fn new(text: impl Into<String>) -> Self {
        let text_str = text.into();
        let font = Font::new("Segoe UI", 13.0);
        let mut base = WidgetBase::new();
        base.focus_policy = FocusPolicy::StrongFocus;
        base.size_policy = QSizePolicy::new(Policy::Preferred, Policy::Fixed);

        let metrics = FontMetrics::from_font(&font);
        let text_w = metrics.horizontal_advance(&text_str, &font).ceil() as i32;
        let text_h = metrics.height.ceil() as i32;
        base.geometry = Rect::new(0, 0, 20 + 8 + text_w, text_h.max(20));

        Self {
            base,
            text: text_str,
            state: CheckState::Unchecked,
            tristate: false,
            font,

            text_color: Color::from_rgba8(20, 20, 20, 255),
            box_bg_color: Color::from_rgba8(255, 255, 255, 255),
            box_border_color: Color::from_rgba8(160, 160, 160, 255),
            check_color: Color::from_rgba8(0, 120, 215, 255),
            focus_ring_color: Color::from_rgba8(0, 120, 215, 180),

            state_changed: Signal::new(),
            toggled: Signal::new(),
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.update();
    }

    pub fn is_checked(&self) -> bool {
        self.state == CheckState::Checked
    }

    pub fn set_checked(&mut self, checked: bool) {
        let new_state = if checked {
            CheckState::Checked
        } else {
            CheckState::Unchecked
        };
        self.set_check_state(new_state);
    }

    pub fn check_state(&self) -> CheckState {
        self.state
    }

    pub fn set_check_state(&mut self, state: CheckState) {
        if self.state != state {
            self.state = state;
            self.state_changed.emit(&state);
            self.toggled.emit(&(state == CheckState::Checked));
            self.update();
        }
    }

    pub fn is_tristate(&self) -> bool {
        self.tristate
    }

    pub fn set_tristate(&mut self, tristate: bool) {
        self.tristate = tristate;
    }

    pub fn toggle(&mut self) {
        if !self.base.enabled {
            return;
        }
        let next_state = if self.tristate {
            match self.state {
                CheckState::Unchecked => CheckState::PartiallyChecked,
                CheckState::PartiallyChecked => CheckState::Checked,
                CheckState::Checked => CheckState::Unchecked,
            }
        } else {
            match self.state {
                CheckState::Checked => CheckState::Unchecked,
                _ => CheckState::Checked,
            }
        };
        self.set_check_state(next_state);
    }
}

impl QObject for CheckBox {
    fn object_data(&self) -> &ObjectData {
        &self.base.object_data
    }

    fn object_data_mut(&mut self) -> &mut ObjectData {
        &mut self.base.object_data
    }

    fn event(&mut self, event: &mut Event) -> bool {
        match &event.kind {
            EventKind::MouseButtonPress { button, .. } => {
                if *button == 1 {
                    self.toggle();
                    true
                } else {
                    false
                }
            }
            EventKind::FocusIn { reason } => {
                self.focus_in_event(*reason);
                true
            }
            EventKind::FocusOut { reason } => {
                self.focus_out_event(*reason);
                true
            }
            EventKind::KeyPress { key, .. } => {
                if *key == 0x20 /* Space */ {
                    self.toggle();
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

impl Widget for CheckBox {
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
        let text_w = metrics.horizontal_advance(&self.text, &self.font).ceil() as i32;
        let text_h = metrics.height.ceil() as i32;
        Size::new(16 + 8 + text_w, text_h.max(18))
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
        self.update();
    }

    fn size_policy(&self) -> QSizePolicy {
        self.base.size_policy
    }

    fn set_size_policy(&mut self, policy: QSizePolicy) {
        self.base.size_policy = policy;
    }

    fn focus_in_event(&mut self, _reason: FocusReason) {
        self.update();
    }

    fn focus_out_event(&mut self, _reason: FocusReason) {
        self.update();
    }

    fn mouse_press_event(&mut self, _pos: Point, button: u32, _modifiers: u32) {
        if button == 1 {
            self.toggle();
        }
    }

    fn key_press_event(&mut self, key: u32, _modifiers: u32, _is_repeat: bool) {
        if key == 0x20 {
            self.toggle();
        }
    }

    fn paint_event(&mut self, painter: &mut Painter) {
        let geom = self.base.geometry;
        let box_size = 16.0f32;
        let box_y = ((geom.height as f32 - box_size) / 2.0).max(0.0);
        let box_rect = RectF::new(0.0, box_y, box_size, box_size);

        // 1. Draw Checkbox frame
        painter.set_brush(Brush::Color(self.box_bg_color));
        let border_c = if self.has_focus() {
            self.focus_ring_color
        } else {
            self.box_border_color
        };
        painter.set_pen(Pen::new(border_c, if self.has_focus() { 1.5 } else { 1.0 }));
        painter.draw_rounded_rect(box_rect, 3.0, 3.0);

        // 2. Draw Checkmark or Dash
        match self.state {
            CheckState::Checked => {
                painter.set_pen(Pen::new(self.check_color, 2.0));
                // Draw checkmark lines
                let p1 = PointF::new(3.5, box_y + 8.0);
                let p2 = PointF::new(6.5, box_y + 11.5);
                let p3 = PointF::new(12.5, box_y + 4.5);
                painter.draw_line(p1, p2);
                painter.draw_line(p2, p3);
            }
            CheckState::PartiallyChecked => {
                painter.set_brush(Brush::Color(self.check_color));
                painter.set_pen(None);
                painter.draw_rect(RectF::new(3.5, box_y + 6.5, 9.0, 3.0));
            }
            CheckState::Unchecked => {}
        }

        // 3. Draw text label
        if !self.text.is_empty() {
            let metrics = FontMetrics::from_font(&self.font);
            let baseline_y = ((geom.height as f32 - metrics.height) / 2.0).max(0.0) + metrics.ascent;
            painter.set_pen(Pen::new(self.text_color, 1.0));
            painter.draw_text(PointF::new(box_size + 8.0, baseline_y), &self.text, &self.font);
        }

        // 4. Focus ring around whole widget if focused
        if self.has_focus() {
            let focus_rect = RectF::new(0.0, 0.0, geom.width as f32, geom.height as f32);
            painter.set_brush(Brush::Color(Color::TRANSPARENT));
            painter.set_pen(Pen::new(self.focus_ring_color, 1.0));
            painter.draw_rounded_rect(focus_rect, 2.0, 2.0);
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
