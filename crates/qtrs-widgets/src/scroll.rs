use qtrs_core::event::{Event, EventKind};
use qtrs_core::object::{ObjectData, ObjectId, QObject};
use qtrs_core::signal::Signal;
use qtrs_gui::geometry::primitives::{Point, Rect, RectF, Size};
use qtrs_gui::paint::brush::Brush;
use qtrs_gui::paint::painter::Painter;
use qtrs_gui::tiny_skia::Color;
use crate::focus::FocusPolicy;
use crate::layout::Layout;
use crate::size_policy::{Policy, QSizePolicy};
use crate::widget::{Widget, WidgetBase, WidgetRef, WidgetWeak};

/// Orientation of a scroll bar or slider (`Qt::Orientation`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    Horizontal,
    Vertical,
}

/// Scrollbar policy for a scroll area (`Qt::ScrollBarPolicy`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScrollBarPolicy {
    #[default]
    AsNeeded,
    AlwaysOn,
    AlwaysOff,
}

/// Standalone scrollbar control (`QScrollBar`).
pub struct ScrollBar {
    base: WidgetBase,
    orientation: Orientation,
    minimum: i32,
    maximum: i32,
    value: i32,
    page_step: i32,
    single_step: i32,

    dragging: bool,
    drag_start_coord: i32,
    drag_start_value: i32,
    hovered: bool,

    // Colors
    track_color: Color,
    thumb_color: Color,
    thumb_hover_color: Color,

    pub value_changed: Signal<i32>,
}

impl ScrollBar {
    pub fn new(orientation: Orientation) -> Self {
        let mut base = WidgetBase::new();
        base.focus_policy = FocusPolicy::NoFocus;

        let (w, h) = match orientation {
            Orientation::Horizontal => (100, 10),
            Orientation::Vertical => (10, 100),
        };
        base.geometry = Rect::new(0, 0, w, h);

        Self {
            base,
            orientation,
            minimum: 0,
            maximum: 100,
            value: 0,
            page_step: 10,
            single_step: 1,

            dragging: false,
            drag_start_coord: 0,
            drag_start_value: 0,
            hovered: false,

            track_color: Color::from_rgba8(240, 240, 240, 120),
            thumb_color: Color::from_rgba8(170, 170, 170, 180),
            thumb_hover_color: Color::from_rgba8(130, 130, 130, 220),

            value_changed: Signal::new(),
        }
    }

    pub fn orientation(&self) -> Orientation {
        self.orientation
    }

    pub fn minimum(&self) -> i32 {
        self.minimum
    }

    pub fn maximum(&self) -> i32 {
        self.maximum
    }

    pub fn value(&self) -> i32 {
        self.value
    }

    pub fn page_step(&self) -> i32 {
        self.page_step
    }

    pub fn single_step(&self) -> i32 {
        self.single_step
    }

    pub fn set_range(&mut self, min: i32, max: i32) {
        let new_max = max.max(min);
        self.minimum = min;
        self.maximum = new_max;
        self.set_value(self.value);
    }

    pub fn set_page_step(&mut self, step: i32) {
        self.page_step = step.max(1);
        self.update();
    }

    pub fn set_single_step(&mut self, step: i32) {
        self.single_step = step.max(1);
    }

    pub fn set_value(&mut self, val: i32) {
        let clamped = val.clamp(self.minimum, self.maximum);
        if self.value != clamped {
            self.value = clamped;
            self.value_changed.emit(&self.value);
            self.update();
        }
    }

    fn track_length(&self) -> i32 {
        match self.orientation {
            Orientation::Horizontal => self.base.geometry.width,
            Orientation::Vertical => self.base.geometry.height,
        }
    }

    fn thumb_geometry(&self) -> (i32, i32) {
        let track_len = self.track_length();
        let range = (self.maximum - self.minimum).max(0);
        if range == 0 {
            return (0, track_len);
        }

        let total_steps = range + self.page_step;
        let thumb_len = ((track_len as f32 * self.page_step as f32) / total_steps as f32)
            .round() as i32;
        let thumb_len = thumb_len.clamp(16, track_len.max(16));

        let available_track = (track_len - thumb_len).max(0);
        let thumb_offset = ((available_track as f32 * (self.value - self.minimum) as f32)
            / range as f32)
            .round() as i32;

        (thumb_offset, thumb_len)
    }
}

impl QObject for ScrollBar {
    fn object_data(&self) -> &ObjectData {
        &self.base.object_data
    }

