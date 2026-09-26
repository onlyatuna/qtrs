use qtrs_widgets::{PlainTextEdit, TextBrowser, TextEdit, Widget};

#[test]
fn text_edit_backspace_removes_one_emoji_grapheme() {
    let mut edit = TextEdit::with_text("a👩‍👩‍👧‍👦b");
    edit.set_cursor_position(2);
    edit.key_press_event(0x08, 0, false);
    assert_eq!(edit.to_plain_text(), "ab");
    assert_eq!(edit.cursor_position(), 1);
}

#[test]
fn text_edit_multiline_navigation_selection_and_undo_redo() {
    let mut edit = TextEdit::with_text("first\nsecond");
    edit.set_cursor_position(2);
    edit.key_press_event(0x28, 0, false);
    assert_eq!(edit.cursor_position(), 8);
    edit.key_press_event(0x26, 0, false);
    assert_eq!(edit.cursor_position(), 2);
    edit.set_cursor_position(9);
    edit.key_press_event(0x25, 1, false);
    assert_eq!(edit.selected_text(), "c");
    edit.insert_text("X\nY");
    assert_eq!(edit.to_plain_text(), "first\nseX\nYond");
    edit.undo();
    assert_eq!(edit.to_plain_text(), "first\nsecond");
    edit.redo();
    assert_eq!(edit.to_plain_text(), "first\nseX\nYond");
}

#[test]
fn plain_text_edit_trims_old_blocks_and_appends_lines() {
    let mut edit = PlainTextEdit::new();
    edit.set_maximum_block_count(2);
    edit.set_plain_text("one\ntwo");
    edit.append_line("three");
    assert_eq!(edit.to_plain_text(), "two\nthree");
    assert_eq!(edit.maximum_block_count(), 2);
}

#[test]
fn text_browser_extracts_links_and_navigates_history() {
    let mut browser = TextBrowser::new();
    browser.set_source("<p><a href=\"https://example.test/next\">next</a></p>");
    assert_eq!(browser.anchor_at(0), Some("https://example.test/next"));
    assert!(browser.activate_link_at(0));
    assert_eq!(browser.source(), "https://example.test/next");
    assert_eq!(browser.to_plain_text(), "https://example.test/next");
    assert!(browser.backward());
    assert!(browser.source().starts_with("<p>"));
    assert!(browser.forward());
    assert_eq!(browser.source(), "https://example.test/next");
}
