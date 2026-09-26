//! Titled, optionally checkable group frame (`QGroupBox`).

use qtrs_core::event::{Event, EventKind, FocusReason};
use qtrs_core::object::{ObjectData, ObjectId, QObject};
use qtrs_core::signal::Signal;
use qtrs_gui::geometry::primitives::{Point, PointF, Rect, RectF, Size};
use qtrs_gui::paint::palette::{ColorGroup, ColorRole, Palette};
use qtrs_gui::paint::{Brush, Painter, Pen};
use qtrs_gui::text::{Font, FontMetrics};
use qtrs_gui::tiny_skia::Color;

use crate::focus::FocusPolicy;
use crate::frame::{FrameShadow, FrameShape, FrameStyle, DEFAULT_STYLED_FRAME_WIDTH};
use crate::input_keys::KEY_SPACE;
use crate::label::Alignment;
use crate::layout::Layout;
use crate::size_policy::{Policy, QSizePolicy};
use crate::widget::{Widget, WidgetBase, WidgetRef, WidgetWeak};

/// Check indicator side length (`QStyle::PM_IndicatorWidth`/`PM_IndicatorHeight`).
const INDICATOR_SIZE: i32 = 13;
/// Spacing between indicator and title (`QStyle::PM_CheckBoxLabelSpacing`).
const INDICATOR_SPACING: i32 = 6;
/// Horizontal inset of the title inside a non-flat frame.
const TITLE_MARGIN: i32 = 8;

/// Sub-control hit by a point (`QStyle::SubControl` for `CC_GroupBox`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GroupBoxSubControl {
    None,
    CheckBox,
    Label,
    Contents,
    Frame,
}

/// Frame with a title that groups child widgets (`QGroupBox`).
///
/// When checkable and unchecked, every enabled child is disabled; children that
/// were already disabled stay disabled after re-checking (Qt's `WA_ForceDisabled`).
pub struct GroupBox {
    pub base: WidgetBase,
    title: String,
    alignment: Alignment,
    flat: bool,
    checkable: bool,
    checked: bool,
    font: Font,
    palette: Palette,
    /// Mouse press started on the indicator or label (`overCheckBox`).
    pressed_title: bool,
    /// Space pressed while focused; released Space clicks.
    space_pressed: bool,
    /// Children this group box disabled because it was unchecked.
    auto_disabled: Vec<ObjectId>,

    /// Emitted with the new state whenever the check state changes (`QGroupBox::toggled`).
    pub toggled: Signal<bool>,
    /// Emitted with the new state when the user activates the check box (`QGroupBox::clicked`).
    pub clicked: Signal<bool>,
}

/// Canonical Qt alias.
pub type QGroupBox = GroupBox;

