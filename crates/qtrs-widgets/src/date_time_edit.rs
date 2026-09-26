//! Date and time editing controls (`QDateTimeEdit`, `QDateEdit`, `QTimeEdit`).

use qtrs_core::event::Event;
use qtrs_core::object::{ObjectId, QObject};
use qtrs_core::signal::Signal;
use qtrs_core::types::{Date, DateTime, Time};
use qtrs_gui::geometry::primitives::{Point, PointF, Rect, RectF, Size};
use qtrs_gui::paint::{Brush, Painter, Pen};
use qtrs_gui::text::{Font, FontMetrics};
use qtrs_gui::tiny_skia::Color;

use crate::focus::FocusPolicy;
use crate::input_common;
use crate::input_keys;
use crate::layout::Layout;
use crate::size_policy::{Policy, QSizePolicy};
use crate::widget::{Widget, WidgetBase, WidgetRef, WidgetWeak};

/// Editing section within a temporal widget (`QDateTimeEdit::Section`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum DateTimeSection {
    #[default]
    NoSection,
    YearSection,
    MonthSection,
    DaySection,
    HourSection,
    MinuteSection,
    SecondSection,
}

pub struct DateTimeEdit {
    pub base: WidgetBase,
    date_time: DateTime,
    min_date_time: DateTime,
    max_date_time: DateTime,
    current_section: usize,
    sections: Vec<DateTimeSection>,
    format_str: String,
    pub date_time_changed: Signal<DateTime>,
    pub date_changed: Signal<Date>,
    pub time_changed: Signal<Time>,
    font: Font,
    up_pressed: bool,
    down_pressed: bool,
}

pub type QDateTimeEdit = DateTimeEdit;

const BUTTON_WIDTH: i32 = 18;

impl DateTimeEdit {
    pub fn new() -> Self {
        Self::with_datetime(DateTime::new(
            Date::new(2026, 9, 26),
            Time::new(12, 0, 0, 0),
        ))
    }

    pub fn with_datetime(dt: DateTime) -> Self {
        let mut base = WidgetBase::new();
        base.focus_policy = FocusPolicy::StrongFocus;
        base.size_policy = QSizePolicy::new(Policy::Preferred, Policy::Fixed);
        base.geometry = Rect::new(0, 0, 160, 26);

        let sections = vec![
            DateTimeSection::YearSection,
            DateTimeSection::MonthSection,
            DateTimeSection::DaySection,
            DateTimeSection::HourSection,
            DateTimeSection::MinuteSection,
            DateTimeSection::SecondSection,
        ];

        Self {
            base,
            date_time: dt,
            min_date_time: DateTime::new(Date::new(1900, 1, 1), Time::new(0, 0, 0, 0)),
            max_date_time: DateTime::new(Date::new(2100, 12, 31), Time::new(23, 59, 59, 999)),
            current_section: 0,
            sections,
            format_str: "yyyy-MM-dd HH:mm:ss".to_string(),
            date_time_changed: Signal::new(),
            date_changed: Signal::new(),
            time_changed: Signal::new(),
            font: Font::new("Segoe UI", 12.0),
            up_pressed: false,
            down_pressed: false,
        }
    }

    pub fn date_time(&self) -> DateTime {
        self.date_time
    }

    pub fn set_date_time(&mut self, dt: DateTime) {
        let clamped = self.clamp_datetime(dt);
        if self.date_time != clamped {
            let prev_date = self.date_time.date;
            let prev_time = self.date_time.time;
            self.date_time = clamped;
            self.update();
            self.date_time_changed.emit(&self.date_time);
            if self.date_time.date != prev_date {
                self.date_changed.emit(&self.date_time.date);
            }
            if self.date_time.time != prev_time {
                self.time_changed.emit(&self.date_time.time);
            }
        }
    }

    pub fn date(&self) -> Date {
        self.date_time.date
    }

    pub fn set_date(&mut self, date: Date) {
        let dt = DateTime::new(date, self.date_time.time);
        self.set_date_time(dt);
    }

    pub fn time(&self) -> Time {
        self.date_time.time
    }

    pub fn set_time(&mut self, time: Time) {
        let dt = DateTime::new(self.date_time.date, time);
        self.set_date_time(dt);
    }
    pub fn set_current_section(&mut self, section: DateTimeSection) {
        if let Some(index) = self.sections.iter().position(|&s| s == section) {
            self.current_section = index;
        }
    }

