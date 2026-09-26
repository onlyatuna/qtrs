//! Numeric spin boxes (`QAbstractSpinBox`, `QSpinBox`, `QDoubleSpinBox`).
//!
//! [`NumericSpinBox<T>`] implements the shared spin box engine: bounded and
//! wrapping stepping (`QAbstractSpinBoxPrivate::bound`), prefix/suffix and
//! special-value text, validated in-place editing with keyboard tracking,
//! key/wheel/mouse stepping. [`SpinBox`] (`i32`) and [`DoubleSpinBox`] (`f64`)
//! are its two instantiations.

use qtrs_core::event::{Event, EventKind, FocusReason};
use qtrs_core::object::{ObjectId, QObject};
use qtrs_core::signal::Signal;
use qtrs_gui::geometry::primitives::{Point, PointF, Rect, RectF, Size};
use qtrs_gui::paint::brush::Brush;
use qtrs_gui::paint::painter::{Painter, Pen};
use qtrs_gui::text::{Font, FontMetrics};
use qtrs_gui::tiny_skia::Color;

use crate::focus::FocusPolicy;
use crate::input_common;
use crate::input_keys;
use crate::size_policy::{Policy, QSizePolicy};
use crate::widget::{Widget, WidgetBase};

/// Number types usable in a [`NumericSpinBox`].
pub trait SpinNumber: Copy + PartialOrd + std::fmt::Debug + Send + Sync + 'static {
    const DEFAULT_MINIMUM: Self;
    const DEFAULT_MAXIMUM: Self;
    const DEFAULT_STEP: Self;
    const DEFAULT_DECIMALS: u32;

    fn to_f64(self) -> f64;
    /// Converts back from `f64`, saturating at the type's limits.
    fn from_f64_saturating(value: f64) -> Self;
    /// Rounds to the displayed precision (identity for integers).
    fn round_to(self, decimals: u32) -> Self;
    /// Renders the bare number (no prefix/suffix).
    fn format_number(self, decimals: u32, base: u32) -> String;
    /// Parses a complete number; `None` if `text` is not a finished number.
    fn parse_number(text: &str, decimals: u32, base: u32) -> Option<Self>;
    /// Whether `text` could be the beginning of a number (sign, digits, separator).
    fn is_partial_syntax(text: &str, decimals: u32, base: u32) -> bool;
}

fn split_sign(text: &str) -> (bool, &str) {
    if let Some(rest) = text.strip_prefix('-') {
        (true, rest)
    } else if let Some(rest) = text.strip_prefix('+') {
        (false, rest)
    } else {
        (false, text)
    }
}

impl SpinNumber for i32 {
    const DEFAULT_MINIMUM: Self = 0;
    const DEFAULT_MAXIMUM: Self = 99;
    const DEFAULT_STEP: Self = 1;
    const DEFAULT_DECIMALS: u32 = 0;

    fn to_f64(self) -> f64 {
        f64::from(self)
    }

    fn from_f64_saturating(value: f64) -> Self {
        value
            .round()
            .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32
    }

    fn round_to(self, _decimals: u32) -> Self {
        self
    }

    fn format_number(self, _decimals: u32, base: u32) -> String {
        if base == 10 {
            return self.to_string();
        }
        let mut magnitude = i64::from(self).unsigned_abs();
        let mut digits = Vec::new();
        loop {
            let d = (magnitude % u64::from(base)) as u32;
            digits.push(char::from_digit(d, base).unwrap_or('0'));
            magnitude /= u64::from(base);
            if magnitude == 0 {
                break;
            }
        }
        let mut out = String::with_capacity(digits.len() + 1);
        if self < 0 {
            out.push('-');
        }
        out.extend(digits.iter().rev());
        out
    }

    fn parse_number(text: &str, _decimals: u32, base: u32) -> Option<Self> {
        let (negative, digits) = split_sign(text);
        if digits.is_empty() {
            return None;
        }
        let magnitude = i64::from_str_radix(digits, base).ok()?;
        i32::try_from(if negative { -magnitude } else { magnitude }).ok()
    }

    fn is_partial_syntax(text: &str, _decimals: u32, base: u32) -> bool {
        let (_, digits) = split_sign(text);
        digits.chars().all(|c| c.is_digit(base))
    }
}

impl SpinNumber for f64 {
    const DEFAULT_MINIMUM: Self = 0.0;
    const DEFAULT_MAXIMUM: Self = 99.99;
    const DEFAULT_STEP: Self = 1.0;
    const DEFAULT_DECIMALS: u32 = 2;

