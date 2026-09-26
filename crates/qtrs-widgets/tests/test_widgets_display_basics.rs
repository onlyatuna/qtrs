use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use qtrs_widgets::{Frame, FrameShape, GroupBox, KeySequence, LCDNumber, QKeySequenceEdit};

#[test]
fn frame_width_tracks_frame_shape_and_line_width() {
    let mut frame = Frame::new();
    frame.set_line_width(2);
    frame.set_frame_shape(FrameShape::Box);
    assert_eq!(frame.frame_width(), 2);
    frame.set_frame_shape(FrameShape::HLine);
    assert_eq!(frame.frame_width(), 2);
    frame.set_frame_shape(FrameShape::NoFrame);
    assert_eq!(frame.frame_width(), 0);
}

#[test]
fn group_box_toggle_emits_state_and_updates_title() {
    let mut group = GroupBox::new("&Options");
    let toggles = Arc::new(AtomicUsize::new(0));
    let count = toggles.clone();
    let _connection = group.toggled.connect(move |_| {
        count.fetch_add(1, Ordering::SeqCst);
    });
    group.set_checkable(true);
    group.set_checked(false);
    group.set_checked(true);
    assert_eq!(group.display_title(), "Options");
    assert_eq!(toggles.load(Ordering::SeqCst), 2);
}

#[test]
fn lcd_number_detects_overflow_and_displays_values() {
    let mut lcd = LCDNumber::new();
    lcd.set_digit_count(2);
    assert!(lcd.check_overflow_i32(123));
    lcd.display_i32(42);
    assert_eq!(lcd.int_value(), 42);
}

#[test]
fn key_sequence_round_trips_chords() {
    let seq = KeySequence::from_portable_string("Ctrl+Shift+S, Ctrl+X");
    assert_eq!(seq.to_portable_string(), "Ctrl+Shift+S, Ctrl+X");
    let mut edit = QKeySequenceEdit::new();
    edit.set_key_sequence(seq.clone());
    assert_eq!(edit.key_sequence(), &seq);
}
