//! Seven-segment LCD number display (`QLCDNumber`).

use qtrs_core::event::{Event, EventKind};
use qtrs_core::object::{ObjectData, ObjectId, QObject};
use qtrs_core::signal::Signal;
use qtrs_gui::geometry::primitives::{PointF, Rect, Size};
use qtrs_gui::paint::palette::{ColorGroup, ColorRole, Palette};
use qtrs_gui::paint::{Brush, Painter, Pen};
use qtrs_gui::tiny_skia::{Color, PathBuilder};

use crate::frame::{FrameShadow, FrameShape, FrameStyle};
use crate::layout::Layout;
use crate::size_policy::{Policy, QSizePolicy};
use crate::widget::{Widget, WidgetBase, WidgetRef, WidgetWeak};

/// Number base used to display integers (`QLCDNumber::Mode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum LcdMode {
    Hex,
    #[default]
    Dec,
    Oct,
    Bin,
}

/// Segment rendering style (`QLCDNumber::SegmentStyle`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum SegmentStyle {
    /// Raised segments filled with the background color.
    Outline,
    /// Raised segments filled with the foreground color.
    #[default]
    Filled,
    /// Flat segments filled with the foreground color.
    Flat,
}

/// Maximum number of digits (`QLCDNumber::setDigitCount` limit).
pub const MAX_LCD_DIGITS: usize = 99;

/// Seven-segment number display (`QLCDNumber`).
pub struct LCDNumber {
    pub base: WidgetBase,
    frame: FrameStyle,
    palette: Palette,
    ndigits: usize,
    /// Exactly `ndigits` display characters.
    digits: Vec<char>,
    /// Per-digit decimal point flags used in small-decimal-point mode.
    points: Vec<bool>,
    value: f64,
    mode: LcdMode,
    small_point: bool,
    fill: bool,
    shadow: bool,

    /// Emitted when asked to display a number that does not fit (`QLCDNumber::overflow`).
    pub overflow: Signal<()>,
}

/// Canonical Qt alias.
pub type QLCDNumber = LCDNumber;

impl Default for LCDNumber {
    fn default() -> Self {
        Self::new()
    }
}

impl LCDNumber {
    /// Creates a 5-digit decimal display showing `0`, framed as a raised box.
    pub fn new() -> Self {
        Self::with_digit_count(5)
    }

    /// Creates a display with `digits` digit positions (clamped to 0..=99).
    pub fn with_digit_count(digits: usize) -> Self {
        let mut base = WidgetBase::new();
        base.size_policy = QSizePolicy::new(Policy::Minimum, Policy::Minimum);
        let ndigits = digits.min(MAX_LCD_DIGITS);
        let mut digit_chars = vec![' '; ndigits];
        if let Some(last) = digit_chars.last_mut() {
            *last = '0';
        }
        let mut lcd = Self {
            base,
            frame: FrameStyle::new(FrameShape::Box, FrameShadow::Raised),
            palette: Palette::light(),
            ndigits,
            digits: digit_chars,
            points: vec![false; ndigits],
            value: 0.0,
            mode: LcdMode::Dec,
            small_point: false,
            fill: true,
            shadow: true,
            overflow: Signal::new(),
        };
        let hint = lcd.size_hint();
        lcd.base.geometry = Rect::new(0, 0, hint.width, hint.height);
        lcd
    }

    /// Number of digit positions (`QLCDNumber::digitCount`).
    pub fn digit_count(&self) -> usize {
        self.ndigits
    }

    /// Changes the number of digit positions, keeping the right-aligned content
    /// (`QLCDNumber::setDigitCount`). Values above 99 clamp to 99.
    pub fn set_digit_count(&mut self, digits: usize) {
        let digits = digits.min(MAX_LCD_DIGITS);
        if digits == self.ndigits {
            return;
        }
        let redisplay = self.ndigits == 0;
        if digits > self.ndigits {
            let grow = digits - self.ndigits;
            self.digits.splice(0..0, std::iter::repeat_n(' ', grow));
            self.points.splice(0..0, std::iter::repeat_n(false, grow));
        } else {
            let shrink = self.ndigits - digits;
            self.digits.drain(0..shrink);
            self.points.drain(0..shrink);
        }
        self.ndigits = digits;
        if redisplay {
            self.display_f64(self.value);
        }
        self.update();
    }

    pub fn mode(&self) -> LcdMode {
        self.mode
    }

    /// Changes the number base and redisplays the current value (`QLCDNumber::setMode`).
    pub fn set_mode(&mut self, mode: LcdMode) {
        self.mode = mode;
        self.display_f64(self.value);
    }