    fn to_f64(self) -> f64 {
        self
    }

    fn from_f64_saturating(value: f64) -> Self {
        value
    }

    fn round_to(self, decimals: u32) -> Self {
        if !self.is_finite() {
            return self;
        }
        // Same approach as QDoubleSpinBoxPrivate::round: print with the precision, parse back.
        format!("{:.*}", decimals as usize, self)
            .parse()
            .unwrap_or(self)
    }

    fn format_number(self, decimals: u32, _base: u32) -> String {
        let v = if self == 0.0 { 0.0 } else { self };
        let text = format!("{:.*}", decimals as usize, v);
        // Avoid "-0.00" for tiny negatives that round to zero.
        if text.starts_with('-') && text[1..].chars().all(|c| c == '0' || c == '.') {
            text[1..].to_string()
        } else {
            text
        }
    }

    fn parse_number(text: &str, decimals: u32, _base: u32) -> Option<Self> {
        if !Self::is_partial_syntax(text, decimals, 10) {
            return None;
        }
        let (_, body) = split_sign(text);
        if !body.chars().any(|c| c.is_ascii_digit()) {
            return None;
        }
        text.parse().ok()
    }

    fn is_partial_syntax(text: &str, decimals: u32, _base: u32) -> bool {
        let (_, body) = split_sign(text);
        let mut parts = body.splitn(2, '.');
        let int_part = parts.next().unwrap_or("");
        let frac_part = parts.next();
        if !int_part.chars().all(|c| c.is_ascii_digit()) {
            return false;
        }
        match frac_part {
            None => true,
            Some(frac) => {
                decimals > 0
                    && frac.len() <= decimals as usize
                    && frac.chars().all(|c| c.is_ascii_digit())
            }
        }
    }
}

/// Result of validating edited spin box text (`QValidator::State`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidatorState {
    Invalid,
    Intermediate,
    Acceptable,
}

/// Which step buttons are drawn (`QAbstractSpinBox::ButtonSymbols`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonSymbols {
    #[default]
    UpDownArrows,
    PlusMinus,
    NoButtons,
}

/// Which step directions are currently available (`QAbstractSpinBox::StepEnabled`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StepEnabled {
    pub up: bool,
    pub down: bool,
}

const BUTTON_WIDTH: i32 = 18;
const TEXT_PAD: f32 = 6.0;

/// Generic numeric spin box engine shared by [`SpinBox`] and [`DoubleSpinBox`].
pub struct NumericSpinBox<T: SpinNumber> {
    base: WidgetBase,
    value: T,
    minimum: T,
    maximum: T,
    single_step: T,
    decimals: u32,
    display_integer_base: u32,
    wrapping: bool,
    keyboard_tracking: bool,
    read_only: bool,
    prefix: String,
    suffix: String,
    special_value_text: String,
    button_symbols: ButtonSymbols,

    /// Clean (prefix/suffix-less) text being edited, `None` when showing the committed value.
    edit_buffer: Option<String>,
    /// Cursor position (in chars) inside the clean text.
    cursor: usize,
    /// Whole text selected, so the next typed character replaces it.
    all_selected: bool,
    /// Last text reported through `text_changed`.
    last_text: String,
    wheel_remainder: i32,
    pressed_button: Option<bool>,

    font: Font,
    bg_color: Color,
    text_color: Color,
    border_color: Color,
    focus_border_color: Color,
    button_color: Color,
    arrow_color: Color,

    /// Emitted when the committed value changes (`valueChanged`).
    pub value_changed: Signal<T>,
    /// Emitted with the full displayed text (prefix + number + suffix) when it changes (`textChanged`).
    pub text_changed: Signal<String>,
    /// Emitted on Return/Enter or focus loss (`editingFinished`).
    pub editing_finished: Signal<()>,
}

/// Integer spin box (`QSpinBox`).
pub type SpinBox = NumericSpinBox<i32>;
pub type QSpinBox = SpinBox;
/// Floating-point spin box (`QDoubleSpinBox`).
pub type DoubleSpinBox = NumericSpinBox<f64>;
pub type QDoubleSpinBox = DoubleSpinBox;
pub type QAbstractSpinBox<T> = NumericSpinBox<T>;

impl NumericSpinBox<i32> {
    /// Creates a spin box with range `0..=99`, step 1 and value 0.
    pub fn new() -> Self {
        Self::with_defaults()
    }