impl GroupBox {
    /// Creates a non-checkable group box with a left-aligned title.
    pub fn new(title: impl Into<String>) -> Self {
        let mut base = WidgetBase::new();
        base.size_policy = QSizePolicy::new(Policy::Preferred, Policy::Preferred);
        let mut group = Self {
            base,
            title: title.into(),
            alignment: Alignment::Left,
            flat: false,
            checkable: false,
            checked: true,
            font: Font::new("Segoe UI", 13.0),
            palette: Palette::light(),
            pressed_title: false,
            space_pressed: false,
            auto_disabled: Vec::new(),
            toggled: Signal::new(),
            clicked: Signal::new(),
        };
        let hint = group.minimum_size_hint();
        group.base.geometry = Rect::new(0, 0, hint.width.max(160), hint.height.max(80));
        group
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    /// Sets the title; `&` marks a mnemonic and `&&` a literal ampersand.
    pub fn set_title(&mut self, title: impl Into<String>) {
        let title = title.into();
        if self.title != title {
            self.title = title;
            self.relayout();
        }
    }

    /// Horizontal title alignment (`QGroupBox::alignment`).
    pub fn alignment(&self) -> Alignment {
        self.alignment
    }

    pub fn set_alignment(&mut self, alignment: Alignment) {
        self.alignment = alignment;
        self.update();
    }

    /// Flat group boxes draw only a top line and no side margins (`QGroupBox::flat`).
    pub fn is_flat(&self) -> bool {
        self.flat
    }

    pub fn set_flat(&mut self, flat: bool) {
        if self.flat != flat {
            self.flat = flat;
            self.relayout();
        }
    }

    pub fn is_checkable(&self) -> bool {
        self.checkable
    }

    /// Makes the title carry a check box (`QGroupBox::setCheckable`); becoming
    /// checkable checks the box, and children are re-enabled either way.
    pub fn set_checkable(&mut self, checkable: bool) {
        let was_checkable = self.checkable;
        self.checkable = checkable;
        if checkable {
            self.set_checked(true);
            if !was_checkable {
                self.base.focus_policy = FocusPolicy::StrongFocus;
                self.set_children_enabled(true);
            }
        } else {
            if was_checkable {
                self.base.focus_policy = FocusPolicy::NoFocus;
            }
            self.set_children_enabled(true);
        }
        if was_checkable != checkable {
            self.relayout();
        }
    }

    /// `true` only when checkable and checked (`QGroupBox::isChecked`).
    pub fn is_checked(&self) -> bool {
        self.checkable && self.checked
    }

    /// Changes the check state of a checkable group box, enabling/disabling
    /// children and emitting `toggled` (`QGroupBox::setChecked`).
    pub fn set_checked(&mut self, checked: bool) {
        if self.checkable && checked != self.checked {
            self.checked = checked;
            self.set_children_enabled(checked);
            self.update();
            self.toggled.emit(&checked);
        }
    }

    /// Toggles the check state as a user click would and emits `clicked` (`QGroupBoxPrivate::click`).
    pub fn click(&mut self) {
        if !self.checkable || !self.base.enabled {
            return;
        }
        self.set_checked(!self.checked);
        self.clicked.emit(&self.checked);
    }

    pub fn font(&self) -> &Font {
        &self.font
    }

    pub fn set_font(&mut self, font: Font) {
        self.font = font;
        self.relayout();
    }

    pub fn palette(&self) -> &Palette {
        &self.palette
    }

    pub fn set_palette(&mut self, palette: Palette) {
        self.palette = palette;
        self.update();
    }

    /// Title with mnemonic markers removed (`&x` -> `x`, `&&` -> `&`).
    pub fn display_title(&self) -> String {
        let mut out = String::with_capacity(self.title.len());
        let mut chars = self.title.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '&' {
                if let Some(next) = chars.next() {
                    out.push(next);
                }
            } else {
                out.push(ch);
            }
        }
        out
    }

    fn metrics(&self) -> FontMetrics {
        FontMetrics::from_font(&self.font)
    }

    fn text_height(&self) -> i32 {
        self.metrics().height.ceil() as i32
    }

    fn frame_width(&self) -> i32 {
        if self.flat {
            0
        } else {
            DEFAULT_STYLED_FRAME_WIDTH
        }
    }

    /// Height reserved for the title row and the frame's top offset (title is vertically centered on the frame line).
    fn top_metrics(&self) -> (i32, i32) {
        if self.title.is_empty() && !self.checkable {
            return (0, 0);
        }
        let check_height = if self.checkable { INDICATOR_SIZE } else { 0 };
        let top_height = self.text_height().max(check_height);
        (top_height, top_height / 2)
    }

    /// Rectangle the frame is drawn in (`SC_GroupBoxFrame`).
    pub fn frame_rect(&self) -> Rect {
        let (_, top_margin) = self.top_metrics();
        Rect::new(
            0,
            top_margin,
            self.base.geometry.width,
            (self.base.geometry.height - top_margin).max(0),
        )
    }

    /// Area available to children (`SC_GroupBoxContents`).
    pub fn contents_rect(&self) -> Rect {
        let (top_height, top_margin) = self.top_metrics();
        let frame = self.frame_rect();
        let fw = self.frame_width();
        let top = fw + top_height - top_margin;
        Rect::new(
            frame.x + fw,
            frame.y + top,
            (frame.width - 2 * fw).max(0),
            (frame.height - top - fw).max(0),
        )
    }

    fn title_text_width(&self) -> i32 {
        let text = format!("{} ", self.display_title());
        self.metrics().horizontal_advance(&text, &self.font).ceil() as i32
    }

