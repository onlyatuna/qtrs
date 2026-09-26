use crate::accessibility::{AccessibleAction, AccessibleRole};
use crate::action::{Action, ActionRef};
use crate::layout::Layout;
use crate::style::{ButtonStyleOption, DefaultStyle, Style};
use crate::widget::{Widget, WidgetBase, WidgetRef, WidgetWeak};
use qtrs_core::event::{Event, EventKind};
use qtrs_core::object::{ObjectData, ObjectId, QObject};
use qtrs_core::signal::Signal;
use qtrs_gui::geometry::primitives::{Point, Rect, RectF, Size};
use qtrs_gui::paint::Painter;
use qtrs_gui::text::{Font, FontMetrics};
use qtrs_gui::tiny_skia::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonState {
    #[default]
    Normal,
    Hovered,
    Pressed,
}

pub struct Button {
    pub base: WidgetBase,
    text: String,
    font: Font,
    state: ButtonState,
    pub clicked: Signal<()>,
    normal_bg: Color,
    hover_bg: Color,
    press_bg: Color,
    text_color: Color,
    border_color: Color,
    border_radius: f32,
    action: Option<ActionRef>,
    style: std::rc::Rc<dyn Style>,
}

impl Button {
    pub fn new(text: impl Into<String>) -> Self {
        let text_str = text.into();
        let font = Font::new("Segoe UI", 13.0);
        let mut base = WidgetBase::new();
        base.focus_policy = crate::focus::FocusPolicy::StrongFocus;
        let metrics = FontMetrics::from_font(&font);
        let text_w = metrics.horizontal_advance(&text_str, &font).ceil() as i32 + 24;
        let text_h = metrics.height.ceil() as i32 + 14;
        base.geometry = Rect::new(0, 0, text_w.max(80), text_h.max(30));

        Self {
            base,
            text: text_str,
            font,
            state: ButtonState::Normal,
            clicked: Signal::new(),
            normal_bg: Color::from_rgba8(240, 240, 240, 255),
            hover_bg: Color::from_rgba8(225, 235, 245, 255),
            press_bg: Color::from_rgba8(200, 220, 240, 255),
            text_color: Color::from_rgba8(20, 20, 20, 255),
            border_color: Color::from_rgba8(200, 200, 200, 255),
            border_radius: 4.0,
            action: None,
            style: std::rc::Rc::new(DefaultStyle),
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.update();
    }

    pub fn state(&self) -> ButtonState {
        self.state
    }

    pub fn set_colors(&mut self, normal: Color, hover: Color, pressed: Color) {
        self.normal_bg = normal;
        self.hover_bg = hover;
        self.press_bg = pressed;
        self.update();
    }

    pub fn set_border_radius(&mut self, radius: f32) {
        self.border_radius = radius;
        self.update();
    }

    /// Shares this button with menus and tool bars; activating it triggers that action.
    pub fn set_action(&mut self, action: Option<ActionRef>) {
        if let Some(action) = &action {
            self.text = action.borrow().display_text();
        }
        self.action = action;
        self.update();
    }

    pub fn action(&self) -> Option<ActionRef> {
        self.action.clone()
    }

    /// Replaces the rendering policy used by this button.
    pub fn set_style(&mut self, style: std::rc::Rc<dyn Style>) {
        self.style = style;
        self.update();
    }

    pub fn style(&self) -> &dyn Style {
        self.style.as_ref()
    }

    fn activate(&self) {
        if let Some(action) = &self.action {
            if !action.borrow().is_enabled() {
                return;
            }
            Action::trigger(action);
        }
        self.clicked.emit(&());
    }
}

impl QObject for Button {
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
            EventKind::Enter { .. } => {
                self.enter_event(Point::new(0, 0));
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

impl Widget for Button {
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
        let contents = Size::new(
            metrics.horizontal_advance(&self.text, &self.font).ceil() as i32,
            metrics.height.ceil() as i32,
        );
        let style_metrics = self.style.metrics();
        let styled = self.style.size_from_contents(
            contents,
            style_metrics.button_horizontal_padding,
            style_metrics.button_vertical_padding,
        );
        Size::new(styled.width.max(80), styled.height.max(30))
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

    fn enter_event(&mut self, _pos: Point) {
        if self.base.enabled && self.state != ButtonState::Pressed {
            self.state = ButtonState::Hovered;
            self.update();
        }
    }

    fn leave_event(&mut self) {
        if self.base.enabled && self.state != ButtonState::Normal {
            self.state = ButtonState::Normal;
            self.update();
        }
    }

    fn mouse_press_event(&mut self, _pos: Point, button: u32, _modifiers: u32) {
        if self.base.enabled && button == 1 {
            self.state = ButtonState::Pressed;
            self.update();
        }
    }

    fn mouse_release_event(&mut self, pos: Point, button: u32, _modifiers: u32) {
        if self.base.enabled && button == 1 && self.state == ButtonState::Pressed {
            let in_bounds =
                Rect::new(0, 0, self.base.geometry.width, self.base.geometry.height).contains(pos);
            if in_bounds {
                self.activate();
            } else {
                self.state = ButtonState::Normal;
            }
            self.update();
        }
    }
    fn focus_policy(&self) -> crate::focus::FocusPolicy {
        self.base.focus_policy
    }

    fn set_focus_policy(&mut self, policy: crate::focus::FocusPolicy) {
        self.base.focus_policy = policy;
    }

    fn has_focus(&self) -> bool {
        self.base.has_focus
    }

    fn set_has_focus(&mut self, focus: bool) {
        self.base.has_focus = focus;
    }

    fn accessible_role(&self) -> AccessibleRole {
        AccessibleRole::Button
    }

    fn accessible_name(&self) -> String {
        self.text.clone()
    }

    fn accessible_actions(&self) -> Vec<AccessibleAction> {
        vec![AccessibleAction::Invoke, AccessibleAction::Focus]
    }

    fn perform_accessible_action(&mut self, action: AccessibleAction) -> bool {
        match action {
            AccessibleAction::Invoke if self.base.enabled => {
                self.activate();
                true
            }
            AccessibleAction::Focus if self.base.enabled => {
                self.set_has_focus(true);
                true
            }
            _ => false,
        }
    }

    fn focus_in_event(&mut self, _reason: qtrs_core::event::FocusReason) {
        self.update();
    }

    fn focus_out_event(&mut self, _reason: qtrs_core::event::FocusReason) {
        self.update();
    }

    fn key_press_event(&mut self, key: u32, _modifiers: u32, _is_repeat: bool) {
        if self.base.enabled
            && (key == 0x20 || key == 0x0D || key == 0x01000004 || key == 0x01000005)
        {
            self.state = ButtonState::Pressed;
            self.update();
        }
    }

    fn key_release_event(&mut self, key: u32, _modifiers: u32) {
        if self.base.enabled
            && (key == 0x20 || key == 0x0D || key == 0x01000004 || key == 0x01000005)
        {
            if self.state == ButtonState::Pressed {
                self.activate();
                self.update();
            }
        }
    }

    fn paint_event(&mut self, painter: &mut Painter) {
        let geom = self.base.geometry;
        self.style.draw_button(
            painter,
            &ButtonStyleOption {
                rect: RectF::new(0.0, 0.0, geom.width as f32, geom.height as f32),
                text: &self.text,
                font: &self.font,
                state: self.state,
                enabled: self.base.enabled,
                focused: self.has_focus(),
                normal_color: self.normal_bg,
                hover_color: self.hover_bg,
                pressed_color: self.press_bg,
                text_color: self.text_color,
                border_color: self.border_color,
                border_radius: self.border_radius,
            },
        );
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
