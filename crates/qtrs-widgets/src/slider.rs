//! Range-based slider controls (`QAbstractSlider`, `QSlider`).
//!
//! [`AbstractSlider`] carries the value/position/step logic shared by
//! [`Slider`] and [`crate::dial::Dial`]; it mirrors `qabstractslider.cpp`,
//! including the distinction between the committed `value` and the visual
//! `slider_position` when tracking is disabled.

use qtrs_core::event::{Event, FocusReason};
use qtrs_core::object::{ObjectId, QObject};
use qtrs_core::signal::Signal;
use qtrs_gui::geometry::primitives::{Point, PointF, Rect, RectF, Size};
use qtrs_gui::paint::brush::Brush;
use qtrs_gui::paint::painter::{Painter, Pen};
use qtrs_gui::tiny_skia::Color;

use crate::focus::FocusPolicy;
use crate::input_common;
use crate::input_keys;
use crate::scroll::Orientation;
use crate::size_policy::{Policy, QSizePolicy};
use crate::widget::{Widget, WidgetBase};

/// Actions that can be triggered on a slider (`QAbstractSlider::SliderAction`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SliderAction {
    #[default]
    NoAction,
    SingleStepAdd,
    SingleStepSub,
    PageStepAdd,
    PageStepSub,
    ToMinimum,
    ToMaximum,
    Move,
}

pub type QSliderAction = SliderAction;

/// Kind of state change reported to [`AbstractSlider::slider_change`] (`QAbstractSlider::SliderChange`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SliderChange {
    RangeChange,
    OrientationChange,
    StepsChange,
    ValueChange,
}

/// State and signals shared by every slider-like widget (`QAbstractSliderPrivate`).
pub struct SliderState {
    minimum: i32,
    maximum: i32,
    value: i32,
    position: i32,
    single_step: i32,
    page_step: i32,
    tracking: bool,
    block_tracking: bool,
    pressed: bool,
    inverted_appearance: bool,
    inverted_controls: bool,
    orientation: Orientation,
    wrapping: bool,
    offset_accumulated: f64,

    /// Emitted when the committed value changes (`valueChanged(int)`).
    pub value_changed: Signal<i32>,
    /// Emitted when the user presses the slider handle (`sliderPressed()`).
    pub slider_pressed: Signal<()>,
    /// Emitted while the handle is dragged (`sliderMoved(int)`).
    pub slider_moved: Signal<i32>,
    /// Emitted when the handle is released (`sliderReleased()`).
    pub slider_released: Signal<()>,
    /// Emitted with `(minimum, maximum)` when the range changes (`rangeChanged(int,int)`).
    pub range_changed: Signal<(i32, i32)>,
    /// Emitted for every triggered action (`actionTriggered(int)`).
    pub action_triggered: Signal<SliderAction>,
}

impl SliderState {
    /// Qt defaults: range `0..=99`, single step 1, page step 10, tracking enabled.
    pub fn new(orientation: Orientation) -> Self {
        Self {
            minimum: 0,
            maximum: 99,
            value: 0,
            position: 0,
            single_step: 1,
            page_step: 10,
            tracking: true,
            block_tracking: false,
            pressed: false,
            inverted_appearance: false,
            inverted_controls: false,
            orientation,
            wrapping: false,
            offset_accumulated: 0.0,
            value_changed: Signal::new(),
            slider_pressed: Signal::new(),
            slider_moved: Signal::new(),
            slider_released: Signal::new(),
            range_changed: Signal::new(),
            action_triggered: Signal::new(),
        }
    }

    /// Clamps (or, with wrapping enabled, wraps) `value` into `[minimum, maximum]`.
    pub fn bound(&self, value: i64) -> i32 {
        let min = i64::from(self.minimum);
        let max = i64::from(self.maximum);
        if self.wrapping && max > min {
            if (min..=max).contains(&value) {
                return value as i32;
            }
            let span = max - min;
            let mut wrapped = min + (value - min) % span;
            if wrapped < min {
                wrapped += span;
            }
            wrapped as i32
        } else {
            value.clamp(min, max) as i32
        }
    }

    fn overflow_safe_add(&self, step: i32) -> i32 {
        self.bound(i64::from(self.position) + i64::from(step))
    }

    pub(crate) fn set_wrapping(&mut self, wrapping: bool) {
        self.wrapping = wrapping;
    }

