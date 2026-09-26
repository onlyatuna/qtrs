//! Rounded range control (`QDial`).
//!
//! Angle/value mapping follows `qdial.cpp` (`valueFromPoint`) and
//! `QStyleHelper::calcRadialPos`: without wrapping the dial sweeps 300° from
//! 240° (lower left, minimum) clockwise to -60° (lower right, maximum); with
//! wrapping it covers the full circle starting at the bottom (270°).

use std::f64::consts::PI;

use qtrs_core::event::{Event, FocusReason};
use qtrs_core::object::{ObjectId, QObject};
use qtrs_gui::geometry::primitives::{Point, PointF, Rect, RectF, Size};
use qtrs_gui::paint::brush::Brush;
use qtrs_gui::paint::painter::{Painter, Pen};
use qtrs_gui::tiny_skia::Color;

use crate::focus::FocusPolicy;
use crate::input_common::{self, leaf_qobject_common, leaf_widget_common};
use crate::scroll::Orientation;
use crate::size_policy::{Policy, QSizePolicy};
use crate::slider::{AbstractSlider, SliderState};
use crate::widget::{Widget, WidgetBase};

/// Circular slider (`QDial`).
pub struct Dial {
    base: WidgetBase,
    state: SliderState,
    notch_target: f64,
    notches_visible: bool,
    mouse_down: bool,

    face_color: Color,
    border_color: Color,
    notch_color: Color,
    pointer_color: Color,
    focus_ring_color: Color,
}

pub type QDial = Dial;

impl Dial {
    pub fn new() -> Self {
        let mut base = WidgetBase::with_geometry(Rect::new(0, 0, 50, 50));
        base.focus_policy = FocusPolicy::WheelFocus;
        base.size_policy = QSizePolicy::new(Policy::Preferred, Policy::Preferred);
        Self {
            base,
            state: SliderState::new(Orientation::Horizontal),
            notch_target: 3.7,
            notches_visible: false,
            mouse_down: false,
            face_color: Color::from_rgba8(240, 240, 240, 255),
            border_color: Color::from_rgba8(140, 140, 140, 255),
            notch_color: Color::from_rgba8(90, 90, 90, 255),
            pointer_color: Color::from_rgba8(0, 120, 215, 255),
            focus_ring_color: Color::from_rgba8(0, 120, 215, 180),
        }
    }

    /// Whether values wrap around between maximum and minimum.
    pub fn wrapping(&self) -> bool {
        self.state.wrapping()
    }

    pub fn set_wrapping(&mut self, wrapping: bool) {
        self.state.set_wrapping(wrapping);
        self.update();
    }

    pub fn notches_visible(&self) -> bool {
        self.notches_visible
    }

    pub fn set_notches_visible(&mut self, visible: bool) {
        self.notches_visible = visible;
        self.update();
    }

    /// Target number of pixels between notches (default 3.7).
    pub fn notch_target(&self) -> f64 {
        self.notch_target
    }

    pub fn set_notch_target(&mut self, target: f64) {
        self.notch_target = target;
        self.update();
    }

    /// Value distance between notches; always a non-zero multiple of `single_step` (`QDial::notchSize`).
    pub fn notch_size(&self) -> i32 {
        let geom = self.base.geometry;
        let r = f64::from(geom.width.min(geom.height) / 2);
        let (min, max) = (self.minimum(), self.maximum());
        let page = self.page_step();
        let single = self.single_step();
        let mut l = (r * if self.wrapping() { 6.0 } else { 5.0 } * PI / 6.0).round() as i64;
        if i64::from(max) > i64::from(min) + i64::from(page) {
            l = (l as f64 * f64::from(page) / (f64::from(max) - f64::from(min))).round() as i64;
        }
        l = (l * i64::from(single) / i64::from(if page != 0 { page } else { 1 })).max(1);
        let l = ((self.notch_target / l as f64).round() as i64).max(1);
        (i64::from(single) * l).clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
    }