    /// Combined indicator + label rectangle aligned inside the title row.
    fn title_total_rect(&self) -> (Rect, i32) {
        let th = self.text_height();
        let margin = if self.flat { 0 } else { TITLE_MARGIN };
        let check_width = if self.checkable {
            INDICATOR_SIZE + INDICATOR_SPACING - 1
        } else {
            0
        };
        let check_height = if self.checkable { INDICATOR_SIZE } else { 0 };
        let h = th.max(check_height);
        let available = Rect::new(margin, 0, (self.base.geometry.width - 2 * margin).max(0), h);
        let total_w = (self.title_text_width() + check_width).min(available.width);
        let x = match self.alignment {
            Alignment::Left => available.x,
            Alignment::Center => available.x + (available.width - total_w) / 2,
            Alignment::Right => available.x + available.width - total_w,
        };
        (Rect::new(x, 0, total_w, h), check_width)
    }

    /// Check indicator rectangle (`SC_GroupBoxCheckBox`); empty when not checkable.
    pub fn check_box_rect(&self) -> Rect {
        if !self.checkable {
            return Rect::new(0, 0, 0, 0);
        }
        let (total, _) = self.title_total_rect();
        let top = total.y + (total.height - INDICATOR_SIZE) / 2;
        Rect::new(total.x, top, INDICATOR_SIZE, INDICATOR_SIZE)
    }

    /// Title label rectangle (`SC_GroupBoxLabel`).
    pub fn label_rect(&self) -> Rect {
        let (total, check_width) = self.title_total_rect();
        if !self.checkable {
            return total;
        }
        let th = self.text_height();
        let top = total.y + (total.height - th) / 2;
        Rect::new(
            total.x + check_width - 2,
            top,
            (total.width - check_width).max(0),
            th,
        )
    }

    /// Sub-control under `pos` (`QStyle::hitTestComplexControl(CC_GroupBox)`).
    pub fn hit_test(&self, pos: Point) -> GroupBoxSubControl {
        if self.checkable && self.check_box_rect().contains(pos) {
            GroupBoxSubControl::CheckBox
        } else if !self.title.is_empty() && self.label_rect().contains(pos) {
            GroupBoxSubControl::Label
        } else if self.contents_rect().contains(pos) {
            GroupBoxSubControl::Contents
        } else if self.frame_rect().contains(pos) {
            GroupBoxSubControl::Frame
        } else {
            GroupBoxSubControl::None
        }
    }

    fn is_title_control(&self, pos: Point) -> bool {
        matches!(
            self.hit_test(pos),
            GroupBoxSubControl::CheckBox | GroupBoxSubControl::Label
        )
    }

    /// Enables/disables children (`QGroupBoxPrivate::_q_setChildrenEnabled`).
    fn set_children_enabled(&mut self, enabled: bool) {
        if enabled {
            let restore = std::mem::take(&mut self.auto_disabled);
            for child in &self.base.children {
                let mut child = child.borrow_mut();
                if restore.contains(&child.id()) {
                    child.set_enabled(true);
                }
            }
        } else {
            for child in &self.base.children {
                let mut child = child.borrow_mut();
                if child.is_enabled() {
                    child.set_enabled(false);
                    let id = child.id();
                    if !self.auto_disabled.contains(&id) {
                        self.auto_disabled.push(id);
                    }
                }
            }
        }
    }

    fn relayout(&mut self) {
        let contents = self.contents_rect();
        if let Some(layout) = self.base.layout.as_mut() {
            layout.set_geometry(contents);
        }
        self.update();
    }
}

