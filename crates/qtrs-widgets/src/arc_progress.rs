//! HUD circular dial gauge and arc progress widgets.
//!
//! Provides `ArcProgressWidget` and `DialWidget` for system resource monitors,
//! AI token consumption gauges, status meters, and interactive control dials.

use qtrs_core::event::{Event, EventKind};
use qtrs_core::event_loop::post_event_to_thread;
use qtrs_core::object::{ObjectId, ObjectData, QObject, ThreadId};
use qtrs_core::signal::Signal;
use qtrs_gui::geometry::primitives::{Point, PointF, Rect, RectF, Size};
use qtrs_gui::paint::{Brush, Painter, Pen};
use qtrs_gui::text::font::{Font, FontWeight};
use qtrs_gui::text::font_metrics::FontMetrics;
use qtrs_gui::tiny_skia::{Color, LineCap};
use crate::layout::Layout;
use crate::widget::{Widget, WidgetBase, WidgetRef, WidgetWeak};

/// Color configuration for multi-tier status thresholds (e.g. Normal, Warning, Critical).
#[derive(Debug, Clone, PartialEq)]
pub struct ThresholdColors {
    pub normal: Color,
    pub warning: Color,
    pub critical: Color,
    /// Percentage threshold for warning state (e.g. 70.0 for 70%).
    pub warning_threshold: f32,
    /// Percentage threshold for critical state (e.g. 90.0 for 90%).
    pub critical_threshold: f32,
}

impl Default for ThresholdColors {
    fn default() -> Self {
        Self {
            normal: Color::from_rgba8(0, 210, 255, 255),    // HUD Cyan
            warning: Color::from_rgba8(255, 180, 0, 255),   // Warning Orange/Amber
            critical: Color::from_rgba8(255, 65, 54, 255),  // Critical Red
            warning_threshold: 70.0,
            critical_threshold: 90.0,
        }
    }
}

/// HUD circular dial gauge and arc progress widget.
///
/// Supports customizable start/span angles, track/progress widths, rounded stroke caps,
/// multi-tier threshold warning colors, anti-aliased center value/label readouts,
/// and optional mouse/wheel interaction.
pub struct ArcProgressWidget {
    pub base: WidgetBase,
    value: f32,
    min_value: f32,
    max_value: f32,
    start_angle: f32,
    span_angle: f32,
    track_color: Color,
    progress_color: Color,
    track_width: f32,
    progress_width: f32,
    rounded_caps: bool,
    background_color: Option<Color>,

    threshold_colors: Option<ThresholdColors>,

    show_value_text: bool,
    custom_text: Option<String>,
    prefix: String,
    suffix: String,
    precision: usize,
    text_color: Color,
    font: Font,

    title: Option<String>,
    title_color: Color,
    title_font: Font,

    interactive: bool,
    is_dragging: bool,
    step: f32,

    pub value_changed: Signal<f32>,
}

/// Convenient type alias for dial gauge usage.
pub type DialWidget = ArcProgressWidget;

impl ArcProgressWidget {
    /// Creates a standard 270-degree HUD dial gauge (open at bottom).
    pub fn new() -> Self {
        let mut base = WidgetBase::new();
        base.geometry = Rect::new(0, 0, 120, 120);

        Self {
            base,
            value: 0.0,
            min_value: 0.0,
            max_value: 100.0,
            start_angle: 225.0,
            span_angle: -270.0,
            track_color: Color::from_rgba8(45, 52, 64, 180),
            progress_color: Color::from_rgba8(0, 210, 255, 255),
            track_width: 8.0,
            progress_width: 8.0,
            rounded_caps: true,
            background_color: None,

            threshold_colors: None,

            show_value_text: true,
            custom_text: None,
            prefix: String::new(),
            suffix: "%".to_string(),
            precision: 0,
            text_color: Color::from_rgba8(235, 240, 250, 255),
            font: Font::new("Segoe UI", 16.0).with_weight(FontWeight::Bold),

            title: None,
            title_color: Color::from_rgba8(160, 175, 195, 200),
            title_font: Font::new("Segoe UI", 10.0),

            interactive: false,
            is_dragging: false,
            step: 1.0,

            value_changed: Signal::new(),
        }
    }

    /// Creates a 360-degree full circular progress ring (starts at top, clockwise).
    pub fn full_ring() -> Self {
        let mut widget = Self::new();
        widget.start_angle = 90.0;
        widget.span_angle = -360.0;
        widget
    }

    /// Creates a 180-degree semi-circle meter gauge (starts at 9 o'clock, sweeps across top to 3 o'clock).
    pub fn semicircle() -> Self {
        let mut widget = Self::new();
        widget.start_angle = 180.0;
        widget.span_angle = -180.0;
        widget
    }

