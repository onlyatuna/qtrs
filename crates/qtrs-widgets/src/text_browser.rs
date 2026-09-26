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

/// Read-only rich text viewer with hyperlink activation and source history.
pub struct TextBrowser {
    inner: TextEdit,
    source: String,
    history: Vec<String>,
    history_index: usize,
    pub anchor_clicked: Signal<String>,
}
pub type QTextBrowser = TextBrowser;
impl Default for TextBrowser {
    fn default() -> Self {
        Self::new()
    }
}
impl TextBrowser {
    pub fn new() -> Self {
        let mut inner = TextEdit::new();
        inner.set_read_only(true);
        Self {
            inner,
            source: String::new(),
            history: Vec::new(),
            history_index: 0,
            anchor_clicked: Signal::new(),
        }
    }
    pub fn set_html(&mut self, h: &str) {
        self.inner.set_html(h);
    }
    pub fn to_plain_text(&self) -> String {
        self.inner.to_plain_text()
    }
    pub fn source(&self) -> &str {
        &self.source
    }
    pub fn set_source(&mut self, s: impl Into<String>) {
        let s = s.into();
        if self.source == s {
            return;
        }
        self.source = s.clone();
        self.history.truncate(self.history_index);
        self.history.push(s.clone());
        self.history_index = self.history.len();
        if s.trim_start().starts_with('<') {
            self.inner.set_html(&s)
        } else {
            self.inner.set_plain_text(s)
        }
    }
    pub fn backward(&mut self) -> bool {
        if self.history_index <= 1 {
            return false;
        }
        self.history_index -= 1;
        self.load_history();
        true
    }
    pub fn forward(&mut self) -> bool {
        if self.history_index >= self.history.len() {
            return false;
        }
        self.history_index += 1;
        self.load_history();
        true
    }
    fn load_history(&mut self) {
        if let Some(s) = self.history.get(self.history_index - 1).cloned() {
            self.source = s.clone();
            if s.trim_start().starts_with('<') {
                self.inner.set_html(&s)
            } else {
                self.inner.set_plain_text(s)
            }
        }
    }
    pub fn history(&self) -> &[String] {
        &self.history
    }
    pub fn activate_link(&mut self, href: impl Into<String>) {
        let h = href.into();
        self.anchor_clicked.emit(&h);
        if h.starts_with('#') {
            return;
        }
        self.set_source(h);
    }
    pub fn anchor_at(&self, pos: usize) -> Option<&str> {
        self.inner.document().anchor_at(pos)
    }
    pub fn activate_link_at(&mut self, pos: usize) -> bool {
        if let Some(href) = self.anchor_at(pos).map(str::to_owned) {
            self.activate_link(href);
            true
        } else {
            false
        }
    }
}
impl QObject for TextBrowser {
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
impl Widget for TextBrowser {
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
        if k == 0x25 {
            self.backward();
        } else if k == 0x27 {
            self.forward();
        } else {
            self.inner.key_press_event(k, m, r)
        }
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
