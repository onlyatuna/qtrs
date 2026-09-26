use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use qtrs_core::event::{Event, EventKind};
use qtrs_core::object::QObject;
use qtrs_core::types::{Date, DateTime, Time};
use qtrs_widgets::{
    CalendarWidget, CommandLinkButton, DateEdit, DateTimeEdit, DateTimeSection, FontComboBox,
    TimeEdit, Widget,
};

#[test]
fn calendar_page_navigation_and_date_clamping() {
    let mut calendar = CalendarWidget::new();
    calendar.set_current_page(2024, 1);
    calendar.show_prev_month();
    assert_eq!(calendar.current_page(), (2023, 12));
    calendar.show_next_month();
    assert_eq!(calendar.current_page(), (2024, 1));

    calendar.set_date_range(Date::new(2024, 2, 10), Date::new(2024, 2, 20));
    calendar.set_selected_date(Date::new(2024, 2, 1));
    assert_eq!(calendar.selected_date(), Date::new(2024, 2, 10));
}

#[test]
fn date_edit_steps_calendar_year() {
    let mut edit = DateEdit::with_date(Date::new(2024, 1, 31));
    edit.step_up();
    assert_eq!(edit.date(), Date::new(2025, 1, 31));
    let mut leap = DateEdit::with_date(Date::new(2024, 2, 29));
    leap.step_up();
    assert_eq!(leap.date(), Date::new(2025, 2, 28));
}

#[test]
fn datetime_edit_section_steps_carry_across_date_boundary() {
    let mut edit = DateTimeEdit::with_datetime(DateTime::new(
        Date::new(2024, 2, 29),
        Time::new(23, 59, 59, 0),
    ));
    edit.set_current_section(DateTimeSection::SecondSection);
    edit.step_up();
    assert_eq!(edit.date(), Date::new(2024, 3, 1));
    assert_eq!(edit.time(), Time::new(0, 0, 0, 0));

    edit.set_display_format("dd/MM/yyyy HH:mm");
    assert_eq!(edit.display_format(), "dd/MM/yyyy HH:mm");
}

#[test]
fn time_edit_hour_step_wraps_with_day_carry() {
    let mut time = TimeEdit::with_time(Time::new(23, 59, 59, 0));
    time.step_up();
    assert_eq!(time.time(), Time::new(0, 59, 59, 0));
}

#[test]
fn command_link_clicks_only_after_inside_press_release() {
    let mut button = CommandLinkButton::with_description("Open", "Open a project");
    button.set_geometry(qtrs_gui::geometry::primitives::Rect::new(0, 0, 220, 60));
    let clicks = Arc::new(AtomicUsize::new(0));
    let count = clicks.clone();
    let _connection = button.clicked.connect(move |_| {
        count.fetch_add(1, Ordering::SeqCst);
    });

    let mut press = Event::new_spontaneous(EventKind::MouseButtonPress {
        x: 20,
        y: 20,
        button: 1,
    });
    button.event(&mut press);
    let mut release = Event::new_spontaneous(EventKind::MouseButtonRelease {
        x: 20,
        y: 20,
        button: 1,
    });
    button.event(&mut release);
    assert_eq!(clicks.load(Ordering::SeqCst), 1);

    let mut outside = Event::new_spontaneous(EventKind::MouseButtonPress {
        x: 20,
        y: 20,
        button: 1,
    });
    button.event(&mut outside);
    let mut release = Event::new_spontaneous(EventKind::MouseButtonRelease {
        x: 300,
        y: 100,
        button: 1,
    });
    button.event(&mut release);
    assert_eq!(clicks.load(Ordering::SeqCst), 1);
}

#[test]
fn font_combo_box_exposes_nonempty_family_selection() {
    let fonts = FontComboBox::new();
    assert!(!fonts.current_font().family.is_empty());
}