    // -------------------------------------------------------------------------
    // Value and Range
    // -------------------------------------------------------------------------

    pub fn value(&self) -> f32 {
        self.value
    }

    pub fn set_value(&mut self, val: f32) {
        let clamped = val.clamp(self.min_value, self.max_value);
        if (self.value - clamped).abs() > 1e-4 {
            self.value = clamped;
            self.update();
            self.value_changed.emit(&clamped);
        }
    }

    pub fn min_value(&self) -> f32 {
        self.min_value
    }

    pub fn max_value(&self) -> f32 {
        self.max_value
    }

    pub fn set_range(&mut self, min: f32, max: f32) {
        if max > min {
            self.min_value = min;
            self.max_value = max;
            let clamped = self.value.clamp(min, max);
            if (self.value - clamped).abs() > 1e-4 {
                self.value = clamped;
                self.value_changed.emit(&clamped);
            }
            self.update();
        }
    }

    /// Returns the normalized progress ratio in the range `[0.0, 1.0]`.
    pub fn progress_ratio(&self) -> f32 {
        let span = self.max_value - self.min_value;
        if span.abs() < 1e-5 {
            0.0
        } else {
            ((self.value - self.min_value) / span).clamp(0.0, 1.0)
        }
    }

    /// Returns the progress percentage in the range `[0.0, 100.0]`.
    pub fn progress_percentage(&self) -> f32 {
        self.progress_ratio() * 100.0
    }

    // -------------------------------------------------------------------------
    // Angles and Geometry
    // -------------------------------------------------------------------------

    pub fn start_angle(&self) -> f32 {
        self.start_angle
    }

    pub fn span_angle(&self) -> f32 {
        self.span_angle
    }

    pub fn set_angles(&mut self, start_deg: f32, span_deg: f32) {
        self.start_angle = start_deg;
        self.span_angle = span_deg;
        self.update();
    }

    pub fn track_width(&self) -> f32 {
        self.track_width
    }

    pub fn set_track_width(&mut self, width: f32) {
        self.track_width = width.max(0.0);
        self.update();
    }

    pub fn progress_width(&self) -> f32 {
        self.progress_width
    }

    pub fn set_progress_width(&mut self, width: f32) {
        self.progress_width = width.max(0.0);
        self.update();
    }

    pub fn rounded_caps(&self) -> bool {
        self.rounded_caps
    }

    pub fn set_rounded_caps(&mut self, rounded: bool) {
        self.rounded_caps = rounded;
        self.update();
    }

    // -------------------------------------------------------------------------
    // Colors and Thresholds
    // -------------------------------------------------------------------------

    pub fn track_color(&self) -> Color {
        self.track_color
    }

    pub fn set_track_color(&mut self, color: Color) {
        self.track_color = color;
        self.update();
    }

    pub fn progress_color(&self) -> Color {
        self.progress_color
    }

    pub fn set_progress_color(&mut self, color: Color) {
        self.progress_color = color;
        self.update();
    }

    pub fn set_background_color(&mut self, color: Option<Color>) {
        self.background_color = color;
        self.update();
    }

    pub fn set_threshold_colors(
        &mut self,
        normal: Color,
        warning: Color,
        critical: Color,
        warning_pct: f32,
        critical_pct: f32,
    ) {
        self.threshold_colors = Some(ThresholdColors {
            normal,
            warning,
            critical,
            warning_threshold: warning_pct,
            critical_threshold: critical_pct,
        });
        self.update();
    }

    pub fn clear_threshold_colors(&mut self) {
        self.threshold_colors = None;
        self.update();
    }

    /// Resolves active progress color based on current value and threshold settings.
    pub fn current_progress_color(&self) -> Color {
        if let Some(thresholds) = &self.threshold_colors {
            let pct = self.progress_percentage();
            if pct >= thresholds.critical_threshold {
                thresholds.critical
            } else if pct >= thresholds.warning_threshold {
                thresholds.warning
            } else {
                thresholds.normal
            }
        } else {
            self.progress_color
        }
    }

    // -------------------------------------------------------------------------
    // Text and Labels
    // -------------------------------------------------------------------------

    pub fn show_value_text(&self) -> bool {
        self.show_value_text
    }

    pub fn set_show_value_text(&mut self, show: bool) {
        self.show_value_text = show;
        self.update();
    }

    pub fn custom_text(&self) -> Option<&str> {
        self.custom_text.as_deref()
    }