    pub fn current_section(&self) -> DateTimeSection {
        self.sections
            .get(self.current_section)
            .copied()
            .unwrap_or(DateTimeSection::NoSection)
    }
    pub fn display_format(&self) -> &str {
        &self.format_str
    }

    pub fn set_display_format(&mut self, format: impl Into<String>) {
        let format = format.into();
        if self.format_str != format {
            self.format_str = format;
            self.update();
        }
    }

    pub fn step_by(&mut self, steps: i32) {
        if self.sections.is_empty() {
            return;
        }
        let sec = self.sections[self.current_section.min(self.sections.len() - 1)];
        let mut d = self.date_time.date;
        let mut t = self.date_time.time;

        match sec {
            DateTimeSection::YearSection => {
                d.year += steps;
                d.day = d.day.min(Date::days_in_month_of(d.year, d.month));
            }
            DateTimeSection::MonthSection => {
                let total_m = (d.year * 12 + (d.month as i32 - 1)) + steps;
                d.year = total_m.div_euclid(12);
                d.month = (total_m.rem_euclid(12) + 1) as u32;
                let max_d = Date::days_in_month_of(d.year, d.month);
                d.day = d.day.min(max_d);
            }
            DateTimeSection::DaySection => {
                d = d.add_days(steps as i64);
            }
            DateTimeSection::HourSection
            | DateTimeSection::MinuteSection
            | DateTimeSection::SecondSection => {
                let unit = match sec {
                    DateTimeSection::HourSection => 3600i64,
                    DateTimeSection::MinuteSection => 60,
                    DateTimeSection::SecondSection => 1,
                    _ => unreachable!(),
                };
                let total = t.hour as i64 * 3600
                    + t.minute as i64 * 60
                    + t.second as i64
                    + steps as i64 * unit;
                let day_delta = total.div_euclid(86_400);
                let seconds_in_day = total.rem_euclid(86_400);
                d = d.add_days(day_delta);
                t.hour = (seconds_in_day / 3600) as u32;
                t.minute = ((seconds_in_day / 60) % 60) as u32;
                t.second = (seconds_in_day % 60) as u32;
            }
            DateTimeSection::NoSection => {}
        }

        self.set_date_time(DateTime::new(d, t));
    }

    pub fn step_up(&mut self) {
        self.step_by(1);
    }

    pub fn step_down(&mut self) {
        self.step_by(-1);
    }

    fn clamp_datetime(&self, dt: DateTime) -> DateTime {
        if dt < self.min_date_time {
            self.min_date_time
        } else if dt > self.max_date_time {
            self.max_date_time
        } else {
            dt
        }
    }

    fn formatted_text(&self) -> String {
        let d = self.date_time.date;
        let t = self.date_time.time;
        self.format_str
            .replace("yyyy", &format!("{:04}", d.year))
            .replace("MM", &format!("{:02}", d.month))
            .replace("dd", &format!("{:02}", d.day))
            .replace("HH", &format!("{:02}", t.hour))
            .replace("mm", &format!("{:02}", t.minute))
            .replace("ss", &format!("{:02}", t.second))
    }
}

impl QObject for DateTimeEdit {
    leaf_qobject_common!();

    fn event(&mut self, event: &mut Event) -> bool {
        input_common::dispatch_input_event(self, event)
    }
}

impl Widget for DateTimeEdit {
    leaf_widget_common!();

    fn size_hint(&self) -> Size {
        Size::new(160, 26)
    }

    fn update(&mut self) {
        self.base.dirty = Some(self.base.geometry);
    }
    fn set_enabled(&mut self, enabled: bool) {
        self.base.enabled = enabled;
        self.update();
    }

    fn set_window_id(&mut self, window_id: Option<qtrs_core::object::ObjectId>) {
        self.base.window_id = window_id;
    }