    pub(crate) fn wrapping(&self) -> bool {
        self.wrapping
    }
}

/// Shared slider behavior (`QAbstractSlider`).
///
/// Implementors expose their [`SliderState`]; every range, stepping, tracking and
/// keyboard/wheel rule is provided here.
pub trait AbstractSlider: Widget {
    fn slider_state(&self) -> &SliderState;
    fn slider_state_mut(&mut self) -> &mut SliderState;

    /// Hook invoked after range/orientation/step/value changes (`sliderChange`).
    fn slider_change(&mut self, _change: SliderChange) {
        self.update();
    }

    fn minimum(&self) -> i32 {
        self.slider_state().minimum
    }

    fn maximum(&self) -> i32 {
        self.slider_state().maximum
    }

    fn set_minimum(&mut self, min: i32) {
        let max = self.maximum().max(min);
        self.set_range(min, max);
    }

    fn set_maximum(&mut self, max: i32) {
        let min = self.minimum().min(max);
        self.set_range(min, max);
    }

    /// Sets the range; `max` is raised to `min` if smaller. The value is re-bounded.
    fn set_range(&mut self, min: i32, max: i32) {
        let (old_min, old_max) = (self.minimum(), self.maximum());
        {
            let s = self.slider_state_mut();
            s.minimum = min;
            s.maximum = max.max(min);
        }
        if (old_min, old_max) != (self.minimum(), self.maximum()) {
            let range = (self.minimum(), self.maximum());
            self.slider_state().range_changed.emit(&range);
            self.slider_change(SliderChange::RangeChange);
            let value = self.value();
            self.set_value(value);
        }
    }

    fn value(&self) -> i32 {
        self.slider_state().value
    }

    /// Sets the committed value (bounded), emitting `value_changed` when it changes.
    fn set_value(&mut self, value: i32) {
        let value = self.slider_state().bound(i64::from(value));
        let s = self.slider_state();
        if s.value == value && s.position == value {
            return;
        }
        let emit_moved = {
            let s = self.slider_state_mut();
            s.value = value;
            if s.position != value {
                s.position = value;
                s.pressed
            } else {
                false
            }
        };
        if emit_moved {
            self.slider_state().slider_moved.emit(&value);
        }
        self.slider_change(SliderChange::ValueChange);
        self.slider_state().value_changed.emit(&value);
    }

    fn slider_position(&self) -> i32 {
        self.slider_state().position
    }

    /// Moves the visual handle; commits the value immediately only when tracking.
    fn set_slider_position(&mut self, position: i32) {
        let position = self.slider_state().bound(i64::from(position));
        if position == self.slider_state().position {
            return;
        }
        self.slider_state_mut().position = position;
        let (tracking, pressed, blocked) = {
            let s = self.slider_state();
            (s.tracking, s.pressed, s.block_tracking)
        };
        if !tracking {
            self.update();
        }
        if pressed {
            self.slider_state().slider_moved.emit(&position);
        }
        if tracking && !blocked {
            self.trigger_action(SliderAction::Move);
        }
    }

    fn single_step(&self) -> i32 {
        self.slider_state().single_step
    }

    fn set_single_step(&mut self, step: i32) {
        let step = step.saturating_abs();
        if step != self.single_step() {
            self.slider_state_mut().single_step = step;
            self.slider_change(SliderChange::StepsChange);
        }
    }

    fn page_step(&self) -> i32 {
        self.slider_state().page_step
    }

    fn set_page_step(&mut self, step: i32) {
        let step = step.saturating_abs();
        if step != self.page_step() {
            self.slider_state_mut().page_step = step;
            self.slider_change(SliderChange::StepsChange);
        }
    }

    fn has_tracking(&self) -> bool {
        self.slider_state().tracking
    }

    fn set_tracking(&mut self, enable: bool) {
        self.slider_state_mut().tracking = enable;
    }

    fn is_slider_down(&self) -> bool {
        self.slider_state().pressed
    }

    /// Marks the handle as pressed/released; releasing commits a pending position.
    fn set_slider_down(&mut self, down: bool) {
        let changed = self.slider_state().pressed != down;
        self.slider_state_mut().pressed = down;
        if changed {
            if down {
                self.slider_state().slider_pressed.emit(&());
            } else {
                self.slider_state().slider_released.emit(&());
            }
        }
        if !down && self.slider_position() != self.value() {
            self.trigger_action(SliderAction::Move);
        }
    }

