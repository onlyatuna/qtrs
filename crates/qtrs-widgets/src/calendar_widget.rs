//! Monthly calendar grid widget (`QCalendarWidget`).

use qtrs_core::event::Event;
use qtrs_core::object::QObject;
use qtrs_core::signal::Signal;
use qtrs_core::types::Date;
use qtrs_gui::geometry::primitives::{Point, PointF, Rect, RectF, Size};
use qtrs_gui::paint::{Brush, Painter, Pen};
use qtrs_gui::text::{Font, FontMetrics, FontWeight};
use qtrs_gui::tiny_skia::Color;

use crate::focus::FocusPolicy;
use crate::input_common;
use crate::input_keys;
use crate::size_policy::{Policy, QSizePolicy};
use crate::widget::{Widget, WidgetBase};

pub struct CalendarWidget {
    pub base: WidgetBase,
    selected_date: Date,
    current_year: i32,
    current_month: u32,
    min_date: Date,
    max_date: Date,
    first_day_of_week: u32, // 1 = Mon, 7 = Sun
    pub selection_changed: Signal<Date>,
    pub clicked: Signal<Date>,
    pub current_page_changed: Signal<(i32, u32)>,
    font: Font,
    header_font: Font,
}

pub type QCalendarWidget = CalendarWidget;

const NAV_BAR_HEIGHT: i32 = 32;
const DOW_HEADER_HEIGHT: i32 = 24;
const DAYS_PER_WEEK: usize = 7;
const WEEKS_DISPLAYED: usize = 6;

impl CalendarWidget {
    pub fn new() -> Self {
        let today = Date::new(2026, 9, 26);
        let mut base = WidgetBase::new();
        base.focus_policy = FocusPolicy::StrongFocus;
        base.size_policy = QSizePolicy::new(Policy::Preferred, Policy::Preferred);
        base.geometry = Rect::new(0, 0, 280, 240);

        let font = Font::new("Segoe UI", 12.0);
        let mut header_font = Font::new("Segoe UI", 12.0);
        header_font.weight = FontWeight::Bold;

        Self {
            base,
            selected_date: today,
            current_year: today.year,
            current_month: today.month,
            min_date: Date::new(1900, 1, 1),
            max_date: Date::new(2100, 12, 31),
            first_day_of_week: 1, // Monday
            selection_changed: Signal::new(),
            clicked: Signal::new(),
            current_page_changed: Signal::new(),
            font,
            header_font,
        }
    }

    pub fn selected_date(&self) -> Date {
        self.selected_date
    }

    pub fn set_selected_date(&mut self, date: Date) {
        if !date.is_valid() {
            return;
        }
        let clamped = self.clamp_date(date);
        if self.selected_date != clamped {
            self.selected_date = clamped;
            self.current_year = clamped.year;
            self.current_month = clamped.month;
            self.update();
            self.selection_changed.emit(&self.selected_date);
        }
    }
    pub fn current_page(&self) -> (i32, u32) {
        (self.current_year, self.current_month)
    }

    pub fn first_day_of_week(&self) -> u32 {
        self.first_day_of_week
    }

    /// Sets the first weekday using ISO numbering (Monday=1 through Sunday=7).
    pub fn set_first_day_of_week(&mut self, day: u32) {
        if (1..=7).contains(&day) && day != self.first_day_of_week {
            self.first_day_of_week = day;
            self.update();
        }
    }

    pub fn set_current_page(&mut self, year: i32, month: u32) {
        if (1..=12).contains(&month) && (self.current_year != year || self.current_month != month) {
            self.current_year = year;
            self.current_month = month;
            self.update();
            self.current_page_changed.emit(&(year, month));
        }
    }

    pub fn show_prev_month(&mut self) {
        if self.current_month == 1 {
            self.set_current_page(self.current_year - 1, 12);
        } else {
            self.set_current_page(self.current_year, self.current_month - 1);
        }
    }

    pub fn show_next_month(&mut self) {
        if self.current_month == 12 {
            self.set_current_page(self.current_year + 1, 1);
        } else {
            self.set_current_page(self.current_year, self.current_month + 1);
        }
    }

    pub fn set_date_range(&mut self, min: Date, max: Date) {
        if min <= max {
            self.min_date = min;
            self.max_date = max;
            let clamped = self.clamp_date(self.selected_date);
            self.set_selected_date(clamped);
        }
    }