    pub fn display_integer_base(&self) -> u32 {
        self.display_integer_base
    }

    /// Sets the radix used to display and parse values (2..=36; others are ignored).
    pub fn set_display_integer_base(&mut self, base: u32) {
        if (2..=36).contains(&base) && base != self.display_integer_base {
            self.display_integer_base = base;
            self.refresh_text();
        }
    }
}

impl Default for NumericSpinBox<i32> {
    fn default() -> Self {
        Self::new()
    }
}

impl NumericSpinBox<f64> {
    /// Creates a double spin box with range `0.0..=99.99`, step 1.0 and 2 decimals.
    pub fn new() -> Self {
        Self::with_defaults()
    }

    pub fn decimals(&self) -> u32 {
        self.decimals
    }

    /// Sets the displayed precision (clamped to `0..=323`) and re-rounds range and value.
    pub fn set_decimals(&mut self, decimals: u32) {
        self.decimals = decimals.min(323);
        let (min, max) = (self.minimum, self.maximum);
        self.set_range(min, max);
        let value = self.value;
        self.set_value(value);
        self.refresh_text();
    }
}

impl Default for NumericSpinBox<f64> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: SpinNumber> NumericSpinBox<T> {
    fn with_defaults() -> Self {
        let font = Font::new("Segoe UI", 13.0);
        let mut base = WidgetBase::new();
        base.focus_policy = FocusPolicy::WheelFocus;
        base.size_policy = QSizePolicy::new(Policy::Minimum, Policy::Fixed);
        let mut spin = Self {
            base,
            value: T::DEFAULT_MINIMUM,
            minimum: T::DEFAULT_MINIMUM,
            maximum: T::DEFAULT_MAXIMUM,
            single_step: T::DEFAULT_STEP,
            decimals: T::DEFAULT_DECIMALS,
            display_integer_base: 10,
            wrapping: false,
            keyboard_tracking: true,
            read_only: false,
            prefix: String::new(),
            suffix: String::new(),
            special_value_text: String::new(),
            button_symbols: ButtonSymbols::UpDownArrows,
            edit_buffer: None,
            cursor: 0,
            all_selected: false,
            last_text: String::new(),
            wheel_remainder: 0,
            pressed_button: None,
            font,
            bg_color: Color::from_rgba8(255, 255, 255, 255),
            text_color: Color::from_rgba8(20, 20, 20, 255),
            border_color: Color::from_rgba8(160, 160, 160, 255),
            focus_border_color: Color::from_rgba8(0, 120, 215, 255),
            button_color: Color::from_rgba8(240, 240, 240, 255),
            arrow_color: Color::from_rgba8(70, 70, 70, 255),
            value_changed: Signal::new(),
            text_changed: Signal::new(),
            editing_finished: Signal::new(),
        };
        spin.last_text = spin.text();
        spin.cursor = spin.clean_text().chars().count();
        let hint = spin.size_hint();
        spin.base.geometry = Rect::new(0, 0, hint.width, hint.height);
        spin.base.dirty = Some(Rect::new(0, 0, hint.width, hint.height));
        spin
    }

    // ----- Range & value -------------------------------------------------------------------

    pub fn value(&self) -> T {
        self.value
    }

    /// Sets the value. Out-of-range values clamp (or, with wrapping, jump to the opposite bound).
    pub fn set_value(&mut self, value: T) {
        let bounded = self.bound(value.round_to(self.decimals), self.value, 0);
        self.edit_buffer = None;
        self.commit_value(bounded);
        self.refresh_text();
    }

    pub fn minimum(&self) -> T {
        self.minimum
    }

    pub fn maximum(&self) -> T {
        self.maximum
    }

    pub fn set_minimum(&mut self, minimum: T) {
        let max = if self.maximum < minimum {
            minimum
        } else {
            self.maximum
        };
        self.set_range(minimum, max);
    }

    pub fn set_maximum(&mut self, maximum: T) {
        let min = if self.minimum > maximum {
            maximum
        } else {
            self.minimum
        };
        self.set_range(min, maximum);
    }

    /// Sets the range (`maximum` raised to `minimum` if smaller) and re-bounds the value.
    pub fn set_range(&mut self, minimum: T, maximum: T) {
        let minimum = minimum.round_to(self.decimals);
        let maximum = maximum.round_to(self.decimals);
        self.minimum = minimum;
        self.maximum = if maximum < minimum { minimum } else { maximum };
        let v = self.value;
        let bounded = self.bound(v, v, 0);
        self.commit_value(bounded);
        self.refresh_text();
    }