    fn object_data_mut(&mut self) -> &mut ObjectData {
        &mut self.base.object_data
    }

    fn event(&mut self, event: &mut Event) -> bool {
        match &event.kind {
            EventKind::Enter { .. } => {
                self.hovered = true;
                self.update();
                true
            }
            EventKind::Leave => {
                self.hovered = false;
                self.update();
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
            EventKind::MouseMove { x, y } => {
                self.mouse_move_event(Point::new(*x, *y));
                true
            }
            EventKind::Wheel { angle_delta_y, .. } => {
                self.wheel_event(Point::new(0, 0), *angle_delta_y, 0);
                true
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

impl Widget for ScrollBar {
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
        match self.orientation {
            Orientation::Horizontal => Size::new(100, 10),
            Orientation::Vertical => Size::new(10, 100),
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

    fn mouse_press_event(&mut self, pos: Point, button: u32, _modifiers: u32) {
        if button != 1 {
            return;
        }

        let coord = match self.orientation {
            Orientation::Horizontal => pos.x,
            Orientation::Vertical => pos.y,
        };

        let (thumb_off, thumb_len) = self.thumb_geometry();

        if coord >= thumb_off && coord <= thumb_off + thumb_len {
            // Click inside thumb -> start drag
            self.dragging = true;
            self.drag_start_coord = coord;
            self.drag_start_value = self.value;
        } else if coord < thumb_off {
            // Page up
            self.set_value(self.value - self.page_step);
        } else {
            // Page down
            self.set_value(self.value + self.page_step);
        }
    }

    fn mouse_release_event(&mut self, _pos: Point, button: u32, _modifiers: u32) {
        if button == 1 {
            self.dragging = false;
        }
    }

    fn mouse_move_event(&mut self, pos: Point) {
        if !self.dragging {
            return;
        }

        let coord = match self.orientation {
            Orientation::Horizontal => pos.x,
            Orientation::Vertical => pos.y,
        };

        let delta_pixels = coord - self.drag_start_coord;
        let track_len = self.track_length();
        let (_, thumb_len) = self.thumb_geometry();
        let available_track = (track_len - thumb_len).max(1);
        let range = self.maximum - self.minimum;

        let delta_val = ((delta_pixels as f32 * range as f32) / available_track as f32).round() as i32;
        self.set_value(self.drag_start_value + delta_val);
    }

    fn wheel_event(&mut self, _pos: Point, delta_y: i32, _modifiers: u32) {
        let steps = if delta_y > 0 {
            -self.single_step * 3
        } else {
            self.single_step * 3
        };
        self.set_value(self.value + steps);
    }

    fn paint_event(&mut self, painter: &mut Painter) {
        let geom = self.base.geometry;
        let rect_f = RectF::new(0.0, 0.0, geom.width as f32, geom.height as f32);

        // 1. Draw track
        painter.set_brush(Brush::Color(self.track_color));
        painter.set_pen(None);
        painter.draw_rounded_rect(rect_f, 3.0, 3.0);

        // 2. Draw thumb
        let (thumb_off, thumb_len) = self.thumb_geometry();
        let thumb_rect = match self.orientation {
            Orientation::Horizontal => RectF::new(
                thumb_off as f32,
                1.0,
                thumb_len as f32,
                (geom.height - 2).max(1) as f32,
            ),
            Orientation::Vertical => RectF::new(
                1.0,
                thumb_off as f32,
                (geom.width - 2).max(1) as f32,
                thumb_len as f32,
            ),
        };

        let t_color = if self.dragging || self.hovered {
            self.thumb_hover_color
        } else {
            self.thumb_color
        };
        painter.set_brush(Brush::Color(t_color));
        painter.draw_rounded_rect(thumb_rect, 4.0, 4.0);
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// Scrollable container widget supporting dynamic viewport clipping and auto-scrollbars (`QScrollArea`).
pub struct ScrollArea {
    base: WidgetBase,
    widget: Option<WidgetRef>,
    h_scrollbar: ScrollBar,
    v_scrollbar: ScrollBar,
    h_policy: ScrollBarPolicy,
    v_policy: ScrollBarPolicy,
    scroll_x: i32,
    scroll_y: i32,
}

impl ScrollArea {
    /// Creates a new scroll area.
    pub fn new() -> Self {
        let mut base = WidgetBase::new();
        base.size_policy = QSizePolicy::new(Policy::Expanding, Policy::Expanding);

        let h_bar = ScrollBar::new(Orientation::Horizontal);
        let v_bar = ScrollBar::new(Orientation::Vertical);

        Self {
            base,
            widget: None,
            h_scrollbar: h_bar,
            v_scrollbar: v_bar,
            h_policy: ScrollBarPolicy::AsNeeded,
            v_policy: ScrollBarPolicy::AsNeeded,
            scroll_x: 0,
            scroll_y: 0,
        }
    }

    /// Sets the child content widget inside the scroll area.
    pub fn set_widget(&mut self, widget: WidgetRef) {
        self.base.add_child(widget.clone());
        self.widget = Some(widget);
        self.update_scrollbars();
    }

    pub fn widget(&self) -> Option<WidgetRef> {
        self.widget.clone()
    }

    pub fn horizontal_scroll_bar(&self) -> &ScrollBar {
        &self.h_scrollbar
    }

    pub fn horizontal_scroll_bar_mut(&mut self) -> &mut ScrollBar {
        &mut self.h_scrollbar
    }

    pub fn vertical_scroll_bar(&self) -> &ScrollBar {
        &self.v_scrollbar
    }

    pub fn vertical_scroll_bar_mut(&mut self) -> &mut ScrollBar {
        &mut self.v_scrollbar
    }

    pub fn set_horizontal_scrollbar_policy(&mut self, policy: ScrollBarPolicy) {
        self.h_policy = policy;
        self.update_scrollbars();
    }

    pub fn set_vertical_scrollbar_policy(&mut self, policy: ScrollBarPolicy) {
        self.v_policy = policy;
        self.update_scrollbars();
    }

    pub fn scroll_position(&self) -> Point {
        Point::new(self.scroll_x, self.scroll_y)
    }

    pub fn set_scroll_position(&mut self, x: i32, y: i32) {
        self.h_scrollbar.set_value(x);
        self.v_scrollbar.set_value(y);
        self.scroll_x = self.h_scrollbar.value();
        self.scroll_y = self.v_scrollbar.value();
        if let Some(w) = &self.widget {
            let mut b = w.borrow_mut();
            let g = b.geometry();
            b.set_geometry(Rect::new(-self.scroll_x, -self.scroll_y, g.width, g.height));
        }
        self.update();
    }

    /// Calculates the inner viewport bounds excluding any active scrollbars.
    pub fn viewport_rect(&self) -> Rect {
        let geom = self.base.geometry;
        let bar_size = 10;

        let v_visible = self.v_scrollbar.is_visible();
        let h_visible = self.h_scrollbar.is_visible();

        let w = if v_visible {
            (geom.width - bar_size).max(0)
        } else {
            geom.width
        };
        let h = if h_visible {
            (geom.height - bar_size).max(0)
        } else {
            geom.height
        };

        Rect::new(0, 0, w, h)
    }

    fn update_scrollbars(&mut self) {
        let geom = self.base.geometry;
        if geom.width <= 0 || geom.height <= 0 {
            return;
        }

        let bar_size = 10;
        let content_size = if let Some(w) = &self.widget {
            let b = w.borrow();
            let g = b.geometry();
            let h = b.size_hint();
            Size::new(g.width.max(h.width), g.height.max(h.height))
        } else {
            Size::new(0, 0)
        };

        // Determine scrollbar visibility
        let show_v = match self.v_policy {
            ScrollBarPolicy::AlwaysOn => true,
            ScrollBarPolicy::AlwaysOff => false,
            ScrollBarPolicy::AsNeeded => content_size.height > geom.height,
        };

        let view_w = if show_v { geom.width - bar_size } else { geom.width };

        let show_h = match self.h_policy {
            ScrollBarPolicy::AlwaysOn => true,
            ScrollBarPolicy::AlwaysOff => false,
            ScrollBarPolicy::AsNeeded => content_size.width > view_w,
        };

        let view_h = if show_h { geom.height - bar_size } else { geom.height };

        // Position and update vertical scrollbar
        self.v_scrollbar.set_visible(show_v);
        if show_v {
            self.v_scrollbar.set_geometry(Rect::new(
                geom.width - bar_size,
                0,
                bar_size,
                view_h,
            ));
            let max_v = (content_size.height - view_h).max(0);
            self.v_scrollbar.set_range(0, max_v);
            self.v_scrollbar.set_page_step(view_h);
        }

        // Position and update horizontal scrollbar
        self.h_scrollbar.set_visible(show_h);
        if show_h {
            self.h_scrollbar.set_geometry(Rect::new(
                0,
                geom.height - bar_size,
                view_w,
                bar_size,
            ));
            let max_h = (content_size.width - view_w).max(0);
            self.h_scrollbar.set_range(0, max_h);
            self.h_scrollbar.set_page_step(view_w);
        }

        self.scroll_x = self.h_scrollbar.value();
        self.scroll_y = self.v_scrollbar.value();

        // Position content widget
        if let Some(w) = &self.widget {
            let child_w = content_size.width.max(view_w);
            let child_h = content_size.height.max(view_h);
            w.borrow_mut().set_geometry(Rect::new(
                -self.scroll_x,
                -self.scroll_y,
                child_w,
                child_h,
            ));
        }

        self.update();
    }
}

impl Default for ScrollArea {
    fn default() -> Self {
        Self::new()
    }
}

impl QObject for ScrollArea {
    fn object_data(&self) -> &ObjectData {
        &self.base.object_data
    }

    fn object_data_mut(&mut self) -> &mut ObjectData {
        &mut self.base.object_data
    }

    fn event(&mut self, event: &mut Event) -> bool {
        match &event.kind {
            EventKind::Wheel { angle_delta_y, pixel_delta_x, modifiers, .. } => {
                let is_shift = (*modifiers & 0x02000000 != 0) || (*modifiers & 1 != 0);
                if is_shift || *pixel_delta_x != 0 {
                    self.h_scrollbar.wheel_event(Point::new(0, 0), *angle_delta_y, 0);
                    self.scroll_x = self.h_scrollbar.value();
                } else {
                    self.v_scrollbar.wheel_event(Point::new(0, 0), *angle_delta_y, 0);
                    self.scroll_y = self.v_scrollbar.value();
                }
                if let Some(w) = &self.widget {
                    let mut b = w.borrow_mut();
                    let g = b.geometry();
                    b.set_geometry(Rect::new(-self.scroll_x, -self.scroll_y, g.width, g.height));
                }
                self.update();
                true
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

impl Widget for ScrollArea {
    fn id(&self) -> ObjectId {
        self.base.object_data.id
    }

    fn geometry(&self) -> Rect {
        self.base.geometry
    }

    fn set_geometry(&mut self, rect: Rect) {
        if self.base.geometry != rect {
            self.base.geometry = rect;
            self.update_scrollbars();
        }
    }

    fn size_hint(&self) -> Size {
        Size::new(200, 200)
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
        if let Some(w) = &self.widget {
            w.borrow_mut().set_window_id(window_id);
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

    fn wheel_event(&mut self, pos: Point, delta_y: i32, modifiers: u32) {
        let is_shift = (modifiers & 0x02000000 != 0) || (modifiers & 1 != 0);
        if is_shift {
            self.h_scrollbar.wheel_event(pos, delta_y, modifiers);
            self.scroll_x = self.h_scrollbar.value();
        } else {
            self.v_scrollbar.wheel_event(pos, delta_y, modifiers);
            self.scroll_y = self.v_scrollbar.value();
        }
        if let Some(w) = &self.widget {
            let mut b = w.borrow_mut();
            let g = b.geometry();
            b.set_geometry(Rect::new(-self.scroll_x, -self.scroll_y, g.width, g.height));
        }
        self.update();
    }

    fn paint_event(&mut self, painter: &mut Painter) {
        let viewport = self.viewport_rect();

        // 1. Clip and render content widget
        if let Some(w) = &self.widget {
            painter.save();
            painter.set_clip_rect(RectF::new(
                viewport.x as f32,
                viewport.y as f32,
                viewport.width as f32,
                viewport.height as f32,
            ));
            painter.translate(-self.scroll_x as f32, -self.scroll_y as f32);

            let mut w_borrow = w.borrow_mut();
            w_borrow.paint_event(painter);
            drop(w_borrow);

            painter.restore();
        }

        // 2. Render scrollbars
        if self.v_scrollbar.is_visible() {
            painter.save();
            let g = self.v_scrollbar.geometry();
            painter.translate(g.x as f32, g.y as f32);
            self.v_scrollbar.paint_event(painter);
            painter.restore();
        }

        if self.h_scrollbar.is_visible() {
            painter.save();
            let g = self.h_scrollbar.geometry();
            painter.translate(g.x as f32, g.y as f32);
            self.h_scrollbar.paint_event(painter);
            painter.restore();
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