    fn clamp_date(&self, date: Date) -> Date {
        if date < self.min_date {
            self.min_date
        } else if date > self.max_date {
            self.max_date
        } else {
            date
        }
    }

    fn grid_rect(&self) -> Rect {
        let r = self.geometry();
        let top = NAV_BAR_HEIGHT + DOW_HEADER_HEIGHT;
        Rect::new(0, top, r.width, r.height - top)
    }

    fn cell_rect(&self, col: usize, row: usize) -> Rect {
        let grid = self.grid_rect();
        let cell_w = grid.width / DAYS_PER_WEEK as i32;
        let cell_h = grid.height / WEEKS_DISPLAYED as i32;
        Rect::new(
            grid.x + (col as i32) * cell_w,
            grid.y + (row as i32) * cell_h,
            cell_w,
            cell_h,
        )
    }

    fn date_at_cell(&self, col: usize, row: usize) -> Date {
        let first_of_month = Date::new(self.current_year, self.current_month, 1);
        let first_dow = first_of_month.day_of_week();
        let offset = ((first_dow + 7 - self.first_day_of_week) % 7) as i32;
        let day_index = (row * DAYS_PER_WEEK + col) as i32 - offset + 1;

        if day_index < 1 {
            let (prev_y, prev_m) = if self.current_month == 1 {
                (self.current_year - 1, 12)
            } else {
                (self.current_year, self.current_month - 1)
            };
            let days_in_prev = Date::days_in_month_of(prev_y, prev_m);
            Date::new(prev_y, prev_m, (days_in_prev as i32 + day_index) as u32)
        } else {
            let days_in_cur = Date::days_in_month_of(self.current_year, self.current_month);
            if day_index <= days_in_cur as i32 {
                Date::new(self.current_year, self.current_month, day_index as u32)
            } else {
                let (next_y, next_m) = if self.current_month == 12 {
                    (self.current_year + 1, 1)
                } else {
                    (self.current_year, self.current_month + 1)
                };
                Date::new(next_y, next_m, (day_index - days_in_cur as i32) as u32)
            }
        }
    }

    fn cell_at_pos(&self, pos: Point) -> Option<(usize, usize)> {
        let grid = self.grid_rect();
        if !grid.contains(pos) {
            return None;
        }
        let cell_w = grid.width / DAYS_PER_WEEK as i32;
        let cell_h = grid.height / WEEKS_DISPLAYED as i32;
        if cell_w <= 0 || cell_h <= 0 {
            return None;
        }
        let col = ((pos.x - grid.x) / cell_w).clamp(0, 6) as usize;
        let row = ((pos.y - grid.y) / cell_h).clamp(0, 5) as usize;
        Some((col, row))
    }
}

impl QObject for CalendarWidget {
    leaf_qobject_common!();

    fn event(&mut self, event: &mut Event) -> bool {
        input_common::dispatch_input_event(self, event)
    }
}

impl Widget for CalendarWidget {
    leaf_widget_common!();