    fn inverted_appearance(&self) -> bool {
        self.slider_state().inverted_appearance
    }

    fn set_inverted_appearance(&mut self, invert: bool) {
        self.slider_state_mut().inverted_appearance = invert;
        self.update();
    }

    fn inverted_controls(&self) -> bool {
        self.slider_state().inverted_controls
    }

    fn set_inverted_controls(&mut self, invert: bool) {
        self.slider_state_mut().inverted_controls = invert;
    }

    fn orientation(&self) -> Orientation {
        self.slider_state().orientation
    }

    fn set_orientation(&mut self, orientation: Orientation) {
        if self.orientation() == orientation {
            return;
        }
        self.slider_state_mut().orientation = orientation;
        let policy = transposed_policy(self.size_policy());
        self.set_size_policy(policy);
        self.slider_change(SliderChange::OrientationChange);
    }

    /// Performs `action` and commits the resulting position (`triggerAction`).
    fn trigger_action(&mut self, action: SliderAction) {
        self.slider_state_mut().block_tracking = true;
        let target = {
            let s = self.slider_state();
            match action {
                SliderAction::SingleStepAdd => Some(s.overflow_safe_add(s.single_step)),
                SliderAction::SingleStepSub => Some(s.overflow_safe_add(-s.single_step)),
                SliderAction::PageStepAdd => Some(s.overflow_safe_add(s.page_step)),
                SliderAction::PageStepSub => Some(s.overflow_safe_add(-s.page_step)),
                SliderAction::ToMinimum => Some(s.minimum),
                SliderAction::ToMaximum => Some(s.maximum),
                SliderAction::Move | SliderAction::NoAction => None,
            }
        };
        if let Some(target) = target {
            self.set_slider_position(target);
        }
        self.slider_state().action_triggered.emit(&action);
        self.slider_state_mut().block_tracking = false;
        let position = self.slider_position();
        self.set_value(position);
    }

    /// Keyboard handling from `QAbstractSlider::keyPressEvent`. Returns `true` if consumed.
    fn slider_key_press(&mut self, key: u32, _modifiers: u32) -> bool {
        let inverted = self.inverted_controls();
        let (add, sub) = if inverted {
            (SliderAction::SingleStepSub, SliderAction::SingleStepAdd)
        } else {
            (SliderAction::SingleStepAdd, SliderAction::SingleStepSub)
        };
        let action = if input_keys::is_left(key) || input_keys::is_down(key) {
            sub
        } else if input_keys::is_right(key) || input_keys::is_up(key) {
            add
        } else if input_keys::is_page_up(key) {
            if inverted {
                SliderAction::PageStepSub
            } else {
                SliderAction::PageStepAdd
            }
        } else if input_keys::is_page_down(key) {
            if inverted {
                SliderAction::PageStepAdd
            } else {
                SliderAction::PageStepSub
            }
        } else if input_keys::is_home(key) {
            SliderAction::ToMinimum
        } else if input_keys::is_end(key) {
            SliderAction::ToMaximum
        } else {
            return false;
        };
        self.trigger_action(action);
        true
    }

    /// Wheel handling from `QAbstractSliderPrivate::scrollByDelta`.
    ///
    /// Each 120-unit notch scrolls three single steps (bounded by a page);
    /// Ctrl/Shift scroll a whole page. Returns `true` if the value changed or a
    /// partial scroll is pending.
    fn slider_wheel(&mut self, delta_y: i32, modifiers: u32) -> bool {
        const WHEEL_SCROLL_LINES: f64 = 3.0;
        let offset = f64::from(delta_y) / 120.0;
        let page = self.page_step();
        let steps = if input_keys::has_ctrl(modifiers) || input_keys::has_shift(modifiers) {
            self.slider_state_mut().offset_accumulated = 0.0;
            ((offset * f64::from(page)) as i32).clamp(-page, page)
        } else {
            let steps_f = WHEEL_SCROLL_LINES * offset * f64::from(self.single_step());
            let s = self.slider_state_mut();
            if s.offset_accumulated != 0.0 && (offset / s.offset_accumulated) < 0.0 {
                s.offset_accumulated = 0.0;
            }
            s.offset_accumulated += steps_f;
            let steps = (s.offset_accumulated as i32).clamp(-page, page);
            s.offset_accumulated -= f64::from(s.offset_accumulated as i32);
            if steps == 0 {
                let effective = if s.inverted_controls {
                    -s.offset_accumulated
                } else {
                    s.offset_accumulated
                };
                if effective > 0.0 && s.value < s.maximum {
                    return true;
                }
                if effective < 0.0 && s.value > s.minimum {
                    return true;
                }
                s.offset_accumulated = 0.0;
                return false;
            }
            steps
        };
        let steps = if self.inverted_controls() {
            -steps
        } else {
            steps
        };
        let previous = self.value();
        {
            let s = self.slider_state_mut();
            s.position = s.overflow_safe_add(steps);
        }
        self.trigger_action(SliderAction::Move);
        if previous == self.value() {
            self.slider_state_mut().offset_accumulated = 0.0;
            return false;
        }
        true
    }
}