    pub fn set_hex_mode(&mut self) {
        self.set_mode(LcdMode::Hex);
    }

    pub fn set_dec_mode(&mut self) {
        self.set_mode(LcdMode::Dec);
    }

    pub fn set_oct_mode(&mut self) {
        self.set_mode(LcdMode::Oct);
    }

    pub fn set_bin_mode(&mut self) {
        self.set_mode(LcdMode::Bin);
    }

    /// Whether decimal points occupy their own digit position (`false`) or sit
    /// between digits (`true`) (`QLCDNumber::smallDecimalPoint`).
    pub fn small_decimal_point(&self) -> bool {
        self.small_point
    }

    pub fn set_small_decimal_point(&mut self, small: bool) {
        self.small_point = small;
        self.update();
    }

    pub fn segment_style(&self) -> SegmentStyle {
        match (self.fill, self.shadow) {
            (false, _) => SegmentStyle::Outline,
            (true, true) => SegmentStyle::Filled,
            (true, false) => SegmentStyle::Flat,
        }
    }

    pub fn set_segment_style(&mut self, style: SegmentStyle) {
        self.fill = matches!(style, SegmentStyle::Flat | SegmentStyle::Filled);
        self.shadow = matches!(style, SegmentStyle::Outline | SegmentStyle::Filled);
        self.update();
    }

    /// The displayed value; 0 when the displayed string is not a number (`QLCDNumber::value`).
    pub fn value(&self) -> f64 {
        self.value
    }

    /// The displayed value rounded to the nearest integer (`QLCDNumber::intValue`).
    pub fn int_value(&self) -> i32 {
        let rounded = self.value.round();
        if rounded.is_nan() {
            0
        } else {
            rounded.clamp(i32::MIN as f64, i32::MAX as f64) as i32
        }
    }

    /// Currently shown characters, one per digit position (without small decimal points).
    pub fn digit_string(&self) -> String {
        self.digits.iter().collect()
    }

    /// Per-position decimal point flags (only set in small-decimal-point mode).
    pub fn decimal_points(&self) -> &[bool] {
        &self.points
    }

    /// Displays an integer in the current base; emits `overflow` and keeps the old
    /// display if it does not fit (`QLCDNumber::display(int)`).
    pub fn display_i32(&mut self, num: i32) {
        self.value = num as f64;
        match int_to_lcd_string(num, self.mode, self.ndigits) {
            Some(text) => self.set_display_string(&text),
            None => self.overflow.emit(&()),
        }
    }

    /// Displays a floating-point number (`QLCDNumber::display(double)`); non-decimal
    /// bases show the truncated integer.
    pub fn display_f64(&mut self, num: f64) {
        self.value = num;
        match double_to_lcd_string(num, self.mode, self.ndigits) {
            Some(text) => self.set_display_string(&text),
            None => self.overflow.emit(&()),
        }
    }

    /// Displays arbitrary text using the characters a seven-segment display can show
    /// (`QLCDNumber::display(QString)`); unknown characters render blank.
    pub fn display_str(&mut self, text: &str) {
        self.value = text.trim().parse::<f64>().unwrap_or(0.0);
        self.set_display_string(text);
    }

    /// Returns `true` if `num` would overflow in the current base and digit count.
    pub fn check_overflow_i32(&self, num: i32) -> bool {
        int_to_lcd_string(num, self.mode, self.ndigits).is_none()
    }

    /// Returns `true` if `num` would overflow in the current base and digit count.
    pub fn check_overflow_f64(&self, num: f64) -> bool {
        double_to_lcd_string(num, self.mode, self.ndigits).is_none()
    }

    pub fn frame_style(&self) -> FrameStyle {
        self.frame
    }

    pub fn set_frame_style(&mut self, style: FrameStyle) {
        self.frame = style;
        self.update();
    }

    pub fn palette(&self) -> &Palette {
        &self.palette
    }

    pub fn set_palette(&mut self, palette: Palette) {
        self.palette = palette;
        self.update();
    }