    fn paint_event(&mut self, painter: &mut Painter) {
        let r = self.geometry();
        let rf = RectF::new(0.0, 0.0, r.width as f32, r.height as f32);
        let border_color = if self.has_focus() {
            Color::from_rgba8(0, 120, 215, 255)
        } else {
            Color::from_rgba8(190, 190, 190, 255)
        };
        painter.set_brush(Brush::Color(Color::from_rgba8(255, 255, 255, 255)));
        painter.set_pen(Pen::new(border_color, 1.0));
        painter.draw_rounded_rect(rf, 3.0, 3.0);

        let btn_x = (r.width - BUTTON_WIDTH) as f32;
        let half_h = (r.height as f32) * 0.5;

        let up_rf = RectF::new(btn_x, 1.0, BUTTON_WIDTH as f32 - 1.0, half_h - 1.0);
        let up_bg = if self.up_pressed {
            Color::from_rgba8(200, 220, 240, 255)
        } else {
            Color::from_rgba8(240, 240, 240, 255)
        };
        painter.set_brush(Brush::Color(up_bg));
        painter.set_pen(Pen::new(Color::TRANSPARENT, 0.0));
        painter.draw_rect(up_rf);

        painter.set_pen(Pen::new(Color::from_rgba8(60, 60, 60, 255), 1.5));
        painter.draw_line(
            PointF::new(btn_x + 5.0, half_h * 0.65),
            PointF::new(btn_x + (BUTTON_WIDTH as f32) * 0.5, half_h * 0.25),
        );
        painter.draw_line(
            PointF::new(btn_x + (BUTTON_WIDTH as f32) * 0.5, half_h * 0.25),
            PointF::new(btn_x + BUTTON_WIDTH as f32 - 5.0, half_h * 0.65),
        );

        let down_rf = RectF::new(btn_x, half_h, BUTTON_WIDTH as f32 - 1.0, half_h - 1.0);
        let down_bg = if self.down_pressed {
            Color::from_rgba8(200, 220, 240, 255)
        } else {
            Color::from_rgba8(240, 240, 240, 255)
        };
        painter.set_brush(Brush::Color(down_bg));
        painter.set_pen(Pen::new(Color::TRANSPARENT, 0.0));
        painter.draw_rect(down_rf);

        painter.set_pen(Pen::new(Color::from_rgba8(60, 60, 60, 255), 1.5));
        painter.draw_line(
            PointF::new(btn_x + 5.0, half_h + half_h * 0.35),
            PointF::new(btn_x + (BUTTON_WIDTH as f32) * 0.5, half_h + half_h * 0.75),
        );
        painter.draw_line(
            PointF::new(btn_x + (BUTTON_WIDTH as f32) * 0.5, half_h + half_h * 0.75),
            PointF::new(btn_x + BUTTON_WIDTH as f32 - 5.0, half_h + half_h * 0.35),
        );

        painter.set_pen(Pen::new(Color::from_rgba8(210, 210, 210, 255), 1.0));
        painter.draw_line(
            PointF::new(btn_x, 1.0),
            PointF::new(btn_x, r.height as f32 - 1.0),
        );

        let text = self.formatted_text();
        let m = FontMetrics::from_font(&self.font);
        let text_y = (r.height as f32 + m.ascent - m.descent) * 0.5;
        painter.set_pen(Pen::new(Color::from_rgba8(20, 20, 20, 255), 1.0));
        painter.draw_text(PointF::new(8.0, text_y), &text, &self.font);
    }

    fn mouse_press_event(&mut self, pos: Point, button: u32, _modifiers: u32) {
        if button != 1 {
            return;
        }
        let r = self.geometry();
        if pos.x >= r.width - BUTTON_WIDTH {
            let half_h = r.height / 2;
            if pos.y < half_h {
                self.up_pressed = true;
                self.step_up();
            } else {
                self.down_pressed = true;
                self.step_down();
            }
            self.base.has_focus = true;
            self.update();
        } else {
            self.base.has_focus = true;
        }
    }

    fn mouse_release_event(&mut self, _pos: Point, _button: u32, _modifiers: u32) {
        if self.up_pressed || self.down_pressed {
            self.up_pressed = false;
            self.down_pressed = false;
            self.update();
        }
    }

    fn key_press_event(&mut self, key: u32, _modifiers: u32, _is_repeat: bool) {
        if input_keys::is_up(key) {
            self.step_up();
        } else if input_keys::is_down(key) {
            self.step_down();
        } else if input_keys::is_left(key) {
            if self.current_section > 0 {
                self.current_section -= 1;
                self.update();
            }
        } else if input_keys::is_right(key) || key == 0x0100_0001 {
            if self.current_section + 1 < self.sections.len() {
                self.current_section += 1;
                self.update();
            }
        }
    }