pub use AbstractSlider as QAbstractSlider;

/// Swaps horizontal and vertical policies (`QSizePolicy::transpose`).
pub(crate) fn transposed_policy(policy: QSizePolicy) -> QSizePolicy {
    QSizePolicy {
        horizontal: policy.vertical,
        vertical: policy.horizontal,
        horizontal_stretch: policy.vertical_stretch,
        vertical_stretch: policy.horizontal_stretch,
    }
}

/// Converts a logical value to a pixel offset in `[0, span]` (`QStyle::sliderPositionFromValue`).
pub fn slider_position_from_value(
    min: i32,
    max: i32,
    value: i32,
    span: i32,
    upside_down: bool,
) -> i32 {
    if span <= 0 || max <= min {
        return 0;
    }
    if value < min {
        return if upside_down { span } else { 0 };
    }
    if value > max {
        return if upside_down { 0 } else { span };
    }
    let range = (i64::from(max) - i64::from(min)) as u128;
    let p = if upside_down {
        i64::from(max) - i64::from(value)
    } else {
        i64::from(value) - i64::from(min)
    } as u128;
    let span = span as u128;
    ((2 * p * span + range) / (2 * range)) as i32
}

/// Converts a pixel offset in `[0, span]` to a logical value (`QStyle::sliderValueFromPosition`).
pub fn slider_value_from_position(
    min: i32,
    max: i32,
    pos: i32,
    span: i32,
    upside_down: bool,
) -> i32 {
    if span <= 0 || pos <= 0 {
        return if upside_down { max } else { min };
    }
    if pos >= span {
        return if upside_down { min } else { max };
    }
    let range = (i64::from(max) - i64::from(min)).max(0) as u128;
    let tmp = ((2 * pos as u128 * range + span as u128) / (2 * span as u128)) as i64;
    if upside_down {
        (i64::from(max) - tmp) as i32
    } else {
        (tmp + i64::from(min)) as i32
    }
}

/// Where tick marks are drawn relative to the groove (`QSlider::TickPosition`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TickPosition {
    #[default]
    NoTicks,
    /// Above a horizontal slider, left of a vertical one (`TicksAbove` / `TicksLeft`).
    TicksAbove,
    /// Below a horizontal slider, right of a vertical one (`TicksBelow` / `TicksRight`).
    TicksBelow,
    TicksBothSides,
}

impl TickPosition {
    pub const TICKS_LEFT: TickPosition = TickPosition::TicksAbove;
    pub const TICKS_RIGHT: TickPosition = TickPosition::TicksBelow;
}

/// Length of the slider handle along the groove, in pixels.
const HANDLE_LENGTH: i32 = 12;
/// Space reserved for tick marks on each ticked side.
const TICK_SPACE: i32 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SliderPress {
    None,
    Handle,
    Groove,
}

/// Linear slider with a draggable handle (`QSlider`).
pub struct Slider {
    base: WidgetBase,
    state: SliderState,
    tick_position: TickPosition,
    tick_interval: i32,
    pressed_control: SliderPress,
    click_offset: i32,
    handle_hovered: bool,

    groove_color: Color,
    fill_color: Color,
    handle_color: Color,
    handle_border_color: Color,
    tick_color: Color,
    focus_ring_color: Color,
}

pub type QSlider = Slider;

impl Slider {
    /// Creates a vertical slider (Qt's default orientation for `QSlider`).
    pub fn new() -> Self {
        Self::with_orientation(Orientation::Vertical)
    }