    /// Pointer angle in radians for `value` (0 = east, counter-clockwise positive).
    pub fn angle_for_value(&self, value: i32) -> f64 {
        let (min, max) = (f64::from(self.minimum()), f64::from(self.maximum()));
        let mut value = f64::from(value);
        if self.inverted_appearance() {
            value = max - (value - min);
        }
        if max == min {
            PI / 2.0
        } else if self.wrapping() {
            PI * 3.0 / 2.0 - (value - min) * 2.0 * PI / (max - min)
        } else {
            (PI * 8.0 - (value - min) * 10.0 * PI / (max - min)) / 6.0
        }
    }

    /// Pointer angle in degrees for `value`.
    pub fn angle_degrees_for_value(&self, value: i32) -> f64 {
        self.angle_for_value(value).to_degrees()
    }

    /// Value corresponding to a pointer angle in radians (inverse of [`Dial::angle_for_value`]).
    pub fn value_for_angle(&self, angle: f64) -> i32 {
        let mut a = angle;
        // Normalize into [-π/2, 3π/2) like `valueFromPoint`.
        while a < -PI / 2.0 {
            a += 2.0 * PI;
        }
        while a >= 3.0 * PI / 2.0 {
            a -= 2.0 * PI;
        }
        let (minimum, maximum) = (i64::from(self.minimum()), i64::from(self.maximum()));
        let (mut minv, mut dist) = (minimum, 0i64);
        if minimum < 0 {
            dist = -minimum;
            minv = 0;
        }
        let r = (maximum - minimum) as f64;
        let mut v = if self.wrapping() {
            (0.5 + minv as f64 + r * (PI * 3.0 / 2.0 - a) / (2.0 * PI)) as i64
        } else {
            (0.5 + minv as f64 + r * (PI * 4.0 / 3.0 - a) / (PI * 10.0 / 6.0)) as i64
        };
        if dist > 0 {
            v -= dist;
        }
        let bounded = self.slider_state().bound(v);
        if self.inverted_appearance() {
            self.maximum() - (bounded - self.minimum())
        } else {
            bounded
        }
    }

    /// Value under a widget-local point (`QDialPrivate::valueFromPoint`).
    pub fn value_from_point(&self, pos: Point) -> i32 {
        let yy = f64::from(self.base.geometry.height) / 2.0 - f64::from(pos.y);
        let xx = f64::from(pos.x) - f64::from(self.base.geometry.width) / 2.0;
        let a = if xx != 0.0 || yy != 0.0 { yy.atan2(xx) } else { 0.0 };
        self.value_for_angle(a)
    }
}

impl Default for Dial {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Dial {
    fn drop(&mut self) {
        // Registry cleanup only touches thread-locals through `try_with`.
        // SAFETY: Drop runs on the registration thread; no callbacks are active at this point.
        unsafe { qtrs_core::object::unregister_qobject(self.base.object_data.id) };
    }
}

impl AbstractSlider for Dial {
    fn slider_state(&self) -> &SliderState {
        &self.state
    }

    fn slider_state_mut(&mut self) -> &mut SliderState {
        &mut self.state
    }
}

impl QObject for Dial {
    leaf_qobject_common!();

    fn event(&mut self, event: &mut Event) -> bool {
        input_common::dispatch_input_event(self, event)
    }
}

impl Widget for Dial {
    leaf_widget_common!();

    fn size_hint(&self) -> Size {
        Size::new(50, 50)
    }

    fn minimum_size_hint(&self) -> Size {
        Size::new(50, 50)
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.base.enabled = enabled;
        if !enabled {
            self.mouse_down = false;
            self.set_slider_down(false);
        }
        self.update();
    }

    fn update(&mut self) {
        let rect = input_common::local_rect(&self.base);
        input_common::request_update(&mut self.base, rect);
    }

    fn set_window_id(&mut self, window_id: Option<ObjectId>) {
        self.base.window_id = window_id;
    }

    fn set_has_focus(&mut self, focus: bool) {
        self.base.has_focus = focus;
        self.update();
    }

    fn focus_in_event(&mut self, _reason: FocusReason) {
        self.update();
    }

    fn focus_out_event(&mut self, _reason: FocusReason) {
        self.update();
    }

