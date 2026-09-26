use crate::{
    focus::FocusPolicy,
    input_common, input_keys,
    size_policy::{Policy, QSizePolicy},
    widget::{Widget, WidgetBase},
};
use qtrs_core::event::{Event, EventKind, FocusReason};
use qtrs_core::object::QObject;
use qtrs_core::signal::Signal;
use qtrs_gui::geometry::primitives::{PointF, Rect, RectF, Size};
use qtrs_gui::paint::{
    painter::{Painter, Pen},
    Brush,
};
use qtrs_gui::text::{
    Font, MoveMode, MoveOperation, TextCursor, TextDocument, UnicodeSegmentation,
};
use qtrs_gui::tiny_skia::Color;

/// Rich-text multiline editor backed by qtrs-gui's document and grapheme cursor.
pub struct TextEdit {
    pub(crate) base: WidgetBase,
    document: TextDocument,
    cursor: TextCursor,
    read_only: bool,
    pub text_changed: Signal<String>,
    pub cursor_position_changed: Signal<usize>,
    pub selection_changed: Signal<()>,
    undo: Vec<(TextDocument, TextCursor)>,
    redo: Vec<(TextDocument, TextCursor)>,
    font: Font,
}
pub type QTextEdit = TextEdit;
impl Default for TextEdit {
    fn default() -> Self {
        Self::new()
    }
}
impl TextEdit {
    pub fn new() -> Self {
        let mut base = WidgetBase::new();
        base.focus_policy = FocusPolicy::StrongFocus;
        base.size_policy = QSizePolicy::new(Policy::Expanding, Policy::Expanding);
        base.geometry = Rect::new(0, 0, 240, 120);
        Self {
            base,
            document: TextDocument::new(),
            cursor: TextCursor::new(),
            read_only: false,
            text_changed: Signal::new(),
            cursor_position_changed: Signal::new(),
            selection_changed: Signal::new(),
            undo: Vec::new(),
            redo: Vec::new(),
            font: Font::new("Segoe UI", 13.0),
        }
    }
    pub fn with_text(text: impl Into<String>) -> Self {
        let mut s = Self::new();
        s.set_plain_text(text);
        s
    }
    pub fn document(&self) -> &TextDocument {
        &self.document
    }
    pub fn to_plain_text(&self) -> String {
        self.document.to_plain_text()
    }
    pub fn plain_text(&self) -> String {
        self.to_plain_text()
    }
    pub fn to_html(&self) -> String {
        self.document.to_html()
    }
    pub fn to_markdown(&self) -> String {
        self.document.to_markdown()
    }
    pub fn set_plain_text(&mut self, text: impl Into<String>) {
        self.replace_document(|d| d.set_plain_text(text));
    }
    pub fn set_html(&mut self, text: &str) {
        self.replace_document(|d| d.set_html(text));
    }
    pub fn set_markdown(&mut self, text: &str) {
        self.replace_document(|d| d.set_markdown(text));
    }
    fn replace_document(&mut self, f: impl FnOnce(&mut TextDocument)) {
        let old = self.to_plain_text();
        f(&mut self.document);
        self.cursor = TextCursor::at_position(self.document.character_count());
        self.undo.clear();
        self.redo.clear();
        let now = self.to_plain_text();
        if old != now {
            self.text_changed.emit(&now);
        }
        self.cursor_position_changed.emit(&self.cursor.position());
        self.selection_changed.emit(&());
        self.update();
    }
    pub fn is_read_only(&self) -> bool {
        self.read_only
    }
    pub fn set_read_only(&mut self, v: bool) {
        self.read_only = v;
        self.update();
    }
    pub fn set_word_wrap_width(&mut self, width: f32) {
        self.document.set_text_width(width);
        self.update();
    }
    pub fn word_wrap_width(&self) -> f32 {
        self.document.text_width()
    }
    pub fn cursor_position(&self) -> usize {
        self.cursor.position()
    }
    pub fn set_cursor_position(&mut self, pos: usize) {
        let old = self.cursor.position();
        let sel = self.cursor.has_selection();
        self.cursor.set_position(
            pos.min(self.document.character_count()),
            MoveMode::MoveAnchor,
        );
        if old != self.cursor.position() {
            self.cursor_position_changed.emit(&self.cursor.position());
        }
        if sel {
            self.selection_changed.emit(&());
        }
        self.update();
    }
    pub fn select_all(&mut self) {
        let changed = !self.cursor.has_selection();
        self.cursor
            .select(&self.document, qtrs_gui::text::SelectionType::Document);
        if changed {
            self.selection_changed.emit(&());
        }
        self.update();
    }
    pub fn has_selection(&self) -> bool {
        self.cursor.has_selection()
    }
    pub fn selected_text(&self) -> String {
        self.cursor.selected_text(&self.document)
    }
    fn snapshot(&mut self) {
        self.undo.push((self.document.clone(), self.cursor.clone()));
        self.redo.clear();
    }
    fn finish_edit(&mut self, old: String, old_pos: usize, had_sel: bool) {
        let now = self.to_plain_text();
        if now != old {
            self.text_changed.emit(&now);
        }
        if old_pos != self.cursor.position() {
            self.cursor_position_changed.emit(&self.cursor.position());
        }
        if had_sel || self.cursor.has_selection() {
            self.selection_changed.emit(&());
        }
        self.update();
    }
    pub fn undo(&mut self) {
        if let Some((d, c)) = self.undo.pop() {
            self.redo.push((self.document.clone(), self.cursor.clone()));
            let old = self.to_plain_text();
            let p = self.cursor.position();
            let s = self.cursor.has_selection();
            self.document = d;
            self.cursor = c;
            self.finish_edit(old, p, s);
        }
    }
    pub fn redo(&mut self) {
        if let Some((d, c)) = self.redo.pop() {
            self.undo.push((self.document.clone(), self.cursor.clone()));
            let old = self.to_plain_text();
            let p = self.cursor.position();
            let s = self.cursor.has_selection();
            self.document = d;
            self.cursor = c;
            self.finish_edit(old, p, s);
        }
    }
    pub fn insert_text(&mut self, text: &str) {
        if self.read_only || text.is_empty() {
            return;
        }
        let old = self.to_plain_text();
        let p = self.cursor.position();
        let s = self.cursor.has_selection();
        self.snapshot();
        self.cursor.insert_text(&mut self.document, text);
        self.finish_edit(old, p, s);
    }
    fn backspace(&mut self) {
        if self.read_only {
            return;
        }
        let old = self.to_plain_text();
        let p = self.cursor.position();
        let s = self.cursor.has_selection();
        if p == 0 && !s {
            return;
        }
        self.snapshot();
        self.cursor.delete_previous_char(&mut self.document);
        self.finish_edit(old, p, s);
    }
    fn move_vertical(&mut self, direction: isize, keep_anchor: bool) {
        let text = self.document.to_plain_text();
        let pos = self.cursor.position();
        let mut starts = Vec::new();
        let mut offset = 0;
        for line in text.split('\n') {
            starts.push((offset, line.graphemes(true).count()));
            offset += line.graphemes(true).count() + 1;
        }
        let row = starts
            .iter()
            .rposition(|(start, _)| *start <= pos)
            .unwrap_or(0);
        let col = pos.saturating_sub(starts[row].0).min(starts[row].1);
        let next =
            (row as isize + direction).clamp(0, starts.len().saturating_sub(1) as isize) as usize;
        self.cursor.set_position(
            starts[next].0 + col.min(starts[next].1),
            if keep_anchor {
                MoveMode::KeepAnchor
            } else {
                MoveMode::MoveAnchor
            },
        );
    }
}
impl QObject for TextEdit {
    leaf_qobject_common!();
    fn event(&mut self, e: &mut Event) -> bool {
        input_common::dispatch_input_event(self, e)
    }
}
impl Widget for TextEdit {
    leaf_widget_common!();
    fn set_enabled(&mut self, v: bool) {
        self.base.enabled = v;
    }
    fn set_window_id(&mut self, id: Option<qtrs_core::object::ObjectId>) {
        self.base.window_id = id;
    }
    fn update(&mut self) {
        self.base.dirty = Some(Rect::new(
            0,
            0,
            self.base.geometry.width,
            self.base.geometry.height,
        ));
        let _ = qtrs_core::event_loop::post_event_to_thread(
            qtrs_core::object::ThreadId::current(),
            self.base.window_id.unwrap_or(self.base.object_data.id),
            Event::new(EventKind::UpdateRequest),
        );
    }
    fn size_hint(&self) -> Size {
        Size::new(240, 120)
    }
    fn paint_event(&mut self, p: &mut Painter) {
        let g = self.base.geometry;
        p.set_brush(Brush::Color(Color::from_rgba8(255, 255, 255, 255)));
        p.set_pen(Some(Pen::new(Color::from_rgba8(130, 130, 130, 255), 1.0)));
        p.draw_rect(RectF::new(0.0, 0.0, g.width as f32, g.height as f32));
        p.set_pen(Some(Pen::new(Color::from_rgba8(20, 20, 20, 255), 1.0)));
        let lines = self.document.to_plain_text();
        for (i, line) in lines.split('\n').enumerate() {
            p.draw_text(PointF::new(5.0, 18.0 + i as f32 * 18.0), line, &self.font);
        }
    }
    fn key_press_event(&mut self, key: u32, mods: u32, _: bool) {
        let ctrl = input_keys::has_ctrl(mods);
        let shift = input_keys::has_shift(mods);
        if ctrl && (key == 0x5A || key == 0x0100_005A) {
            self.undo();
            return;
        }
        if ctrl && (key == 0x59 || key == 0x0100_0059) {
            self.redo();
            return;
        }
        let old = self.cursor.position();
        let had = self.cursor.has_selection();
        if input_keys::is_up(key) || input_keys::is_down(key) {
            self.move_vertical(if input_keys::is_up(key) { -1 } else { 1 }, shift);
            if old != self.cursor.position() {
                self.cursor_position_changed.emit(&self.cursor.position());
            }
            if had != self.cursor.has_selection() {
                self.selection_changed.emit(&());
            }
            self.update();
            return;
        }
        let op = if input_keys::is_left(key) {
            Some(MoveOperation::Left)
        } else if input_keys::is_right(key) {
            Some(MoveOperation::Right)
        } else if input_keys::is_home(key) {
            Some(MoveOperation::StartOfBlock)
        } else if input_keys::is_end(key) {
            Some(MoveOperation::EndOfBlock)
        } else {
            None
        };
        if let Some(op) = op {
            self.cursor.move_position(
                &self.document,
                op,
                if shift {
                    MoveMode::KeepAnchor
                } else {
                    MoveMode::MoveAnchor
                },
                1,
            );
            if old != self.cursor.position() {
                self.cursor_position_changed.emit(&self.cursor.position());
            }
            if had != self.cursor.has_selection() {
                self.selection_changed.emit(&());
            }
            self.update();
        } else if input_keys::is_backspace(key) {
            self.backspace();
        } else if input_keys::is_delete(key) {
            if !self.read_only {
                let before = self.to_plain_text();
                let p = self.cursor.position();
                let s = self.cursor.has_selection();
                if p < self.document.character_count() || s {
                    self.snapshot();
                    self.cursor.delete_char(&mut self.document);
                    self.finish_edit(before, p, s);
                }
            }
        } else if input_keys::is_enter(key) {
            self.insert_text("\n");
        } else if let Some(c) = input_keys::typed_char(key, mods) {
            self.insert_text(&c.to_string());
        }
    }
    fn focus_in_event(&mut self, _: FocusReason) {
        self.base.has_focus = true;
        self.update();
    }
    fn focus_out_event(&mut self, _: FocusReason) {
        self.base.has_focus = false;
        self.update();
    }
}