    pub fn with_orientation(orientation: Orientation) -> Self {
        let mut base = WidgetBase::new();
        base.focus_policy = FocusPolicy::StrongFocus;
        let policy = QSizePolicy::new(Policy::Expanding, Policy::Fixed);
        base.size_policy = match orientation {
            Orientation::Horizontal => policy,
            Orientation::Vertical => transposed_policy(policy),
        };
        let mut slider = Self {
            base,
            state: SliderState::new(orientation),
            tick_position: TickPosition::NoTicks,
            tick_interval: 0,
            pressed_control: SliderPress::None,
            click_offset: 0,
            handle_hovered: false,
            groove_color: Color::from_rgba8(200, 200, 200, 255),
            fill_color: Color::from_rgba8(0, 120, 215, 255),
            handle_color: Color::from_rgba8(255, 255, 255, 255),
            handle_border_color: Color::from_rgba8(140, 140, 140, 255),
            tick_color: Color::from_rgba8(110, 110, 110, 255),
            focus_ring_color: Color::from_rgba8(0, 120, 215, 180),
        };
        let hint = slider.size_hint();
        slider.base.geometry = Rect::new(0, 0, hint.width, hint.height);
        slider.base.dirty = Some(Rect::new(0, 0, hint.width, hint.height));
        slider
    }

    pub fn tick_position(&self) -> TickPosition {
        self.tick_position
    }

    pub fn set_tick_position(&mut self, position: TickPosition) {
        self.tick_position = position;
        self.update();
    }

    pub fn tick_interval(&self) -> i32 {
        self.tick_interval
    }

    /// Sets the value interval between ticks; `0` picks single or page step automatically.
    pub fn set_tick_interval(&mut self, interval: i32) {
        self.tick_interval = interval.max(0);
        self.update();
    }

    /// Values at which tick marks are drawn (QCommonStyle `CC_Slider` algorithm).
    pub fn tick_values(&self) -> Vec<i32> {
        if self.tick_position == TickPosition::NoTicks {
            return Vec::new();
        }
        let (min, max) = (self.minimum(), self.maximum());
        let span = self.span();
        let mut interval = self.tick_interval;
        if interval <= 0 {
            interval = self.single_step();
            let step_px = slider_position_from_value(min, max, interval, span, false)
                - slider_position_from_value(min, max, 0, span, false);
            if step_px < 3 {
                interval = self.page_step();
            }
        }
        if interval <= 0 {
            interval = 1;
        }
        let mut ticks = Vec::new();
        let mut v = i64::from(min);
        let last = i64::from(max) + 1;
        while v <= last {
            if v == last && interval == 1 {
                break;
            }
            ticks.push(v.min(i64::from(max)) as i32);
            v += i64::from(interval);
        }
        ticks
    }

    /// Pixel range the handle's leading edge can travel.
    fn span(&self) -> i32 {
        (self.along_extent() - HANDLE_LENGTH).max(0)
    }

    fn along_extent(&self) -> i32 {
        match self.orientation() {
            Orientation::Horizontal => self.base.geometry.width,
            Orientation::Vertical => self.base.geometry.height,
        }
    }

    fn across_extent(&self) -> i32 {
        match self.orientation() {
            Orientation::Horizontal => self.base.geometry.height,
            Orientation::Vertical => self.base.geometry.width,
        }
    }

    fn upside_down(&self) -> bool {
        match self.orientation() {
            Orientation::Horizontal => self.inverted_appearance(),
            Orientation::Vertical => !self.inverted_appearance(),
        }
    }

    fn pick(&self, pos: Point) -> i32 {
        match self.orientation() {
            Orientation::Horizontal => pos.x,
            Orientation::Vertical => pos.y,
        }
    }

    /// Offset of the handle's leading edge for the current slider position.
    pub fn handle_offset(&self) -> i32 {
        slider_position_from_value(
            self.minimum(),
            self.maximum(),
            self.slider_position(),
            self.span(),
            self.upside_down(),
        )
    }

    /// Handle rectangle in widget-local coordinates.
    pub fn handle_rect(&self) -> Rect {
        let offset = self.handle_offset();
        match self.orientation() {
            Orientation::Horizontal => {
                Rect::new(offset, 0, HANDLE_LENGTH, self.base.geometry.height)
            }
            Orientation::Vertical => Rect::new(0, offset, self.base.geometry.width, HANDLE_LENGTH),
        }
    }