    fn wheel_event(&mut self, _pos: Point, delta_y: i32, _modifiers: u32) {
        if delta_y > 0 {
            self.step_up();
        } else if delta_y < 0 {
            self.step_down();
        }
    }
}

pub struct DateEdit {
    inner: DateTimeEdit,
}

pub type QDateEdit = DateEdit;

impl DateEdit {
    pub fn new() -> Self {
        Self::with_date(Date::new(2026, 9, 26))
    }

    pub fn with_date(date: Date) -> Self {
        let mut dt = DateTimeEdit::with_datetime(DateTime::new(date, Time::new(0, 0, 0, 0)));
        dt.sections = vec![
            DateTimeSection::YearSection,
            DateTimeSection::MonthSection,
            DateTimeSection::DaySection,
        ];
        dt.format_str = "yyyy-MM-dd".to_string();
        Self { inner: dt }
    }

    pub fn date(&self) -> Date {
        self.inner.date()
    }

    pub fn set_date(&mut self, date: Date) {
        self.inner.set_date(date);
    }

    pub fn step_up(&mut self) {
        self.inner.step_up();
    }

    pub fn step_down(&mut self) {
        self.inner.step_down();
    }
}

impl QObject for DateEdit {
    fn object_data(&self) -> &qtrs_core::object::ObjectData {
        self.inner.object_data()
    }

    fn object_data_mut(&mut self) -> &mut qtrs_core::object::ObjectData {
        self.inner.object_data_mut()
    }

    fn event(&mut self, event: &mut Event) -> bool {
        self.inner.event(event)
    }
}

impl Widget for DateEdit {
    fn id(&self) -> ObjectId {
        self.inner.id()
    }
    fn geometry(&self) -> Rect {
        self.inner.geometry()
    }
    fn set_geometry(&mut self, rect: Rect) {
        self.inner.set_geometry(rect);
    }
    fn is_visible(&self) -> bool {
        self.inner.is_visible()
    }
    fn set_visible(&mut self, v: bool) {
        self.inner.set_visible(v);
    }
    fn is_enabled(&self) -> bool {
        self.inner.is_enabled()
    }
    fn set_enabled(&mut self, e: bool) {
        self.inner.set_enabled(e);
    }
    fn update(&mut self) {
        self.inner.update();
    }
    fn dirty_rect(&self) -> Option<Rect> {
        self.inner.dirty_rect()
    }
    fn clear_dirty(&mut self) {
        self.inner.clear_dirty();
    }
    fn layout(&self) -> Option<&dyn Layout> {
        self.inner.layout()
    }
    fn layout_mut(&mut self) -> Option<&mut Box<dyn Layout>> {
        self.inner.layout_mut()
    }
    fn set_layout(&mut self, l: Box<dyn Layout>) {
        self.inner.set_layout(l);
    }
    fn parent_widget(&self) -> Option<WidgetWeak> {
        self.inner.parent_widget()
    }
    fn set_parent_widget(&mut self, p: Option<WidgetWeak>) {
        self.inner.set_parent_widget(p);
    }
    fn window_id(&self) -> Option<ObjectId> {
        self.inner.window_id()
    }
    fn set_window_id(&mut self, w: Option<ObjectId>) {
        self.inner.set_window_id(w);
    }
    fn children(&self) -> Vec<WidgetRef> {
        self.inner.children()
    }
    fn add_child(&mut self, c: WidgetRef) {
        self.inner.add_child(c);
    }
    fn remove_child(&mut self, cid: ObjectId) {
        self.inner.remove_child(cid);
    }
    fn size_hint(&self) -> Size {
        self.inner.size_hint()
    }
    fn size_policy(&self) -> QSizePolicy {
        self.inner.size_policy()
    }
    fn set_size_policy(&mut self, p: QSizePolicy) {
        self.inner.set_size_policy(p);
    }
    fn focus_policy(&self) -> FocusPolicy {
        self.inner.focus_policy()
    }
    fn set_focus_policy(&mut self, p: FocusPolicy) {
        self.inner.set_focus_policy(p);
    }
    fn has_focus(&self) -> bool {
        self.inner.has_focus()
    }
    fn set_has_focus(&mut self, f: bool) {
        self.inner.set_has_focus(f);
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn paint_event(&mut self, painter: &mut Painter) {
        self.inner.paint_event(painter);
    }
}

pub struct TimeEdit {
    inner: DateTimeEdit,
}

pub type QTimeEdit = TimeEdit;

impl TimeEdit {
    pub fn new() -> Self {
        Self::with_time(Time::new(12, 0, 0, 0))
    }

