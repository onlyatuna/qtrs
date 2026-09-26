use crate::{
    focus::FocusPolicy,
    input_common,
    layout::Layout,
    text_edit::TextEdit,
    widget::{Widget, WidgetRef, WidgetWeak},
};
use qtrs_core::event::Event;
use qtrs_core::object::{ObjectData, ObjectId, QObject};
use qtrs_core::signal::Signal;
use qtrs_gui::geometry::primitives::{Rect, Size};
use qtrs_gui::paint::painter::Painter;

/// Plain-text multiline editor with an optional paragraph limit.
pub struct PlainTextEdit {
    inner: TextEdit,
    maximum_block_count: usize,
}
pub type QPlainTextEdit = PlainTextEdit;
impl Default for PlainTextEdit {
    fn default() -> Self {
        Self::new()
    }
}
impl PlainTextEdit {
    pub fn new() -> Self {
        Self {
            inner: TextEdit::new(),
            maximum_block_count: 0,
        }
    }
    pub fn with_text(t: impl Into<String>) -> Self {
        let mut s = Self::new();
        s.set_plain_text(t);
        s
    }
    pub fn to_plain_text(&self) -> String {
        self.inner.to_plain_text()
    }
    pub fn plain_text(&self) -> String {
        self.to_plain_text()
    }
    pub fn text_changed(&self) -> &Signal<String> {
        &self.inner.text_changed
    }
    pub fn cursor_position_changed(&self) -> &Signal<usize> {
        &self.inner.cursor_position_changed
    }
    pub fn selection_changed(&self) -> &Signal<()> {
        &self.inner.selection_changed
    }
    pub fn set_plain_text(&mut self, t: impl Into<String>) {
        let t = t.into();
        let cap = self.maximum_block_count;
        let text = if cap == 0 {
            t
        } else {
            let mut v: Vec<_> = t.split('\n').collect();
            if v.len() > cap {
                v.drain(..v.len() - cap);
            }
            v.join("\n")
        };
        self.inner.set_plain_text(text);
    }
    pub fn set_maximum_block_count(&mut self, n: usize) {
        self.maximum_block_count = n;
        self.trim_blocks();
    }
    pub fn maximum_block_count(&self) -> usize {
        self.maximum_block_count
    }
    pub fn append_plain_text(&mut self, line: impl AsRef<str>) {
        let mut s = self.to_plain_text();
        if !s.is_empty() {
            s.push('\n');
        }
        s.push_str(line.as_ref());
        self.set_plain_text(s);
    }
    pub fn append_line(&mut self, line: impl AsRef<str>) {
        self.append_plain_text(line)
    }
    fn trim_blocks(&mut self) {
        if self.maximum_block_count > 0 {
            let text = self.to_plain_text();
            let mut lines: Vec<_> = text.split('\n').collect();
            if lines.len() > self.maximum_block_count {
                lines.drain(..lines.len() - self.maximum_block_count);
                self.inner.set_plain_text(lines.join("\n"));
            }
        }
    }
    pub fn set_read_only(&mut self, v: bool) {
        self.inner.set_read_only(v)
    }
    pub fn is_read_only(&self) -> bool {
        self.inner.is_read_only()
    }
    pub fn set_word_wrap_width(&mut self, w: f32) {
        self.inner.set_word_wrap_width(w)
    }
    pub fn cursor_position(&self) -> usize {
        self.inner.cursor_position()
    }
    pub fn set_cursor_position(&mut self, p: usize) {
        self.inner.set_cursor_position(p)
    }
    pub fn select_all(&mut self) {
        self.inner.select_all()
    }
    pub fn selected_text(&self) -> String {
        self.inner.selected_text()
    }
    pub fn insert_text(&mut self, t: &str) {
        self.inner.insert_text(t);
        self.trim_blocks();
    }
    pub fn undo(&mut self) {
        self.inner.undo();
        self.trim_blocks()
    }
    pub fn redo(&mut self) {
        self.inner.redo();
        self.trim_blocks()
    }
}
impl QObject for PlainTextEdit {
    fn object_data(&self) -> &ObjectData {
        self.inner.object_data()
    }
    fn object_data_mut(&mut self) -> &mut ObjectData {
        self.inner.object_data_mut()
    }
    fn as_qobject_any(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }
    fn as_qobject_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
    fn event(&mut self, e: &mut Event) -> bool {
        input_common::dispatch_input_event(self, e)
    }
}
impl Widget for PlainTextEdit {
    fn id(&self) -> ObjectId {
        self.inner.id()
    }
    fn geometry(&self) -> Rect {
        self.inner.geometry()
    }
    fn set_geometry(&mut self, r: Rect) {
        self.inner.set_geometry(r)
    }
    fn size_hint(&self) -> Size {
        self.inner.size_hint()
    }
    fn is_visible(&self) -> bool {
        self.inner.is_visible()
    }
    fn set_visible(&mut self, v: bool) {
        self.inner.set_visible(v)
    }
    fn is_enabled(&self) -> bool {
        self.inner.is_enabled()
    }
    fn set_enabled(&mut self, v: bool) {
        self.inner.set_enabled(v)
    }
    fn update(&mut self) {
        self.inner.update()
    }
    fn dirty_rect(&self) -> Option<Rect> {
        self.inner.dirty_rect()
    }
    fn clear_dirty(&mut self) {
        self.inner.clear_dirty()
    }
    fn layout(&self) -> Option<&dyn Layout> {
        None
    }
    fn layout_mut(&mut self) -> Option<&mut Box<dyn Layout>> {
        None
    }
    fn set_layout(&mut self, _: Box<dyn Layout>) {}
    fn parent_widget(&self) -> Option<WidgetWeak> {
        self.inner.parent_widget()
    }
    fn set_parent_widget(&mut self, p: Option<WidgetWeak>) {
        self.inner.set_parent_widget(p)
    }
    fn window_id(&self) -> Option<ObjectId> {
        self.inner.window_id()
    }
    fn set_window_id(&mut self, id: Option<ObjectId>) {
        self.inner.set_window_id(id)
    }
    fn children(&self) -> Vec<WidgetRef> {
        vec![]
    }
    fn add_child(&mut self, _: WidgetRef) {}
    fn remove_child(&mut self, _: ObjectId) {}
    fn paint_event(&mut self, p: &mut Painter) {
        self.inner.paint_event(p)
    }
    fn key_press_event(&mut self, k: u32, m: u32, r: bool) {
        self.inner.key_press_event(k, m, r);
        self.trim_blocks()
    }
    fn key_release_event(&mut self, k: u32, m: u32) {
        self.inner.key_release_event(k, m)
    }
    fn focus_policy(&self) -> FocusPolicy {
        self.inner.focus_policy()
    }
    fn set_focus_policy(&mut self, p: FocusPolicy) {
        self.inner.set_focus_policy(p)
    }
    fn has_focus(&self) -> bool {
        self.inner.has_focus()
    }
    fn set_has_focus(&mut self, v: bool) {
        self.inner.set_has_focus(v)
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