    pub fn single_step(&self) -> T {
        self.single_step
    }

    /// Sets the step size; negative steps are ignored (as in Qt).
    pub fn set_single_step(&mut self, step: T) {
        if step.to_f64() >= 0.0 {
            self.single_step = step;
        }
    }

    pub fn wrapping(&self) -> bool {
        self.wrapping
    }

    pub fn set_wrapping(&mut self, wrapping: bool) {
        self.wrapping = wrapping;
        self.update();
    }

    pub fn keyboard_tracking(&self) -> bool {
        self.keyboard_tracking
    }

    /// When `true` (default), `value_changed` fires while typing whenever the text is acceptable.
    pub fn set_keyboard_tracking(&mut self, tracking: bool) {
        self.keyboard_tracking = tracking;
    }

    pub fn is_read_only(&self) -> bool {
        self.read_only
    }

    pub fn set_read_only(&mut self, read_only: bool) {
        self.read_only = read_only;
        self.update();
    }

    pub fn button_symbols(&self) -> ButtonSymbols {
        self.button_symbols
    }

    pub fn set_button_symbols(&mut self, symbols: ButtonSymbols) {
        self.button_symbols = symbols;
        self.update();
    }

    pub fn font(&self) -> &Font {
        &self.font
    }

    pub fn set_font(&mut self, font: Font) {
        self.font = font;
        self.update();
    }

    // ----- Text ------------------------------------------------------------------------------

    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    pub fn set_prefix(&mut self, prefix: impl Into<String>) {
        self.prefix = prefix.into();
        self.refresh_text();
    }

    pub fn suffix(&self) -> &str {
        &self.suffix
    }

    pub fn set_suffix(&mut self, suffix: impl Into<String>) {
        self.suffix = suffix.into();
        self.refresh_text();
    }

    pub fn special_value_text(&self) -> &str {
        &self.special_value_text
    }

    /// Text shown instead of the number while the value equals the minimum.
    pub fn set_special_value_text(&mut self, text: impl Into<String>) {
        self.special_value_text = text.into();
        self.refresh_text();
    }

    /// Renders `value` without prefix/suffix (`textFromValue`).
    pub fn text_from_value(&self, value: T) -> String {
        value.format_number(self.decimals, self.display_integer_base)
    }

    /// Parses text (with or without prefix/suffix, or the special value text) (`valueFromText`).
    pub fn value_from_text(&self, text: &str) -> Option<T> {
        if !self.special_value_text.is_empty() && text == self.special_value_text {
            return Some(self.minimum);
        }
        let clean = self.strip_affixes(text);
        T::parse_number(clean, self.decimals, self.display_integer_base)
            .map(|v| v.round_to(self.decimals))
    }

    /// Displayed text: special value text, or prefix + number (or edit buffer) + suffix.
    pub fn text(&self) -> String {
        if let Some(buffer) = &self.edit_buffer {
            return format!("{}{}{}", self.prefix, buffer, self.suffix);
        }
        if self.is_special_value() {
            return self.special_value_text.clone();
        }
        format!(
            "{}{}{}",
            self.prefix,
            self.text_from_value(self.value),
            self.suffix
        )
    }

    /// Text without prefix, suffix or surrounding whitespace (`cleanText`).
    pub fn clean_text(&self) -> String {
        match &self.edit_buffer {
            Some(buffer) => buffer.trim().to_string(),
            None => self.text_from_value(self.value),
        }
    }

    /// Classifies clean text the way `QSpinBox::validate` does.
    pub fn validate(&self, clean: &str) -> ValidatorState {
        let clean = clean.trim();
        if !T::is_partial_syntax(clean, self.decimals, self.display_integer_base) {
            return ValidatorState::Invalid;
        }
        if clean.starts_with('-') && self.minimum.to_f64() >= 0.0 {
            return ValidatorState::Invalid;
        }
        if clean.starts_with('+') && self.maximum.to_f64() < 0.0 {
            return ValidatorState::Invalid;
        }
        let Some(num) = T::parse_number(clean, self.decimals, self.display_integer_base) else {
            return ValidatorState::Intermediate;
        };
        let (n, min, max) = (num.to_f64(), self.minimum.to_f64(), self.maximum.to_f64());
        if n >= min && n <= max {
            ValidatorState::Acceptable
        } else if (n >= 0.0 && n > max) || (n < 0.0 && n < min) {
            // More digits only move further away from the range.
            ValidatorState::Invalid
        } else {
            ValidatorState::Intermediate
        }
    }