impl QObject for GroupBox {
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
            EventKind::MouseButtonPress { x, y, button } => {
                let pos = Point::new(*x, *y);
                let handled = *button == 1 && self.checkable && self.is_title_control(pos);
                self.mouse_press_event(pos, *button, 0);
                handled
            }
            EventKind::MouseButtonRelease { x, y, button } => {
                let handled = *button == 1 && self.pressed_title;
                self.mouse_release_event(Point::new(*x, *y), *button, 0);
                handled
            }
            EventKind::KeyPress {
                key,
                modifiers,
                is_repeat,
            } => {
                let handled = self.checkable && *key == KEY_SPACE;
                self.key_press_event(*key, *modifiers, *is_repeat);
                handled
            }
            EventKind::KeyRelease { key, modifiers } => {
                let handled = self.space_pressed && *key == KEY_SPACE;
                self.key_release_event(*key, *modifiers);
                handled
            }
            EventKind::FocusIn { reason } => {
                self.focus_in_event(*reason);
                true
            }
            EventKind::FocusOut { reason } => {
                self.focus_out_event(*reason);
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
            _ => false,
        }
    }
}

impl Widget for GroupBox {
    fn id(&self) -> ObjectId {
        self.base.object_data.id
    }

    fn geometry(&self) -> Rect {
        self.base.geometry
    }

    fn set_geometry(&mut self, rect: Rect) {
        if self.base.geometry != rect {
            let old = Size::new(self.base.geometry.width, self.base.geometry.height);
            self.base.geometry = rect;
            self.resize_event(Size::new(rect.width, rect.height), old);
            self.relayout();
        }
    }

    fn size_hint(&self) -> Size {
        let min = self.minimum_size_hint();
        match self.base.layout.as_ref() {
            Some(layout) => {
                let hint = layout.size_hint();
                let (top_height, _) = self.top_metrics();
                let fw = self.frame_width();
                Size::new(
                    (hint.width + 2 * fw).max(min.width),
                    (hint.height + top_height + fw).max(min.height),
                )
            }
            None => min,
        }
    }

    /// `QGroupBox::minimumSizeHint`: room for the title (and indicator) plus frame.
    fn minimum_size_hint(&self) -> Size {
        let mut width = self.title_text_width();
        let mut height = self.text_height();
        if self.checkable {
            width += INDICATOR_SIZE + INDICATOR_SPACING;
            height = height.max(INDICATOR_SIZE);
        }
        let margin = if self.flat { 0 } else { TITLE_MARGIN };
        let fw = self.frame_width();
        Size::new(width + 2 * margin, height + 2 * fw)
    }

    fn size_policy(&self) -> QSizePolicy {
        self.base.size_policy
    }