    /// Lays `text` out into the digit buffer (`QLCDNumberPrivate::internalSetString`).
    fn set_display_string(&mut self, text: &str) {
        let n = self.ndigits;
        let chars: Vec<char> = text.chars().collect();
        let mut buffer = vec![' '; n];
        let mut new_points = vec![false; n];
        if !self.small_point {
            let take = chars.len().min(n);
            buffer[n - take..].copy_from_slice(&chars[chars.len() - take..]);
        } else if n > 0 {
            // Points attach to the preceding digit; two points in a row insert a blank digit.
            let mut index: isize = -1;
            let mut last_was_point = true;
            for &ch in &chars {
                if ch == '.' {
                    if last_was_point {
                        if index == n as isize - 1 {
                            break;
                        }
                        index += 1;
                        buffer[index as usize] = ' ';
                    }
                    new_points[index as usize] = true;
                    last_was_point = true;
                } else {
                    if index == n as isize - 1 {
                        break;
                    }
                    index += 1;
                    buffer[index as usize] = ch;
                    new_points[index as usize] = false;
                    last_was_point = false;
                }
            }
            // Right-align the used positions.
            let used = (index + 1) as usize;
            if used < n {
                let shift = n - used;
                for i in (0..used).rev() {
                    buffer[i + shift] = buffer[i];
                    new_points[i + shift] = new_points[i];
                }
                for i in 0..shift {
                    buffer[i] = ' ';
                    new_points[i] = false;
                }
            }
        }
        if buffer == self.digits && (!self.small_point || new_points == self.points) {
            return;
        }
        self.digits = buffer;
        if self.small_point {
            self.points = new_points;
        }
        self.update();
    }

    /// Draws one character's segments at `pos` (`QLCDNumberPrivate::drawDigit`).
    fn draw_digit(
        &self,
        painter: &mut Painter,
        pos: (i32, i32),
        seg_len: i32,
        ch: char,
        colors: &SegmentColors,
    ) {
        for &segment in segments_for(ch) {
            self.draw_segment(painter, pos, segment, seg_len, colors);
        }
    }

    /// Draws a single segment polygon (`QLCDNumberPrivate::drawSegment`).
    fn draw_segment(
        &self,
        painter: &mut Painter,
        pos: (i32, i32),
        segment: u8,
        seg_len: i32,
        colors: &SegmentColors,
    ) {
        let (start, edges) = segment_outline(segment, pos, seg_len, self.small_point);
        if self.fill {
            let mut pb = PathBuilder::new();
            pb.move_to(start.0 as f32, start.1 as f32);
            for &(x, y, _) in &edges {
                pb.line_to(x as f32, y as f32);
            }
            pb.close();
            if let Some(path) = pb.finish() {
                painter.set_pen(None);
                painter.set_brush(Brush::Color(colors.foreground));
                painter.fill_path(&path);
            }
        }
        if self.shadow {
            let mut prev = start;
            for &(x, y, shade) in &edges {
                let color = match shade {
                    Shade::Light => colors.light,
                    Shade::Dark => colors.dark,
                };
                painter.set_pen(Pen::new(color, 1.0));
                painter.draw_line(
                    PointF::new(prev.0 as f32 + 0.5, prev.1 as f32 + 0.5),
                    PointF::new(x as f32 + 0.5, y as f32 + 0.5),
                );
                prev = (x, y);
            }
        }
    }
}

struct SegmentColors {
    light: Color,
    dark: Color,
    foreground: Color,
}

#[derive(Clone, Copy)]
enum Shade {
    Light,
    Dark,
}

/// Segment indices lit for a character (`getSegments` in qlcdnumber.cpp).
///
/// Segments: 0 top, 1 upper-left, 2 upper-right, 3 middle, 4 lower-left,
/// 5 lower-right, 6 bottom, 7 decimal point, 8/9 colon dots.
pub fn segments_for(ch: char) -> &'static [u8] {
    const DIGITS: [&[u8]; 10] = [
        &[0, 1, 2, 4, 5, 6],
        &[2, 5],
        &[0, 2, 3, 4, 6],
        &[0, 2, 3, 5, 6],
        &[1, 2, 3, 5],
        &[0, 1, 3, 5, 6],
        &[0, 1, 3, 4, 5, 6],
        &[0, 2, 5],
        &[0, 1, 2, 3, 4, 5, 6],
        &[0, 1, 2, 3, 5, 6],
    ];
    const HEX: [&[u8]; 6] = [
        &[0, 1, 2, 3, 4, 5],
        &[1, 3, 4, 5, 6],
        &[0, 1, 4, 6],
        &[2, 3, 4, 5, 6],
        &[0, 1, 3, 4, 6],
        &[0, 1, 3, 4],
    ];
    match ch {
        '0'..='9' => DIGITS[ch as usize - '0' as usize],
        'A'..='F' => HEX[ch as usize - 'A' as usize],
        'a'..='f' => HEX[ch as usize - 'a' as usize],
        '-' => &[3],
        'O' => DIGITS[0],
        'g' => DIGITS[9],
        '.' => &[7],
        'h' => &[1, 3, 4, 5],
        'H' => &[1, 2, 3, 4, 5],
        'l' | 'L' => &[1, 4, 6],
        'o' => &[3, 4, 5, 6],
        'p' | 'P' => &[0, 1, 2, 3, 4],
        'r' | 'R' => &[3, 4],
        's' | 'S' => DIGITS[5],
        'u' => &[4, 5, 6],
        'U' => &[1, 2, 4, 5, 6],
        'y' | 'Y' => &[1, 2, 3, 5, 6],
        ':' => &[8, 9],
        '\'' => &[0, 1, 2, 3],
        _ => &[],
    }
}