    /// Maps a handle leading-edge pixel offset to a value (`pixelPosToRangeValue`).
    pub fn pixel_pos_to_value(&self, pixel: i32) -> i32 {
        slider_value_from_position(
            self.minimum(),
            self.maximum(),
            pixel,
            self.span(),
            self.upside_down(),
        )
    }

    fn handle_contains(&self, pos: Point) -> bool {
        let along = self.pick(pos);
        let offset = self.handle_offset();
        along >= offset && along < offset + HANDLE_LENGTH
    }
}

impl Default for Slider {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Slider {
    fn drop(&mut self) {
        // Registry cleanup only touches thread-locals through `try_with`.
        // SAFETY: Drop runs on the registration thread; no callbacks are active at this point.
        unsafe { qtrs_core::object::unregister_qobject(self.base.object_data.id) };
    }
}

impl AbstractSlider for Slider {
    fn slider_state(&self) -> &SliderState {
        &self.state
    }

    fn slider_state_mut(&mut self) -> &mut SliderState {
        &mut self.state
    }
}

impl QObject for Slider {
    leaf_qobject_common!();

    fn event(&mut self, event: &mut Event) -> bool {
        input_common::dispatch_input_event(self, event)
    }
}

impl Widget for Slider {
    leaf_widget_common!();

    fn size_hint(&self) -> Size {
        let ticks = match self.tick_position {
            TickPosition::NoTicks => 0,
            TickPosition::TicksAbove | TickPosition::TicksBelow => TICK_SPACE,
            TickPosition::TicksBothSides => 2 * TICK_SPACE,
        };
        let thick = 20 + ticks;
        match self.orientation() {
            Orientation::Horizontal => Size::new(84, thick),
            Orientation::Vertical => Size::new(thick, 84),
        }
    }