    fn set_size_policy(&mut self, policy: QSizePolicy) {
        self.base.size_policy = policy;
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

    /// Qt `changeEvent(EnabledChange)`: re-enabling an unchecked box keeps its children disabled.
    fn set_enabled(&mut self, enabled: bool) {
        if self.base.enabled == enabled {
            return;
        }
        self.base.enabled = enabled;
        if enabled && self.checkable && !self.checked {
            self.set_children_enabled(false);
        }
        self.update();
    }

    fn update(&mut self) {
        self.base.dirty = Some(Rect::new(
            0,
            0,
            self.base.geometry.width,
            self.base.geometry.height,
        ));
        let target = self.base.window_id.unwrap_or(self.base.object_data.id);
        let _ = qtrs_core::event_loop::post_event_to_thread(
            qtrs_core::object::ThreadId::current(),
            target,
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
        layout.set_geometry(self.contents_rect());
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

    /// Qt `childEvent(ChildAdded)`: children added to an unchecked box start disabled.
    fn add_child(&mut self, child: WidgetRef) {
        if self.checkable && !self.checked {
            let mut w = child.borrow_mut();
            if w.is_enabled() {
                w.set_enabled(false);
                let id = w.id();
                if !self.auto_disabled.contains(&id) {
                    self.auto_disabled.push(id);
                }
            }
        }
        self.base.add_child(child);
        self.update();
    }

    fn remove_child(&mut self, child_id: ObjectId) {
        self.auto_disabled.retain(|&id| id != child_id);
        self.base.remove_child(child_id);
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
        self.update();
    }

    fn focus_in_event(&mut self, _reason: FocusReason) {
        self.update();
    }

    fn focus_out_event(&mut self, _reason: FocusReason) {
        self.space_pressed = false;
        self.update();
    }

    fn mouse_press_event(&mut self, pos: Point, button: u32, _modifiers: u32) {
        if button != 1 {
            return;
        }
        self.pressed_title = self.checkable && self.is_title_control(pos);
        if self.pressed_title {
            self.update();
        }
    }

    fn mouse_release_event(&mut self, pos: Point, button: u32, _modifiers: u32) {
        if button != 1 || !self.pressed_title {
            return;
        }
        self.pressed_title = false;
        if self.checkable && self.is_title_control(pos) {
            self.click();
        } else {
            self.update();
        }
    }

    fn key_press_event(&mut self, key: u32, _modifiers: u32, is_repeat: bool) {
        if key == KEY_SPACE && self.checkable && self.base.enabled && !is_repeat {
            self.space_pressed = true;
            self.update();
        }
    }

    fn key_release_event(&mut self, key: u32, _modifiers: u32) {
        if key == KEY_SPACE && self.space_pressed {
            self.space_pressed = false;
            self.click();
        }
    }

    fn paint_event(&mut self, painter: &mut Painter) {
        let group = if self.base.enabled {
            ColorGroup::Active
        } else {
            ColorGroup::Disabled
        };
        let window = self.palette.color(group, ColorRole::Window);
        let text_color = self.palette.color(group, ColorRole::WindowText);
        let frame = self.frame_rect();

        // Frame: etched box, or a single top line when flat.
        if self.flat {
            let line = self.palette.color(group, ColorRole::Mid);
            painter.fill_rect(
                RectF::new(frame.x as f32, frame.y as f32, frame.width as f32, 1.0),
                line,
            );
        } else {
            let mut etched = FrameStyle::new(FrameShape::Box, FrameShadow::Sunken);
            etched.line_width = 1;
            etched.paint(painter, frame, &self.palette, self.base.enabled);
        }

        if self.title.is_empty() && !self.checkable {
            return;
        }

        // Clear the frame line behind the title row.
        let (total, _) = self.title_total_rect();
        painter.fill_rect(
            RectF::new(
                (total.x - 2) as f32,
                total.y as f32,
                (total.width + 4) as f32,
                total.height as f32,
            ),
            window,
        );

        if self.checkable {
            let r = self.check_box_rect();
            let box_rect = RectF::new(
                r.x as f32 + 0.5,
                r.y as f32 + 0.5,
                r.width as f32 - 1.0,
                r.height as f32 - 1.0,
            );
            let pressed = self.pressed_title || self.space_pressed;
            let fill = if pressed {
                self.palette.color(group, ColorRole::Midlight)
            } else {
                self.palette.color(group, ColorRole::Base)
            };
            painter.set_brush(Brush::Color(fill));
            painter.set_pen(Pen::new(self.palette.color(group, ColorRole::Dark), 1.0));
            painter.draw_rounded_rect(box_rect, 2.0, 2.0);
            if self.checked {
                let accent = self.palette.color(group, ColorRole::Highlight);
                painter.set_pen(Pen::new(accent, 2.0));
                let (x, y) = (r.x as f32, r.y as f32);
                let p1 = PointF::new(x + 3.0, y + 6.5);
                let p2 = PointF::new(x + 5.5, y + 9.5);
                let p3 = PointF::new(x + 10.0, y + 3.5);
                painter.draw_line(p1, p2);
                painter.draw_line(p2, p3);
            }
        }

        let title = self.display_title();
        if !title.is_empty() {
            let label = self.label_rect();
            let metrics = self.metrics();
            let baseline =
                label.y as f32 + (label.height as f32 - metrics.height) / 2.0 + metrics.ascent;
            painter.set_pen(Pen::new(text_color, 1.0));
            painter.draw_text(PointF::new(label.x as f32, baseline), &title, &self.font);

            if self.base.has_focus && self.checkable {
                let focus = self.palette.color(group, ColorRole::Highlight);
                painter.set_brush(Brush::Color(Color::TRANSPARENT));
                painter.set_pen(Pen::new(focus, 1.0).with_dash_pattern(vec![1.0, 1.0]));
                painter.draw_rect(RectF::new(
                    label.x as f32 - 1.5,
                    label.y as f32 - 0.5,
                    label.width as f32 + 2.0,
                    label.height as f32 + 1.0,
                ));
            }
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