/// Polygon for a segment: start point plus each vertex with the shade of the edge
/// leading to it (exact port of `QLCDNumberPrivate::drawSegment` geometry).
fn segment_outline(
    segment: u8,
    pos: (i32, i32),
    seg_len: i32,
    small_point: bool,
) -> ((i32, i32), Vec<(i32, i32, Shade)>) {
    use Shade::{Dark as D, Light as L};
    let width = seg_len / 5;
    let (dx, dy, rel): (i32, i32, Vec<(i32, i32, Shade)>) = match segment {
        0 => (
            0,
            0,
            vec![
                (seg_len - 1, 0, L),
                (seg_len - width - 1, width, D),
                (width, width, D),
                (0, 0, D),
            ],
        ),
        1 => (
            0,
            1,
            vec![
                (width, width, L),
                (width, seg_len - width / 2 - 2, D),
                (0, seg_len - 2, D),
                (0, 0, L),
            ],
        ),
        2 => (
            seg_len - 1,
            1,
            vec![
                (0, seg_len - 2, D),
                (-width, seg_len - width / 2 - 2, D),
                (-width, width, L),
                (0, 0, L),
            ],
        ),
        3 => {
            let mut v = vec![
                (width, -width / 2, L),
                (seg_len - width - 1, -width / 2, L),
                (seg_len - 1, 0, L),
            ];
            if width & 1 == 1 {
                v.push((seg_len - width - 3, width / 2 + 1, D));
                v.push((width + 2, width / 2 + 1, D));
            } else {
                v.push((seg_len - width - 1, width / 2, D));
                v.push((width, width / 2, D));
            }
            v.push((0, 0, D));
            (0, seg_len, v)
        }
        4 => (
            0,
            seg_len + 1,
            vec![
                (width, width / 2, L),
                (width, seg_len - width - 2, D),
                (0, seg_len - 2, D),
                (0, 0, L),
            ],
        ),
        5 => (
            seg_len - 1,
            seg_len + 1,
            vec![
                (0, seg_len - 2, D),
                (-width, seg_len - width - 2, D),
                (-width, width / 2, L),
                (0, 0, L),
            ],
        ),
        6 => (
            0,
            seg_len * 2,
            vec![
                (width, -width, L),
                (seg_len - width - 1, -width, L),
                (seg_len - 1, 0, L),
                (0, 0, D),
            ],
        ),
        7 => {
            let (px, py) = if small_point {
                (seg_len + width / 2, seg_len * 2)
            } else {
                (seg_len / 2, seg_len * 2)
            };
            (
                px,
                py,
                vec![(width, 0, D), (width, -width, D), (0, -width, L), (0, 0, L)],
            )
        }
        8 => (
            seg_len / 2 - width / 2 + 1,
            seg_len / 2 + width,
            vec![(width, 0, D), (width, -width, D), (0, -width, L), (0, 0, L)],
        ),
        _ => (
            seg_len / 2 - width / 2 + 1,
            3 * seg_len / 2 + width,
            vec![(width, 0, D), (width, -width, D), (0, -width, L), (0, 0, L)],
        ),
    };
    let origin = (pos.0 + dx, pos.1 + dy);
    let edges = rel
        .into_iter()
        .map(|(x, y, s)| (origin.0 + x, origin.1 + y, s))
        .collect();
    (origin, edges)
}

/// Formats an integer right-aligned in `ndigits` positions (`int2string`);
/// `None` on overflow.
pub fn int_to_lcd_string(number: i32, mode: LcdMode, ndigits: usize) -> Option<String> {
    let magnitude = number.unsigned_abs();
    let digits = match mode {
        LcdMode::Hex => format!("{:x}", magnitude),
        LcdMode::Dec => format!("{}", magnitude),
        LcdMode::Oct => format!("{:o}", magnitude),
        LcdMode::Bin => format!("{:b}", magnitude),
    };
    let mut text = format!("{:>width$}", digits, width = ndigits);
    if number < 0 {
        // The sign replaces the space before the first digit, or is prepended.
        let first = text.find(|c: char| c != ' ').unwrap_or(0);
        if first > 0 {
            text.replace_range(first - 1..first, "-");
        } else {
            text.insert(0, '-');
        }
    }
    (text.chars().count() <= ndigits).then_some(text)
}

