//! Integration tests verifying the strongly-typed Text Model, GraphemeIndex,
//! TextPosition, and TextRange in LineEdit (preventing broken ZWJ emojis, CJK, and flags).

use qtrs_core::event::{Event, EventKind};
use qtrs_core::object::QObject;
use qtrs_gui::text::GraphemeIndex;
use qtrs_widgets::line_edit::LineEdit;

#[test]
fn test_line_edit_emoji_and_grapheme_navigation() {
    let mut le = LineEdit::new();

    // Family emoji: Man + ZWJ + Woman + ZWJ + Girl + ZWJ + Boy
    // 7 Unicode scalar values, 25 UTF-8 bytes, but exactly 1 user-perceived grapheme cluster!
    let family_emoji = "👨‍👩‍👧‍👦";
    le.set_text(family_emoji);

    // Cursor position must be 1 grapheme, NOT 7!
    assert_eq!(le.cursor_position(), GraphemeIndex(1));
    assert_eq!(le.cursor_position(), 1); // Deref / PartialEq<usize> parity

    let text_pos = le.cursor_text_position();
    assert_eq!(text_pos.grapheme.0, 1);
    assert_eq!(text_pos.byte.0, family_emoji.len());

    // Left Arrow moves back 1 grapheme (to position 0)
    let mut ev_left = Event::new_spontaneous(EventKind::KeyPress {
        key: 0x25,
        modifiers: 0,
        is_repeat: false,
    });
    le.event(&mut ev_left);
    assert_eq!(le.cursor_position(), 0);

    // Right Arrow moves forward 1 grapheme (to position 1)
    let mut ev_right = Event::new_spontaneous(EventKind::KeyPress {
        key: 0x27,
        modifiers: 0,
        is_repeat: false,
    });
    le.event(&mut ev_right);
    assert_eq!(le.cursor_position(), 1);

    // Backspace: must delete the entire 7-scalar ZWJ emoji in ONE action!
    let mut ev_backspace = Event::new_spontaneous(EventKind::KeyPress {
        key: 0x08,
        modifiers: 0,
        is_repeat: false,
    });
    le.event(&mut ev_backspace);
    assert_eq!(le.text(), "", "Backspace must delete whole ZWJ cluster without leaving orphan symbols");
    assert_eq!(le.cursor_position(), 0);
}

#[test]
fn test_line_edit_selection_range_and_replacement() {
    let mut le = LineEdit::new();
    // "🇨🇦 Canada" -> Flag (1 grapheme) + space (1) + "Canada" (6) = 8 graphemes
    le.set_text("🇨🇦 Canada");
    assert_eq!(le.cursor_position(), 8);

    // Select All
    le.select_all();
    assert!(le.has_selected_text());
    assert_eq!(le.selected_text(), "🇨🇦 Canada");

    if let Some(range) = le.selection_range() {
        assert_eq!(range.grapheme_len(), 8);
        assert_eq!(range.slice_str(le.text()), "🇨🇦 Canada");
    } else {
        panic!("Selection range should be present");
    }

    // Type 'R' to replace selection
    let mut ev_r = Event::new_spontaneous(EventKind::KeyPress {
        key: 0x52, // 'R'
        modifiers: 0,
        is_repeat: false,
    });
    le.event(&mut ev_r);

    assert_eq!(le.text(), "R");
    assert_eq!(le.cursor_position(), 1);
    assert!(!le.has_selected_text());
}

#[test]
fn test_line_edit_cjk_character_stability() {
    let mut le = LineEdit::new();
    // 6 Chinese characters: 6 graphemes, 18 UTF-8 bytes
    le.set_text("繁體中文測試");
    assert_eq!(le.cursor_position(), 6);

    let text_pos = le.cursor_text_position();
    assert_eq!(text_pos.grapheme.0, 6);
    assert_eq!(text_pos.byte.0, 18);

    // Move back 3 graphemes
    for _ in 0..3 {
        let mut ev_left = Event::new_spontaneous(EventKind::KeyPress {
            key: 0x25,
            modifiers: 0,
            is_repeat: false,
        });
        le.event(&mut ev_left);
    }
    assert_eq!(le.cursor_position(), 3);
    assert_eq!(le.cursor_text_position().byte.0, 9); // exactly "繁體中" (9 bytes)

    // Backspace: deletes "中"
    let mut ev_backspace = Event::new_spontaneous(EventKind::KeyPress {
        key: 0x08,
        modifiers: 0,
        is_repeat: false,
    });
    le.event(&mut ev_backspace);
    assert_eq!(le.text(), "繁體文測試");
    assert_eq!(le.cursor_position(), 2);
}
