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

/// Radio button control for single-selection choice (`QRadioButton`).
pub struct RadioButton {
    base: WidgetBase,
    text: String,
    checked: bool,
    font: Font,
    pub group_id: Option<i32>,

    // Colors
    text_color: Color,
    circle_bg_color: Color,
    circle_border_color: Color,
    indicator_color: Color,
    focus_ring_color: Color,

    // Signals
    pub toggled: Signal<bool>,
}

impl RadioButton {
    /// Creates a new radio button with the given text.
    pub fn new(text: impl Into<String>) -> Self {
        let text_str = text.into();
        let font = Font::new("Segoe UI", 13.0);
        let mut base = WidgetBase::new();
        base.focus_policy = FocusPolicy::StrongFocus;
        base.size_policy = QSizePolicy::new(Policy::Preferred, Policy::Fixed);

        let metrics = FontMetrics::from_font(&font);
        let text_w = metrics.horizontal_advance(&text_str, &font).ceil() as i32;
        let text_h = metrics.height.ceil() as i32;
        base.geometry = Rect::new(0, 0, 16 + 8 + text_w, text_h.max(20));

        Self {
            base,
            text: text_str,
            checked: false,
            font,
            group_id: None,

            text_color: Color::from_rgba8(20, 20, 20, 255),
            circle_bg_color: Color::from_rgba8(255, 255, 255, 255),
            circle_border_color: Color::from_rgba8(160, 160, 160, 255),
            indicator_color: Color::from_rgba8(0, 120, 215, 255),
            focus_ring_color: Color::from_rgba8(0, 120, 215, 180),

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
        self.checked
    }

    pub fn set_checked(&mut self, checked: bool) {
        if self.checked != checked {
            self.checked = checked;
            self.toggled.emit(&checked);
            self.update();

            // If checked and has parent, uncheck other RadioButtons in same parent
            if checked {
                if let Some(parent_w) = self.base.parent.clone() {
                    if let Some(parent_rc) = parent_w.upgrade() {
                        let self_id = self.base.object_data.id;
                        let siblings = parent_rc.borrow().children();
                        for sib in siblings {
                            if sib.borrow().id() != self_id {
                                if let Some(rb) = sib.borrow_mut().as_any_mut().downcast_mut::<RadioButton>() {
                                    if rb.is_checked() {
                                        rb.set_checked(false);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn click(&mut self) {
        if self.base.enabled && !self.checked {
            self.set_checked(true);
        }
    }
}

impl QObject for RadioButton {
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
                    self.click();
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
                    self.click();
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

impl Widget for RadioButton {
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
            self.click();
        }
    }

    fn key_press_event(&mut self, key: u32, _modifiers: u32, _is_repeat: bool) {
        if key == 0x20 {
            self.click();
        }
    }

    fn paint_event(&mut self, painter: &mut Painter) {
        let geom = self.base.geometry;
        let diameter = 16.0f32;
        let radius = diameter / 2.0;
        let center_y = (geom.height as f32) / 2.0;
        let center_x = radius;

        // 1. Draw outer circle
        painter.set_brush(Brush::Color(self.circle_bg_color));
        let border_c = if self.has_focus() {
            self.focus_ring_color
        } else {
            self.circle_border_color
        };
        painter.set_pen(Pen::new(border_c, if self.has_focus() { 1.5 } else { 1.0 }));
        let r_outer = radius - 0.5;
        painter.draw_ellipse(RectF::new(center_x - r_outer, center_y - r_outer, r_outer * 2.0, r_outer * 2.0));

        // 2. Draw inner dot if checked
        if self.checked {
            painter.set_brush(Brush::Color(self.indicator_color));
            painter.set_pen(None);
            let r_inner = 4.0;
            painter.draw_ellipse(RectF::new(center_x - r_inner, center_y - r_inner, r_inner * 2.0, r_inner * 2.0));
        }

        // 3. Draw text label
        if !self.text.is_empty() {
            let metrics = FontMetrics::from_font(&self.font);
            let baseline_y = ((geom.height as f32 - metrics.height) / 2.0).max(0.0) + metrics.ascent;
            painter.set_pen(Pen::new(self.text_color, 1.0));
            painter.draw_text(PointF::new(diameter + 8.0, baseline_y), &self.text, &self.font);
        }

        // 4. Focus ring
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

/// Container to organize buttons into exclusive or non-exclusive groups (`QButtonGroup`).
pub struct ButtonGroup {
    exclusive: bool,
    buttons: Vec<(ObjectId, i32)>,
    checked_id: Option<i32>,
    pub button_clicked: Signal<i32>,
    pub button_toggled: Signal<(i32, bool)>,
}

impl ButtonGroup {
    /// Creates a new button group with exclusive selection enabled by default.
    pub fn new() -> Self {
        Self {
            exclusive: true,
            buttons: Vec::new(),
            checked_id: None,
            button_clicked: Signal::new(),
            button_toggled: Signal::new(),
        }
    }

    pub fn is_exclusive(&self) -> bool {
        self.exclusive
    }

    pub fn set_exclusive(&mut self, exclusive: bool) {
        self.exclusive = exclusive;
    }

    /// Adds a button to the group with an assigned integer ID.
    pub fn add_button(&mut self, button_id: ObjectId, id: i32) {
        self.buttons.retain(|(b_id, _)| *b_id != button_id);
        self.buttons.push((button_id, id));
    }

    /// Removes a button from the group.
    pub fn remove_button(&mut self, button_id: ObjectId) {
        self.buttons.retain(|(b_id, _)| *b_id != button_id);
        if let Some(id) = self.checked_id {
            if !self.buttons.iter().any(|(_, i)| *i == id) {
                self.checked_id = None;
            }
        }
    }

    /// Returns the ID of the currently checked button, if any.
    pub fn checked_id(&self) -> Option<i32> {
        self.checked_id
    }

    /// Notifies the group that a button state changed.
    pub fn set_checked(&mut self, id: i32, checked: bool) {
        if checked {
            if self.checked_id != Some(id) {
                self.checked_id = Some(id);
                self.button_clicked.emit(&id);
                self.button_toggled.emit(&(id, true));
            }
        } else if self.checked_id == Some(id) {
            self.checked_id = None;
            self.button_toggled.emit(&(id, false));
        }
    }

    /// Returns the assigned ID for a given ObjectId.
    pub fn id_of(&self, button_id: ObjectId) -> Option<i32> {
        self.buttons.iter().find(|(b_id, _)| *b_id == button_id).map(|(_, id)| *id)
    }
}

impl Default for ButtonGroup {
    fn default() -> Self {
        Self::new()
    }
}