    pub fn with_time(time: Time) -> Self {
        let mut dt = DateTimeEdit::with_datetime(DateTime::new(Date::new(2026, 1, 1), time));
        dt.sections = vec![
            DateTimeSection::HourSection,
            DateTimeSection::MinuteSection,
            DateTimeSection::SecondSection,
        ];
        dt.format_str = "HH:mm:ss".to_string();
        Self { inner: dt }
    }

    pub fn time(&self) -> Time {
        self.inner.time()
    }

    pub fn set_time(&mut self, time: Time) {
        self.inner.set_time(time);
    }

    pub fn step_up(&mut self) {
        self.inner.step_up();
    }

    pub fn step_down(&mut self) {
        self.inner.step_down();
    }
}

impl QObject for TimeEdit {
    fn object_data(&self) -> &qtrs_core::object::ObjectData {
        self.inner.object_data()
    }

    fn object_data_mut(&mut self) -> &mut qtrs_core::object::ObjectData {
        self.inner.object_data_mut()
    }

    fn event(&mut self, event: &mut Event) -> bool {
        self.inner.event(event)
    }
}

impl Widget for TimeEdit {
    fn id(&self) -> ObjectId {
        self.inner.id()
    }
    fn geometry(&self) -> Rect {
        self.inner.geometry()
    }
    fn set_geometry(&mut self, rect: Rect) {
        self.inner.set_geometry(rect);
    }
    fn is_visible(&self) -> bool {
        self.inner.is_visible()
    }
    fn set_visible(&mut self, v: bool) {
        self.inner.set_visible(v);
    }
    fn is_enabled(&self) -> bool {
        self.inner.is_enabled()
    }
    fn set_enabled(&mut self, e: bool) {
        self.inner.set_enabled(e);
    }
    fn update(&mut self) {
        self.inner.update();
    }
    fn dirty_rect(&self) -> Option<Rect> {
        self.inner.dirty_rect()
    }
    fn clear_dirty(&mut self) {
        self.inner.clear_dirty();
    }
    fn layout(&self) -> Option<&dyn Layout> {
        self.inner.layout()
    }
    fn layout_mut(&mut self) -> Option<&mut Box<dyn Layout>> {
        self.inner.layout_mut()
    }
    fn set_layout(&mut self, l: Box<dyn Layout>) {
        self.inner.set_layout(l);
    }
    fn parent_widget(&self) -> Option<WidgetWeak> {
        self.inner.parent_widget()
    }
    fn set_parent_widget(&mut self, p: Option<WidgetWeak>) {
        self.inner.set_parent_widget(p);
    }
    fn window_id(&self) -> Option<ObjectId> {
        self.inner.window_id()
    }
    fn set_window_id(&mut self, w: Option<ObjectId>) {
        self.inner.set_window_id(w);
    }
    fn children(&self) -> Vec<WidgetRef> {
        self.inner.children()
    }
    fn add_child(&mut self, c: WidgetRef) {
        self.inner.add_child(c);
    }
    fn remove_child(&mut self, cid: ObjectId) {
        self.inner.remove_child(cid);
    }
    fn size_hint(&self) -> Size {
        self.inner.size_hint()
    }
    fn size_policy(&self) -> QSizePolicy {
        self.inner.size_policy()
    }
    fn set_size_policy(&mut self, p: QSizePolicy) {
        self.inner.set_size_policy(p);
    }
    fn focus_policy(&self) -> FocusPolicy {
        self.inner.focus_policy()
    }
    fn set_focus_policy(&mut self, p: FocusPolicy) {
        self.inner.set_focus_policy(p);
    }
    fn has_focus(&self) -> bool {
        self.inner.has_focus()
    }
    fn set_has_focus(&mut self, f: bool) {
        self.inner.set_has_focus(f);
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn paint_event(&mut self, painter: &mut Painter) {
        self.inner.paint_event(painter);
    }
}