    /// Whether the text is currently being edited (not yet committed).
    pub fn is_editing(&self) -> bool {
        self.edit_buffer.is_some()
    }

    /// Cursor position (in characters) inside the clean text.
    pub fn cursor_position(&self) -> usize {
        self.cursor
    }

    /// Selects the whole number, so the next typed character replaces it.
    pub fn select_all(&mut self) {
        self.all_selected = true;
        self.cursor = self.clean_text().chars().count();
        self.update();
    }

    /// Inserts `text` at the cursor if the result stays valid; returns `true` if accepted.
    pub fn insert_text(&mut self, text: &str) -> bool {
        if self.read_only || text.is_empty() {
            return false;
        }
        let current = if self.all_selected {
            String::new()
        } else {
            self.current_clean()
        };
        let cursor = if self.all_selected {
            0
        } else {
            self.cursor.min(current.chars().count())
        };
        let byte = char_to_byte(&current, cursor);
        let mut candidate = String::with_capacity(current.len() + text.len());
        candidate.push_str(&current[..byte]);
        candidate.push_str(text);
        candidate.push_str(&current[byte..]);
        if self.validate(&candidate) == ValidatorState::Invalid {
            return false;
        }
        self.all_selected = false;
        self.cursor = cursor + text.chars().count();
        self.set_edit_buffer(candidate);
        true
    }

    /// Commits the edited text (`interpretText`): acceptable text sets the value, anything else
    /// reverts to the previous value.
    pub fn interpret_text(&mut self) {
        if let Some(buffer) = self.edit_buffer.take() {
            if let Some(v) = self.acceptable_value(&buffer) {
                self.commit_value(v);
            }
        }
        self.all_selected = false;
        self.refresh_text();
        self.cursor = self.clean_text().chars().count();
    }

    // ----- Stepping --------------------------------------------------------------------------

    /// Directions in which stepping is currently possible.
    pub fn step_enabled(&self) -> StepEnabled {
        if self.read_only {
            return StepEnabled {
                up: false,
                down: false,
            };
        }
        if self.wrapping {
            return StepEnabled {
                up: true,
                down: true,
            };
        }
        StepEnabled {
            up: self.value < self.maximum,
            down: self.value > self.minimum,
        }
    }

    /// Steps the value by `steps * single_step` (`stepBy`), honouring wrapping.
    pub fn step_by(&mut self, steps: i32) {
        let old = self.value;
        let mut commit_edit = true;
        if let Some(buffer) = self.edit_buffer.take() {
            match self.acceptable_value(&buffer) {
                Some(v) => self.commit_value(v),
                None => commit_edit = false,
            }
        }
        if commit_edit {
            let base = self.value;
            let candidate = base.to_f64() + self.single_step.to_f64() * f64::from(steps);
            let candidate = T::from_f64_saturating(candidate).round_to(self.decimals);
            let bounded = self.bound(candidate, old, steps);
            self.commit_value(bounded);
        }
        self.all_selected = false;
        self.refresh_text();
        self.cursor = self.clean_text().chars().count();
    }

    pub fn step_up(&mut self) {
        self.step_by(1);
    }

    pub fn step_down(&mut self) {
        self.step_by(-1);
    }

    /// `QAbstractSpinBoxPrivate::bound`: clamping, or wrapping when stepping past a bound.
    fn bound(&self, v: T, old: T, steps: i32) -> T {
        let (min, max) = (self.minimum, self.maximum);
        if !self.wrapping || steps == 0 {
            if v < min {
                return if self.wrapping { max } else { min };
            }
            if v > max {
                return if self.wrapping { min } else { max };
            }
            return v;
        }
        let was_min = old == min;
        let was_max = old == max;
        if v > max {
            if was_max && steps > 0 {
                min
            } else {
                max
            }
        } else if v < min {
            if !was_max && !was_min {
                min
            } else {
                max
            }
        } else {
            v
        }
    }

    // ----- Internals -------------------------------------------------------------------------

    fn is_special_value(&self) -> bool {
        !self.special_value_text.is_empty() && self.value == self.minimum
    }