    pub fn set_custom_text(&mut self, text: Option<String>) {
        self.custom_text = text;
        self.update();
    }

    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    pub fn set_prefix(&mut self, prefix: impl Into<String>) {
        self.prefix = prefix.into();
        self.update();
    }

    pub fn suffix(&self) -> &str {
        &self.suffix
    }

    pub fn set_suffix(&mut self, suffix: impl Into<String>) {
        self.suffix = suffix.into();
        self.update();
    }

    pub fn precision(&self) -> usize {
        self.precision
    }

    pub fn set_precision(&mut self, precision: usize) {
        self.precision = precision;
        self.update();
    }

    pub fn text_color(&self) -> Color {
        self.text_color
    }

    pub fn set_text_color(&mut self, color: Color) {
        self.text_color = color;
        self.update();
    }

    pub fn font(&self) -> &Font {
        &self.font
    }

    pub fn set_font(&mut self, font: Font) {
        self.font = font;
        self.update();
    }

    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = Some(title.into());
        self.update();
    }

    pub fn clear_title(&mut self) {
        self.title = None;
        self.update();
    }

    pub fn title_color(&self) -> Color {
        self.title_color
    }

    pub fn set_title_color(&mut self, color: Color) {
        self.title_color = color;
        self.update();
    }

    pub fn title_font(&self) -> &Font {
        &self.title_font
    }

    pub fn set_title_font(&mut self, font: Font) {
        self.title_font = font;
        self.update();
    }

    // -------------------------------------------------------------------------
    // Interactivity
    // -------------------------------------------------------------------------

    pub fn is_interactive(&self) -> bool {
        self.interactive
    }

    pub fn set_interactive(&mut self, interactive: bool) {
        self.interactive = interactive;
    }

    pub fn step(&self) -> f32 {
        self.step
    }

    pub fn set_step(&mut self, step: f32) {
        self.step = step.max(1e-4);
    }

    // Maps mouse position to dial value based on angles
    fn update_value_from_pos(&mut self, pos: Point) {
        let geom = self.base.geometry;
        let cx = geom.width as f32 / 2.0;
        let cy = geom.height as f32 / 2.0;

        let dx = pos.x as f32 - cx;
        let dy = cy - pos.y as f32; // Inverted Y: screen down is positive

        if dx.abs() < 1e-4 && dy.abs() < 1e-4 {
            return;
        }

        let mut angle_deg = dy.atan2(dx).to_degrees();
        if angle_deg < 0.0 {
            angle_deg += 360.0;
        }

        let total_sweep = self.span_angle.abs();
        if total_sweep < 1e-3 {
            return;
        }

        let diff = if self.span_angle < 0.0 {
            // Clockwise sweep
            let mut d = self.start_angle - angle_deg;
            while d < 0.0 {
                d += 360.0;
            }
            while d >= 360.0 {
                d -= 360.0;
            }
            d
        } else {
            // Counter-clockwise sweep
            let mut d = angle_deg - self.start_angle;
            while d < 0.0 {
                d += 360.0;
            }
            while d >= 360.0 {
                d -= 360.0;
            }
            d
        };

        let ratio = if diff <= total_sweep {
            (diff / total_sweep).clamp(0.0, 1.0)
        } else {
            // In the gap: snap to nearest endpoint
            let mid_gap = total_sweep + (360.0 - total_sweep) / 2.0;
            if diff < mid_gap {
                1.0
            } else {
                0.0
            }
        };

        let new_val = self.min_value + ratio * (self.max_value - self.min_value);
        self.set_value(new_val);
    }
}

impl Default for ArcProgressWidget {
    fn default() -> Self {
        Self::new()
    }
}

impl QObject for ArcProgressWidget {
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
            EventKind::MouseButtonPress { x, y, button } => {
                self.mouse_press_event(Point::new(*x, *y), *button, 0);
                true
            }
            EventKind::MouseButtonRelease { x, y, button } => {
                self.mouse_release_event(Point::new(*x, *y), *button, 0);
                true
            }
            EventKind::Wheel { x, y, angle_delta_y, modifiers, .. } => {
                self.wheel_event(Point::new(*x, *y), *angle_delta_y, *modifiers);
                true
            }
            _ => false,
        }
    }
}