    fn size_hint(&self) -> Size {
        Size::new(280, 240)
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

        painter.set_brush(Brush::Color(Color::from_rgba8(255, 255, 255, 255)));
        painter.set_pen(Pen::new(Color::from_rgba8(200, 200, 200, 255), 1.0));
        painter.draw_rect(rf);

        let nav_rf = RectF::new(0.0, 0.0, r.width as f32, NAV_BAR_HEIGHT as f32);
        painter.set_pen(Pen::new(Color::TRANSPARENT, 0.0));
        painter.draw_rect(nav_rf);

        painter.set_pen(Pen::new(Color::from_rgba8(80, 80, 80, 255), 2.0));
        painter.draw_line(PointF::new(26.0, 11.0), PointF::new(20.0, 16.0));
        painter.draw_line(PointF::new(20.0, 16.0), PointF::new(26.0, 21.0));

        let next_x = (r.width - 26) as f32;
        painter.draw_line(PointF::new(next_x, 11.0), PointF::new(next_x + 6.0, 16.0));
        painter.draw_line(PointF::new(next_x + 6.0, 16.0), PointF::new(next_x, 21.0));

        let month_names = [
            "January",
            "February",
            "March",
            "April",
            "May",
            "June",
            "July",
            "August",
            "September",
            "October",
            "November",
            "December",
        ];
        let month_str = month_names
            .get((self.current_month.saturating_sub(1)) as usize)
            .unwrap_or(&"");
        let title = format!("{} {}", month_str, self.current_year);
        let t_metrics = FontMetrics::from_font(&self.header_font);
        let t_w = t_metrics.horizontal_advance(&title, &self.header_font);
        let t_x = ((r.width as f32) - t_w) * 0.5;
        let t_y = 16.0 + (t_metrics.ascent - t_metrics.descent) * 0.5;
        painter.set_pen(Pen::new(Color::from_rgba8(20, 20, 20, 255), 1.0));
        painter.draw_text(PointF::new(t_x, t_y), &title, &self.header_font);

        let dows = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
        let dow_y = NAV_BAR_HEIGHT as f32;
        let cell_w = (r.width / DAYS_PER_WEEK as i32) as f32;
        for (i, dow) in dows.iter().enumerate() {
            let col_x = (i as f32) * cell_w;
            let m = FontMetrics::from_font(&self.font);
            let w = m.horizontal_advance(dow, &self.font);
            let x = col_x + (cell_w - w) * 0.5;
            let y = dow_y + 16.0;
            let dow_color = if i >= 5 {
                Color::from_rgba8(180, 50, 50, 255)
            } else {
                Color::from_rgba8(100, 100, 100, 255)
            };
            painter.set_pen(Pen::new(dow_color, 1.0));
            painter.draw_text(PointF::new(x, y), dow, &self.font);
        }

        for row in 0..WEEKS_DISPLAYED {
            for col in 0..DAYS_PER_WEEK {
                let cell = self.cell_rect(col, row);
                let date = self.date_at_cell(col, row);
                let is_cur_month = date.month == self.current_month;
                let is_selected = date == self.selected_date;

                let cell_rf = RectF::new(
                    cell.x as f32,
                    cell.y as f32,
                    cell.width as f32,
                    cell.height as f32,
                );

                if is_selected {
                    let sel_rf = cell_rf.adjusted(2.0, 2.0, -2.0, -2.0);
                    painter.set_brush(Brush::Color(Color::from_rgba8(0, 120, 215, 255)));
                    painter.set_pen(Pen::new(Color::TRANSPARENT, 0.0));
                    painter.draw_rounded_rect(sel_rf, 3.0, 3.0);
                }

                let day_str = date.day.to_string();
                let m = FontMetrics::from_font(&self.font);
                let w = m.horizontal_advance(&day_str, &self.font);
                let text_x = cell_rf.x + (cell_rf.width - w) * 0.5;
                let text_y = cell_rf.y + (cell_rf.height + m.ascent - m.descent) * 0.5;

                let text_color = if is_selected {
                    Color::from_rgba8(255, 255, 255, 255)
                } else if !is_cur_month {
                    Color::from_rgba8(180, 180, 180, 255)
                } else if col >= 5 {
                    Color::from_rgba8(200, 60, 60, 255)
                } else {
                    Color::from_rgba8(30, 30, 30, 255)
                };

                painter.set_pen(Pen::new(text_color, 1.0));
                painter.draw_text(PointF::new(text_x, text_y), &day_str, &self.font);
            }
        }
    }

    fn mouse_press_event(&mut self, pos: Point, button: u32, _modifiers: u32) {
        if button != 1 {
            return;
        }

        if pos.y < NAV_BAR_HEIGHT {
            if pos.x < 40 {
                self.show_prev_month();
                return;
            } else if pos.x > self.geometry().width - 40 {
                self.show_next_month();
                return;
            }
        }

        if let Some((col, row)) = self.cell_at_pos(pos) {
            let date = self.date_at_cell(col, row);
            self.set_selected_date(date);
            self.clicked.emit(&date);
            self.base.has_focus = true;
        }
    }

    fn key_press_event(&mut self, key: u32, _modifiers: u32, _is_repeat: bool) {
        let cur = self.selected_date;
        let next_date = if input_keys::is_left(key) {
            cur.add_days(-1)
        } else if input_keys::is_right(key) {
            cur.add_days(1)
        } else if input_keys::is_up(key) {
            cur.add_days(-7)
        } else if input_keys::is_down(key) {
            cur.add_days(7)
        } else {
            return;
        };
        self.set_selected_date(next_date);
    }
}