    fn mouse_press_event(&mut self, pos: Point, button: u32, _modifiers: u32) {
        if !self.base.enabled || self.maximum() == self.minimum() || button != 1 || self.mouse_down {
            return;
        }
        self.mouse_down = true;
        let v = self.value_from_point(pos);
        self.set_slider_position(v);
        self.set_slider_down(true);
    }

    fn mouse_move_event(&mut self, pos: Point) {
        if self.mouse_down {
            let v = self.value_from_point(pos);
            self.set_slider_position(v);
        }
    }

    fn mouse_release_event(&mut self, pos: Point, button: u32, _modifiers: u32) {
        if button != 1 || !self.mouse_down {
            return;
        }
        self.mouse_down = false;
        let v = self.value_from_point(pos);
        self.set_value(v);
        self.set_slider_down(false);
    }

    fn wheel_event(&mut self, _pos: Point, delta_y: i32, modifiers: u32) {
        if self.base.enabled {
            self.slider_wheel(delta_y, modifiers);
        }
    }

    fn key_press_event(&mut self, key: u32, modifiers: u32, _is_repeat: bool) {
        if self.base.enabled {
            self.slider_key_press(key, modifiers);
        }
    }

    fn paint_event(&mut self, painter: &mut Painter) {
        let geom = self.base.geometry;
        let size = geom.width.min(geom.height) as f32;
        if size <= 4.0 {
            return;
        }
        let cx = geom.width as f32 / 2.0;
        let cy = geom.height as f32 / 2.0;
        let radius = size / 2.0 - 2.0;
        let face_radius = if self.notches_visible { radius * 0.8 } else { radius };

        // 1. Notches around the face
        if self.notches_visible {
            let (min, max) = (i64::from(self.minimum()), i64::from(self.maximum()));
            let notch = i64::from(self.notch_size().max(1));
            let page = i64::from(self.page_step().max(1));
            let mut v = min;
            while v <= max {
                let a = self.angle_for_value(v as i32) as f32;
                let major = (v - min) % page == 0;
                let inner = if major { radius * 0.82 } else { radius * 0.88 };
                painter.set_pen(Pen::new(self.notch_color, if major { 1.5 } else { 1.0 }));
                painter.draw_line(
                    PointF::new(cx + inner * a.cos(), cy - inner * a.sin()),
                    PointF::new(cx + radius * a.cos(), cy - radius * a.sin()),
                );
                v += notch;
            }
        }

        // 2. Dial face
        painter.set_brush(Brush::Color(self.face_color));
        let border = if self.base.has_focus { self.focus_ring_color } else { self.border_color };
        painter.set_pen(Pen::new(border, if self.base.has_focus { 1.5 } else { 1.0 }));
        painter.draw_ellipse(RectF::new(cx - face_radius, cy - face_radius, face_radius * 2.0, face_radius * 2.0));

        // 3. Value arc (non-wrapping only; a wrapping dial has no start/end)
        if !self.wrapping() && self.maximum() != self.minimum() {
            let start = self.angle_degrees_for_value(self.minimum()) as f32;
            let current = self.angle_degrees_for_value(self.slider_position()) as f32;
            let arc_r = face_radius - 3.0;
            if arc_r > 0.0 {
                painter.set_brush(Brush::Color(Color::TRANSPARENT));
                painter.set_pen(Pen::new(self.pointer_color, 2.0));
                painter.draw_arc(
                    RectF::new(cx - arc_r, cy - arc_r, arc_r * 2.0, arc_r * 2.0),
                    start,
                    current - start,
                );
            }
        }

        // 4. Pointer from centre to the current position
        let a = self.angle_for_value(self.slider_position()) as f32;
        let tip = face_radius * 0.75;
        let pointer = if self.base.enabled { self.pointer_color } else { self.border_color };
        painter.set_pen(Pen::new(pointer, 2.5));
        painter.draw_line(
            PointF::new(cx + face_radius * 0.2 * a.cos(), cy - face_radius * 0.2 * a.sin()),
            PointF::new(cx + tip * a.cos(), cy - tip * a.sin()),
        );
        painter.set_pen(None);
        painter.set_brush(Brush::Color(pointer));
        painter.draw_ellipse(RectF::new(cx - 3.0, cy - 3.0, 6.0, 6.0));
    }
}