impl Widget for ArcProgressWidget {
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
            self.update();
        }
    }

    fn size_hint(&self) -> Size {
        Size::new(120, 120)
    }

    fn minimum_size(&self) -> Size {
        Size::new(40, 40)
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
    }

    fn children(&self) -> Vec<WidgetRef> {
        self.base.children.clone()
    }

    fn add_child(&mut self, child: WidgetRef) {
        self.base.children.push(child);
    }

    fn remove_child(&mut self, child_id: ObjectId) {
        self.base.children.retain(|c| c.borrow().id() != child_id);
    }

    fn mouse_press_event(&mut self, pos: Point, button: u32, _modifiers: u32) {
        if self.base.enabled && self.interactive && button == 1 {
            self.is_dragging = true;
            self.update_value_from_pos(pos);
        }
    }

    fn mouse_move_event(&mut self, pos: Point) {
        if self.base.enabled && self.interactive && self.is_dragging {
            self.update_value_from_pos(pos);
        }
    }

    fn mouse_release_event(&mut self, _pos: Point, button: u32, _modifiers: u32) {
        if button == 1 {
            self.is_dragging = false;
        }
    }

    fn wheel_event(&mut self, _pos: Point, delta_y: i32, _modifiers: u32) {
        if self.base.enabled && self.interactive {
            let change = if delta_y > 0 { self.step } else { -self.step };
            self.set_value(self.value + change);
        }
    }

    fn paint_event(&mut self, painter: &mut Painter) {
        let geom = self.base.geometry;
        let w = geom.width as f32;
        let h = geom.height as f32;
        if w <= 0.0 || h <= 0.0 {
            return;
        }

        // 1. Draw optional widget background
        if let Some(bg) = self.background_color {
            painter.set_brush(Brush::Color(bg));
            painter.set_pen(None);
            painter.draw_rect(RectF::new(0.0, 0.0, w, h));
        }

        // 2. Geometry calculations
        let max_stroke = self.track_width.max(self.progress_width);
        let size = w.min(h);
        let radius = ((size - max_stroke) / 2.0).max(1.0);
        let cx = w / 2.0;
        let cy = h / 2.0;
        let arc_rect = RectF::new(cx - radius, cy - radius, radius * 2.0, radius * 2.0);

        let cap = if self.rounded_caps {
            LineCap::Round
        } else {
            LineCap::Butt
        };

        // 3. Draw Track Arc (Background Arc)
        if self.track_width > 0.0 && self.span_angle.abs() > 1e-3 {
            let pen = Pen::new(self.track_color, self.track_width).with_cap(cap);
            painter.set_pen(Some(pen));
            painter.set_brush(Brush::NoBrush);
            painter.draw_arc(arc_rect, self.start_angle, self.span_angle);
        }

        // 4. Draw Progress Arc
        let ratio = self.progress_ratio();
        let progress_span = self.span_angle * ratio;
        if self.progress_width > 0.0 && progress_span.abs() > 0.1 {
            let current_color = self.current_progress_color();
            let pen = Pen::new(current_color, self.progress_width).with_cap(cap);
            painter.set_pen(Some(pen));
            painter.set_brush(Brush::NoBrush);
            painter.draw_arc(arc_rect, self.start_angle, progress_span);
        }

        // 5. Draw Center Text / HUD Value readout
        if self.show_value_text {
            let text = match &self.custom_text {
                Some(t) => t.clone(),
                None => {
                    if self.precision == 0 {
                        format!("{}{:.0}{}", self.prefix, self.value, self.suffix)
                    } else {
                        format!("{}{:.*}{}", self.prefix, self.precision, self.value, self.suffix)
                    }
                }
            };

            let metrics = FontMetrics::from_font(&self.font);
            let text_w = metrics.horizontal_advance(&text, &self.font);
            let text_h = metrics.height;

            let has_title = self.title.as_ref().map(|t| !t.is_empty()).unwrap_or(false);
            let title_metrics = if has_title {
                Some(FontMetrics::from_font(&self.title_font))
            } else {
                None
            };
            let title_h = title_metrics.as_ref().map(|m| m.height).unwrap_or(0.0);

            let total_block_h = if has_title {
                text_h + title_h + 2.0
            } else {
                text_h
            };

            let block_top = (h - total_block_h) / 2.0;
            let value_baseline = block_top + metrics.ascent;
            let value_x = ((w - text_w) / 2.0).max(0.0);

            painter.set_pen(Pen::new(self.text_color, 1.0));
            painter.draw_text(PointF::new(value_x, value_baseline), &text, &self.font);

            if let (Some(title_text), Some(t_metrics)) = (&self.title, title_metrics) {
                let t_w = t_metrics.horizontal_advance(title_text, &self.title_font);
                let t_x = ((w - t_w) / 2.0).max(0.0);
                let t_baseline = block_top + text_h + 2.0 + t_metrics.ascent;
                painter.set_pen(Pen::new(self.title_color, 1.0));
                painter.draw_text(PointF::new(t_x, t_baseline), title_text, &self.title_font);
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