    fn strip_affixes<'a>(&self, text: &'a str) -> &'a str {
        let mut s = text;
        if !self.prefix.is_empty() {
            s = s.strip_prefix(self.prefix.as_str()).unwrap_or(s);
        }
        if !self.suffix.is_empty() {
            s = s.strip_suffix(self.suffix.as_str()).unwrap_or(s);
        }
        s.trim()
    }

    fn current_clean(&self) -> String {
        match &self.edit_buffer {
            Some(buffer) => buffer.clone(),
            None if self.is_special_value() => String::new(),
            None => self.text_from_value(self.value),
        }
    }

    fn acceptable_value(&self, clean: &str) -> Option<T> {
        if self.validate(clean) != ValidatorState::Acceptable {
            return None;
        }
        T::parse_number(clean.trim(), self.decimals, self.display_integer_base)
            .map(|v| v.round_to(self.decimals))
    }

    fn set_edit_buffer(&mut self, buffer: String) {
        let tracked = if self.keyboard_tracking {
            self.acceptable_value(&buffer)
        } else {
            None
        };
        self.edit_buffer = Some(buffer);
        if let Some(v) = tracked {
            self.commit_value(v);
        }
        self.emit_text_if_changed();
        self.update();
    }

    fn commit_value(&mut self, value: T) {
        if value != self.value {
            self.value = value;
            self.value_changed.emit(&value);
            self.update();
        }
    }

    fn refresh_text(&mut self) {
        self.emit_text_if_changed();
        self.update();
    }

    fn emit_text_if_changed(&mut self) {
        let text = self.text();
        if text != self.last_text {
            self.last_text = text.clone();
            self.text_changed.emit(&text);
        }
    }

    fn edit_key(&mut self, key: u32, modifiers: u32) -> bool {
        if input_keys::is_left(key) {
            self.all_selected = false;
            self.cursor = self.cursor.saturating_sub(1);
            self.update();
            return true;
        }
        if input_keys::is_right(key) {
            let len = self.current_clean().chars().count();
            self.all_selected = false;
            self.cursor = (self.cursor + 1).min(len);
            self.update();
            return true;
        }
        if input_keys::is_home(key) {
            self.all_selected = false;
            self.cursor = 0;
            self.update();
            return true;
        }
        if input_keys::is_end(key) {
            self.all_selected = false;
            self.cursor = self.current_clean().chars().count();
            self.update();
            return true;
        }
        if input_keys::is_backspace(key) || input_keys::is_delete(key) {
            if self.read_only {
                return true;
            }
            let current = self.current_clean();
            let len = current.chars().count();
            let cursor = self.cursor.min(len);
            let (candidate, new_cursor) = if self.all_selected {
                (String::new(), 0)
            } else if input_keys::is_backspace(key) && cursor > 0 {
                (remove_char(&current, cursor - 1), cursor - 1)
            } else if input_keys::is_delete(key) && cursor < len {
                (remove_char(&current, cursor), cursor)
            } else {
                return true;
            };
            if self.validate(&candidate) != ValidatorState::Invalid {
                self.all_selected = false;
                self.cursor = new_cursor;
                self.set_edit_buffer(candidate);
            }
            return true;
        }
        if input_keys::has_ctrl(modifiers) && (key == 0x41 || key == 0x61) {
            self.select_all();
            return true;
        }
        if let Some(c) = input_keys::typed_char(key, modifiers) {
            let mut buf = [0u8; 4];
            self.insert_text(c.encode_utf8(&mut buf));
            return true;
        }
        false
    }

    fn button_rects(&self) -> Option<(Rect, Rect)> {
        if self.button_symbols == ButtonSymbols::NoButtons {
            return None;
        }
        let g = self.base.geometry;
        let x = (g.width - BUTTON_WIDTH).max(0);
        let half = g.height / 2;
        Some((
            Rect::new(x, 0, BUTTON_WIDTH, half),
            Rect::new(x, half, BUTTON_WIDTH, g.height - half),
        ))
    }
}

fn char_to_byte(s: &str, char_index: usize) -> usize {
    s.char_indices()
        .nth(char_index)
        .map(|(b, _)| b)
        .unwrap_or(s.len())
}

fn remove_char(s: &str, char_index: usize) -> String {
    s.chars()
        .enumerate()
        .filter(|(i, _)| *i != char_index)
        .map(|(_, c)| c)
        .collect()
}