    fn minimum_size_hint(&self) -> Size {
        let hint = self.size_hint();
        match self.orientation() {
            Orientation::Horizontal => Size::new(HANDLE_LENGTH * 2, hint.height),
            Orientation::Vertical => Size::new(hint.width, HANDLE_LENGTH * 2),
        }
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.base.enabled = enabled;
        if !enabled {
            self.pressed_control = SliderPress::None;
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
        if !self.base.enabled
            || self.maximum() == self.minimum()
            || self.pressed_control != SliderPress::None
        {
            return;
        }
        match button {
            // Middle button: jump the handle centre to the click and start dragging.
            3 => {
                let center = HANDLE_LENGTH / 2;
                let target = self.pixel_pos_to_value(self.pick(pos) - center);
                self.set_slider_position(target);
                self.trigger_action(SliderAction::Move);
                self.pressed_control = SliderPress::Handle;
                self.click_offset = center;
                self.set_slider_down(true);
            }
            1 => {
                if self.handle_contains(pos) {
                    self.pressed_control = SliderPress::Handle;
                    self.click_offset = self.pick(pos) - self.handle_offset();
                    self.set_slider_down(true);
                } else {
                    // Groove click: page towards the pointer.
                    self.pressed_control = SliderPress::Groove;
                    let press_value = self.pixel_pos_to_value(self.pick(pos) - HANDLE_LENGTH / 2);
                    let action = match press_value.cmp(&self.value()) {
                        std::cmp::Ordering::Greater => SliderAction::PageStepAdd,
                        std::cmp::Ordering::Less => SliderAction::PageStepSub,
                        std::cmp::Ordering::Equal => SliderAction::NoAction,
                    };
                    if action != SliderAction::NoAction {
                        self.trigger_action(action);
                    }
                }
            }
            _ => {}
        }
        self.update();
    }

    fn mouse_move_event(&mut self, pos: Point) {
        if self.pressed_control == SliderPress::Handle {
            let target = self.pixel_pos_to_value(self.pick(pos) - self.click_offset);
            self.set_slider_position(target);
            return;
        }
        let hovered = self.handle_contains(pos);
        if hovered != self.handle_hovered {
            self.handle_hovered = hovered;
            self.update();
        }
    }

    fn mouse_release_event(&mut self, _pos: Point, button: u32, _modifiers: u32) {
        if self.pressed_control == SliderPress::None || !(button == 1 || button == 3) {
            return;
        }
        let was_handle = self.pressed_control == SliderPress::Handle;
        self.pressed_control = SliderPress::None;
        if was_handle {
            self.set_slider_down(false);
        }
        self.update();
    }

    fn leave_event(&mut self) {
        if self.handle_hovered {
            self.handle_hovered = false;
            self.update();
        }
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
        let horizontal = self.orientation() == Orientation::Horizontal;
        let along = self.along_extent() as f32;
        let across = self.across_extent() as f32;
        let groove_thickness = 4.0f32;
        let (lead, trail) = match self.tick_position {
            TickPosition::NoTicks => (0.0, 0.0),
            TickPosition::TicksAbove => (TICK_SPACE as f32, 0.0),
            TickPosition::TicksBelow => (0.0, TICK_SPACE as f32),
            TickPosition::TicksBothSides => (TICK_SPACE as f32, TICK_SPACE as f32),
        };
        let center = lead + (across - lead - trail) / 2.0;
        let half_handle = HANDLE_LENGTH as f32 / 2.0;
        let groove_len = (along - HANDLE_LENGTH as f32).max(0.0);

        // Map (along, across) to widget coordinates.
        let rect_at = |a: f32, c: f32, len_a: f32, len_c: f32| {
            if horizontal {
                RectF::new(a, c, len_a, len_c)
            } else {
                RectF::new(c, a, len_c, len_a)
            }
        };
        let point_at = |a: f32, c: f32| {
            if horizontal {
                PointF::new(a, c)
            } else {
                PointF::new(c, a)
            }
        };

        // 1. Groove
        painter.set_pen(None);
        painter.set_brush(Brush::Color(self.groove_color));
        let groove = rect_at(
            half_handle,
            center - groove_thickness / 2.0,
            groove_len,
            groove_thickness,
        );
        painter.draw_rounded_rect(groove, 2.0, 2.0);

        // 2. Filled part between the minimum end and the handle
        let handle_offset = self.handle_offset() as f32;
        let handle_center = handle_offset + half_handle;
        let (fill_start, fill_end) = if self.upside_down() {
            (handle_center, along - half_handle)
        } else {
            (half_handle, handle_center)
        };
        if fill_end > fill_start {
            painter.set_brush(Brush::Color(if self.base.enabled {
                self.fill_color
            } else {
                self.groove_color
            }));
            painter.draw_rounded_rect(
                rect_at(
                    fill_start,
                    center - groove_thickness / 2.0,
                    fill_end - fill_start,
                    groove_thickness,
                ),
                2.0,
                2.0,
            );
        }

        // 3. Tick marks
        let ticks = self.tick_values();
        if !ticks.is_empty() {
            painter.set_pen(Pen::new(self.tick_color, 1.0));
            let span = self.span();
            for v in ticks {
                let p = slider_position_from_value(
                    self.minimum(),
                    self.maximum(),
                    v,
                    span,
                    self.upside_down(),
                ) as f32
                    + half_handle
                    + 0.5;
                if matches!(
                    self.tick_position,
                    TickPosition::TicksAbove | TickPosition::TicksBothSides
                ) {
                    painter.draw_line(point_at(p, 0.0), point_at(p, TICK_SPACE as f32 - 1.0));
                }
                if matches!(
                    self.tick_position,
                    TickPosition::TicksBelow | TickPosition::TicksBothSides
                ) {
                    painter.draw_line(
                        point_at(p, across - TICK_SPACE as f32 + 1.0),
                        point_at(p, across),
                    );
                }
            }
        }

        // 4. Handle
        let handle_thickness = (across - lead - trail).clamp(0.0, 20.0);
        let handle = rect_at(
            handle_offset,
            center - handle_thickness / 2.0,
            HANDLE_LENGTH as f32,
            handle_thickness,
        );
        painter.set_brush(Brush::Color(self.handle_color));
        let border = if self.is_slider_down() || self.handle_hovered || self.base.has_focus {
            self.fill_color
        } else {
            self.handle_border_color
        };
        painter.set_pen(Pen::new(
            border,
            if self.base.has_focus { 1.5 } else { 1.0 },
        ));
        painter.draw_rounded_rect(handle, 3.0, 3.0);

        // 5. Focus ring
        if self.base.has_focus {
            painter.set_brush(Brush::Color(Color::TRANSPARENT));
            painter.set_pen(Pen::new(self.focus_ring_color, 1.0));
            let w = self.base.geometry.width as f32;
            let h = self.base.geometry.height as f32;
            painter.draw_rounded_rect(
                RectF::new(0.5, 0.5, (w - 1.0).max(0.0), (h - 1.0).max(0.0)),
                2.0,
                2.0,
            );
        }
    }
}