/// Formats a double for an `ndigits` display (`double2string`); decimal mode uses
/// C `%g` with decreasing precision until it fits. `None` on overflow.
pub fn double_to_lcd_string(num: f64, mode: LcdMode, ndigits: usize) -> Option<String> {
    if mode != LcdMode::Dec {
        if !(-2_147_483_648.0..2_147_483_648.0).contains(&num) {
            return None;
        }
        return int_to_lcd_string(num as i32, mode, ndigits);
    }
    let mut precision = ndigits;
    loop {
        let mut text = format!(
            "{:>width$}",
            format_c_general(num, precision),
            width = ndigits
        );
        // "1.5e+07" is shown as "1.5 e07" to save the sign position.
        if let Some(e) = text.find('e') {
            if e > 0 && text[e + 1..].starts_with('+') {
                text.replace_range(e..e + 2, " e");
            }
        }
        if text.chars().count() <= ndigits {
            return Some(text);
        }
        if precision == 0 {
            return None;
        }
        precision -= 1;
    }
}

/// C `printf("%.*g", precision, num)` formatting.
pub fn format_c_general(num: f64, precision: usize) -> String {
    if num.is_nan() {
        return if num.is_sign_negative() {
            "-nan".into()
        } else {
            "nan".into()
        };
    }
    if num.is_infinite() {
        return if num < 0.0 {
            "-inf".into()
        } else {
            "inf".into()
        };
    }
    let p = precision.max(1);
    let sci = format!("{:.*e}", p - 1, num);
    let (mantissa, exp_text) = sci.split_once('e').unwrap_or((sci.as_str(), "0"));
    let exponent: i32 = exp_text.parse().unwrap_or(0);
    if exponent < -4 || exponent >= p as i32 {
        let mantissa = strip_fraction_zeros(mantissa);
        let sign = if exponent < 0 { '-' } else { '+' };
        format!("{}e{}{:02}", mantissa, sign, exponent.unsigned_abs())
    } else {
        let decimals = (p as i32 - 1 - exponent).max(0) as usize;
        strip_fraction_zeros(&format!("{:.*}", decimals, num)).to_string()
    }
}

fn strip_fraction_zeros(text: &str) -> &str {
    if text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.')
    } else {
        text
    }
}

impl QObject for LCDNumber {
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

impl Widget for LCDNumber {
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
            self.update();
        }
    }

    /// `QLCDNumber::sizeHint`: 9px per digit (plus one for a full-width point) and 23px high.
    fn size_hint(&self) -> Size {
        let extra = if self.small_point { 0 } else { 1 };
        Size::new(10 + 9 * (self.ndigits as i32 + extra), 23)
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

    fn set_enabled(&mut self, enabled: bool) {
        if self.base.enabled != enabled {
            self.base.enabled = enabled;
            self.update();
        }
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
        let w = self.base.geometry.width;
        let h = self.base.geometry.height;
        self.frame.paint(
            painter,
            Rect::new(0, 0, w, h),
            &self.palette,
            self.base.enabled,
        );
        if self.ndigits == 0 {
            return;
        }
        let group = if self.base.enabled {
            ColorGroup::Active
        } else {
            ColorGroup::Disabled
        };
        let colors = SegmentColors {
            light: self.palette.color(group, ColorRole::Light),
            dark: self.palette.color(group, ColorRole::Dark),
            foreground: if self.fill {
                self.palette.color(group, ColorRole::WindowText)
            } else {
                self.palette.color(group, ColorRole::Window)
            },
        };

        // Layout from QLCDNumberPrivate::drawString.
        let n = self.ndigits as i32;
        let digit_space = if self.small_point { 2 } else { 1 };
        let x_seg_len = w * 5 / (n * (5 + digit_space) + digit_space);
        let y_seg_len = h * 5 / 12;
        let seg_len = x_seg_len.min(y_seg_len);
        if seg_len < 5 {
            return;
        }
        let x_advance = seg_len * (5 + digit_space) / 5;
        let x_offset = (w - n * x_advance + seg_len / 5) / 2;
        let y_offset = (h - seg_len * 2) / 2;

        for i in 0..self.ndigits {
            let pos = (x_offset + x_advance * i as i32, y_offset);
            self.draw_digit(painter, pos, seg_len, self.digits[i], &colors);
            if self.small_point && self.points[i] {
                self.draw_digit(painter, pos, seg_len, '.', &colors);
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