impl<T: SpinNumber> Drop for NumericSpinBox<T> {
    fn drop(&mut self) {
        // Registry cleanup only touches thread-locals through `try_with`.
        // SAFETY: Drop runs on the registration thread; no callbacks are active at this point.
        unsafe { qtrs_core::object::unregister_qobject(self.base.object_data.id) };
    }
}

impl<T: SpinNumber> QObject for NumericSpinBox<T> {
    leaf_qobject_common!();

    fn event(&mut self, event: &mut Event) -> bool {
        if let EventKind::InputMethod { commit_string, .. } = &event.kind {
            if !commit_string.is_empty() {
                let commit = commit_string.clone();
                self.insert_text(&commit);
            }
            return true;
        }
        input_common::dispatch_input_event(self, event)
    }
}

impl<T: SpinNumber> Widget for NumericSpinBox<T> {
    leaf_widget_common!();

    fn size_hint(&self) -> Size {
        let metrics = FontMetrics::from_font(&self.font);
        let widest = |v: T| {
            let s = format!("{}{}{}", self.prefix, self.text_from_value(v), self.suffix);
            metrics.horizontal_advance(&s, &self.font)
        };
        let mut w = widest(self.minimum).max(widest(self.maximum));
        if !self.special_value_text.is_empty() {
            w = w.max(metrics.horizontal_advance(&self.special_value_text, &self.font));
        }
        let buttons = if self.button_symbols == ButtonSymbols::NoButtons {
            0
        } else {
            BUTTON_WIDTH
        };
        let h = (metrics.height.ceil() as i32 + 10).max(24);
        Size::new(w.ceil() as i32 + 2 * TEXT_PAD as i32 + buttons + 4, h)
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.base.enabled = enabled;
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

    fn focus_in_event(&mut self, reason: FocusReason) {
        if matches!(
            reason,
            FocusReason::Tab | FocusReason::Backtab | FocusReason::Shortcut
        ) {
            self.select_all();
        }
        self.update();
    }

    fn focus_out_event(&mut self, _reason: FocusReason) {
        self.interpret_text();
        self.editing_finished.emit(&());
    }

    fn mouse_press_event(&mut self, pos: Point, button: u32, _modifiers: u32) {
        if !self.base.enabled || button != 1 {
            return;
        }
        if let Some((up, down)) = self.button_rects() {
            let enabled = self.step_enabled();
            if up.contains(pos) {
                self.pressed_button = Some(true);
                if enabled.up {
                    self.step_by(1);
                }
                return;
            }
            if down.contains(pos) {
                self.pressed_button = Some(false);
                if enabled.down {
                    self.step_by(-1);
                }
                return;
            }
        }
        // Click in the text area: place the cursor by measuring the clean text.
        let metrics = FontMetrics::from_font(&self.font);
        let prefix_w = metrics.horizontal_advance(&self.prefix, &self.font);
        let x = pos.x as f32 - TEXT_PAD - prefix_w;
        let clean = self.current_clean();
        let mut acc = 0.0f32;
        let mut index = 0;
        for c in clean.chars() {
            let mut buf = [0u8; 4];
            let cw = metrics.horizontal_advance(c.encode_utf8(&mut buf), &self.font);
            if x < acc + cw / 2.0 {
                break;
            }
            acc += cw;
            index += 1;
        }
        self.all_selected = false;
        self.cursor = index;
        self.update();
    }

    fn mouse_release_event(&mut self, _pos: Point, button: u32, _modifiers: u32) {
        if button == 1 && self.pressed_button.take().is_some() {
            self.update();
        }
    }

    fn wheel_event(&mut self, _pos: Point, delta_y: i32, modifiers: u32) {
        if !self.base.enabled {
            return;
        }
        self.wheel_remainder += delta_y;
        let steps = self.wheel_remainder / 120;
        self.wheel_remainder -= steps * 120;
        if steps == 0 {
            return;
        }
        let enabled = self.step_enabled();
        if (steps > 0 && enabled.up) || (steps < 0 && enabled.down) {
            let factor = if input_keys::has_ctrl(modifiers) {
                10
            } else {
                1
            };
            self.step_by(steps * factor);
        }
    }

    fn key_press_event(&mut self, key: u32, modifiers: u32, _is_repeat: bool) {
        if !self.base.enabled {
            return;
        }
        let page = input_keys::is_page_up(key) || input_keys::is_page_down(key);
        if input_keys::is_up(key) || input_keys::is_down(key) || page {
            let up = input_keys::is_up(key) || input_keys::is_page_up(key);
            let enabled = self.step_enabled();
            if (up && !enabled.up) || (!up && !enabled.down) {
                return;
            }
            let mut steps = if up { 1 } else { -1 };
            if page || input_keys::has_ctrl(modifiers) {
                steps *= 10;
            }
            self.step_by(steps);
            return;
        }
        if input_keys::is_enter(key) {
            self.interpret_text();
            self.editing_finished.emit(&());
            return;
        }
        self.edit_key(key, modifiers);
    }

    fn paint_event(&mut self, painter: &mut Painter) {
        let g = self.base.geometry;
        let (w, h) = (g.width as f32, g.height as f32);
        let frame = RectF::new(0.5, 0.5, (w - 1.0).max(0.0), (h - 1.0).max(0.0));

        // 1. Frame
        painter.set_brush(Brush::Color(self.bg_color));
        let focused = self.base.has_focus;
        painter.set_pen(Pen::new(
            if focused {
                self.focus_border_color
            } else {
                self.border_color
            },
            if focused { 1.5 } else { 1.0 },
        ));
        painter.draw_rounded_rect(frame, 3.0, 3.0);

        // 2. Text (with whole-text selection highlight)
        let metrics = FontMetrics::from_font(&self.font);
        let baseline = ((h - metrics.height) / 2.0).max(0.0) + metrics.ascent;
        let text = self.text();
        if self.all_selected && focused {
            let prefix_w = metrics.horizontal_advance(&self.prefix, &self.font);
            let clean = self.current_clean();
            let sel_w = metrics.horizontal_advance(&clean, &self.font);
            painter.set_pen(None);
            painter.set_brush(Brush::Color(Color::from_rgba8(0, 120, 215, 90)));
            painter.draw_rect(RectF::new(
                TEXT_PAD + prefix_w,
                (h - metrics.height) / 2.0,
                sel_w,
                metrics.height,
            ));
        }
        let text_color = if self.base.enabled {
            self.text_color
        } else {
            self.border_color
        };
        painter.set_pen(Pen::new(text_color, 1.0));
        painter.draw_text(PointF::new(TEXT_PAD, baseline), &text, &self.font);

        // 3. Cursor
        if focused && !self.read_only && !(self.edit_buffer.is_none() && self.is_special_value()) {
            let clean = self.current_clean();
            let upto: String = clean.chars().take(self.cursor).collect();
            let x = TEXT_PAD
                + metrics.horizontal_advance(&self.prefix, &self.font)
                + metrics.horizontal_advance(&upto, &self.font);
            let cursor_h = metrics.height.min(h - 6.0).max(0.0);
            let y = (h - cursor_h) / 2.0;
            painter.draw_line(PointF::new(x, y), PointF::new(x, y + cursor_h));
        }

        // 4. Step buttons
        if let Some((up, down)) = self.button_rects() {
            let enabled = self.step_enabled();
            for (rect, is_up) in [(up, true), (down, false)] {
                let pressed = self.pressed_button == Some(is_up);
                let fill = if pressed {
                    self.border_color
                } else {
                    self.button_color
                };
                let r = RectF::new(
                    rect.x as f32,
                    rect.y as f32 + 0.5,
                    rect.width as f32 - 0.5,
                    rect.height as f32 - 1.0,
                );
                painter.set_pen(Pen::new(self.border_color, 1.0));
                painter.set_brush(Brush::Color(fill));
                painter.draw_rect(r);
                let active = self.base.enabled && if is_up { enabled.up } else { enabled.down };
                let color = if active {
                    self.arrow_color
                } else {
                    self.border_color
                };
                let cx = r.x + r.width / 2.0;
                let cy = r.y + r.height / 2.0;
                painter.set_pen(Pen::new(color, 1.5));
                match self.button_symbols {
                    ButtonSymbols::PlusMinus => {
                        painter.draw_line(PointF::new(cx - 3.0, cy), PointF::new(cx + 3.0, cy));
                        if is_up {
                            painter.draw_line(PointF::new(cx, cy - 3.0), PointF::new(cx, cy + 3.0));
                        }
                    }
                    _ => {
                        let dy = if is_up { -1.5 } else { 1.5 };
                        painter.draw_polyline(&[
                            PointF::new(cx - 3.5, cy - dy),
                            PointF::new(cx, cy + dy),
                            PointF::new(cx + 3.5, cy - dy),
                        ]);
                    }
                }
            }
        }
    }
}
