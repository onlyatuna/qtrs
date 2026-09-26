use qtrs_core::event::{Event, EventKind, FocusReason};
use qtrs_core::object::{ObjectData, ObjectId, QObject};
use qtrs_core::signal::Signal;
use qtrs_gui::geometry::primitives::{Point, PointF, Rect, RectF, Size};
use qtrs_gui::paint::brush::Brush;
use qtrs_gui::paint::painter::{Painter, Pen};
use qtrs_gui::text::{grapheme_count, Font, FontMetrics, GraphemeIndex, TextPosition, TextRange, UnicodeSegmentation};
use qtrs_platform::clipboard::Clipboard;
use qtrs_gui::tiny_skia::Color;
use crate::focus::FocusPolicy;
use crate::layout::Layout;
use crate::size_policy::{Policy, QSizePolicy};
use crate::widget::{Widget, WidgetBase, WidgetRef, WidgetWeak};

/// Echo mode defining how text in a line edit is presented (`QLineEdit::EchoMode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EchoMode {
    /// Normal display of characters entered.
    #[default]
    Normal,
    /// Displays asterisk or bullet characters instead of the actual characters.
    Password,
    /// Displays nothing at all.
    NoEcho,
    /// Displays character while editing, then masks it.
    PasswordEchoOnEdit,
}

/// Single-line text input control (`QLineEdit`).
pub struct LineEdit {
    base: WidgetBase,
    text: String,
    placeholder_text: String,
    echo_mode: EchoMode,
    cursor_pos: GraphemeIndex,
    selection_anchor: Option<GraphemeIndex>,
    font: Font,
    cursor_visible: bool,
    read_only: bool,
    max_length: usize,
    ime_preedit: String,
    ime_cursor_pos: i32,

    // Colors
    bg_color: Color,
    text_color: Color,
    placeholder_color: Color,
    selection_color: Color,
    border_color: Color,
    focus_border_color: Color,
    border_radius: f32,

    // Signals
    pub text_changed: Signal<String>,
    pub return_pressed: Signal<()>,
    pub editing_finished: Signal<()>,
}

impl LineEdit {
    /// Creates a new empty single-line text input widget.
    pub fn new() -> Self {
        let font = Font::new("Segoe UI", 13.0);
        let mut base = WidgetBase::new();
        base.focus_policy = FocusPolicy::StrongFocus;
        base.size_policy = QSizePolicy::new(Policy::Expanding, Policy::Fixed);

        let metrics = FontMetrics::from_font(&font);
        let h = metrics.height.ceil() as i32 + 12;
        base.geometry = Rect::new(0, 0, 160, h.max(28));

        Self {
            base,
            text: String::new(),
            placeholder_text: String::new(),
            echo_mode: EchoMode::Normal,
            cursor_pos: GraphemeIndex(0),
            selection_anchor: None,
            font,
            cursor_visible: false,
            read_only: false,
            max_length: 32767,
            ime_preedit: String::new(),
            ime_cursor_pos: 0,

            bg_color: Color::from_rgba8(255, 255, 255, 255),
            text_color: Color::from_rgba8(20, 20, 20, 255),
            placeholder_color: Color::from_rgba8(160, 160, 160, 255),
            selection_color: Color::from_rgba8(0, 120, 215, 90),
            border_color: Color::from_rgba8(180, 180, 180, 255),
            focus_border_color: Color::from_rgba8(0, 120, 215, 255),
            border_radius: 4.0,

            text_changed: Signal::new(),
            return_pressed: Signal::new(),
            editing_finished: Signal::new(),
        }
    }

    /// Creates a line edit with initial text.
    pub fn with_text(text: impl Into<String>) -> Self {
        let mut le = Self::new();
        le.set_text(text);
        le
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.cursor_pos = GraphemeIndex(grapheme_count(&self.text));
        self.selection_anchor = None;
        self.update();
        self.text_changed.emit(&self.text);
    }

    pub fn placeholder_text(&self) -> &str {
        &self.placeholder_text
    }

    pub fn set_placeholder_text(&mut self, text: impl Into<String>) {
        self.placeholder_text = text.into();
        self.update();
    }

    pub fn echo_mode(&self) -> EchoMode {
        self.echo_mode
    }

    pub fn set_echo_mode(&mut self, mode: EchoMode) {
        self.echo_mode = mode;
        self.update();
    }

    pub fn cursor_position(&self) -> GraphemeIndex {
        self.cursor_pos
    }

    pub fn cursor_text_position(&self) -> TextPosition {
        TextPosition::from_grapheme(&self.text, self.cursor_pos.0)
    }

    pub fn set_cursor_position(&mut self, pos: impl Into<GraphemeIndex>) {
        let max_pos = grapheme_count(&self.text);
        self.cursor_pos = GraphemeIndex(pos.into().0.min(max_pos));
        self.update();
    }

    pub fn is_read_only(&self) -> bool {
        self.read_only
    }

    pub fn set_read_only(&mut self, read_only: bool) {
        self.read_only = read_only;
        self.update();
    }

    pub fn max_length(&self) -> usize {
        self.max_length
    }

    pub fn set_max_length(&mut self, len: usize) {
        self.max_length = len;
    }

    pub fn has_selected_text(&self) -> bool {
        if let Some(anchor) = self.selection_anchor {
            anchor != self.cursor_pos
        } else {
            false
        }
    }

    pub fn selection_range(&self) -> Option<TextRange> {
        self.selection_anchor.and_then(|anchor| {
            if anchor == self.cursor_pos {
                None
            } else {
                Some(TextRange::from_graphemes(&self.text, anchor.0, self.cursor_pos.0))
            }
        })
    }

    pub fn selected_text(&self) -> String {
        if let Some(range) = self.selection_range() {
            range.slice_str(&self.text).to_string()
        } else {
            String::new()
        }
    }

    pub fn select_all(&mut self) {
        if !self.text.is_empty() {
            self.selection_anchor = Some(GraphemeIndex(0));
            self.cursor_pos = GraphemeIndex(grapheme_count(&self.text));
            self.update();
        }
    }

    pub fn deselect(&mut self) {
        if self.selection_anchor.is_some() {
            self.selection_anchor = None;
            self.update();
        }
    }

    pub fn clear(&mut self) {
        self.set_text("");
    }

    pub fn copy(&self) {
        let sel = self.selected_text();
        if !sel.is_empty() && self.echo_mode == EchoMode::Normal {
            let _ = Clipboard::set_text(&sel);
        }
    }

    pub fn cut(&mut self) {
        if self.read_only {
            return;
        }
        self.copy();
        self.delete_selection();
    }

    pub fn paste(&mut self) {
        if self.read_only {
            return;
        }
        if let Ok(clip_text) = Clipboard::text() {
            if !clip_text.is_empty() {
                self.insert_text(&clip_text);
            }
        }
    }

    fn delete_selection(&mut self) -> bool {
        if let Some(range) = self.selection_range() {
            let start = range.start();
            let end = range.end();
            let mut new_text = String::with_capacity(self.text.len().saturating_sub(range.byte_len()));
            new_text.push_str(&self.text[..start.byte.0]);
            new_text.push_str(&self.text[end.byte.0..]);
            self.text = new_text;
            self.cursor_pos = start.grapheme;
            self.selection_anchor = None;
            self.update();
            self.text_changed.emit(&self.text);
            true
        } else {
            false
        }
    }

    fn insert_text(&mut self, s: &str) {
        if self.read_only {
            return;
        }
        self.delete_selection();
        let cur_count = grapheme_count(&self.text);
        let s_filtered: String = s.chars().filter(|c| *c != '\r' && *c != '\n').collect();
        let s_count = grapheme_count(&s_filtered);
        if cur_count + s_count > self.max_length {
            return;
        }

        let cur_pos = self.cursor_text_position();
        let split_byte = cur_pos.byte.0.min(self.text.len());
        let mut new_text = String::with_capacity(self.text.len() + s_filtered.len());
        new_text.push_str(&self.text[..split_byte]);
        new_text.push_str(&s_filtered);
        new_text.push_str(&self.text[split_byte..]);
        self.text = new_text;
        self.cursor_pos = GraphemeIndex(self.cursor_pos.0 + s_count);
        self.selection_anchor = None;
        self.update();
        self.text_changed.emit(&self.text);
    }

    fn display_text(&self) -> String {
        match self.echo_mode {
            EchoMode::Normal => self.text.clone(),
            EchoMode::Password => "•".repeat(grapheme_count(&self.text)),
            EchoMode::NoEcho => String::new(),
            EchoMode::PasswordEchoOnEdit => "•".repeat(grapheme_count(&self.text)),
        }
    }
}

impl Default for LineEdit {
    fn default() -> Self {
        Self::new()
    }
}

impl QObject for LineEdit {
    fn object_data(&self) -> &ObjectData {
        &self.base.object_data
    }

    fn object_data_mut(&mut self) -> &mut ObjectData {
        &mut self.base.object_data
    }

    fn event(&mut self, event: &mut Event) -> bool {
        match &event.kind {
            EventKind::MouseButtonPress { x, y, button } => {
                self.mouse_press_event(Point::new(*x, *y), *button, 0);
                true
            }
            EventKind::FocusIn { reason } => {
                self.focus_in_event(*reason);
                true
            }
            EventKind::FocusOut { reason } => {
                self.focus_out_event(*reason);
                true
            }
            EventKind::KeyPress { key, modifiers, is_repeat } => {
                self.key_press_event(*key, *modifiers, *is_repeat);
                true
            }
            EventKind::KeyRelease { key, modifiers } => {
                self.key_release_event(*key, *modifiers);
                true
            }
            EventKind::InputMethod { commit_string, preedit_string, cursor_position } => {
                if !commit_string.is_empty() {
                    self.insert_text(commit_string);
                    self.ime_preedit.clear();
                } else {
                    self.ime_preedit = preedit_string.clone();
                    self.ime_cursor_pos = *cursor_position;
                }
                self.update();
                true
            }
            _ => false,
        }
    }

    fn as_qobject_any(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }

    fn as_qobject_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
}

impl Widget for LineEdit {
    fn id(&self) -> ObjectId {
        self.base.object_data.id
    }

    fn geometry(&self) -> Rect {
        self.base.geometry
    }

    fn set_geometry(&mut self, rect: Rect) {
        if self.base.geometry != rect {
            self.base.geometry = rect;
            self.update();
        }
    }

    fn size_hint(&self) -> Size {
        let metrics = FontMetrics::from_font(&self.font);
        let h = metrics.height.ceil() as i32 + 12;
        Size::new(160, h.max(28))
    }

    fn minimum_size(&self) -> Size {
        Size::new(40, 24)
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
        self.base.enabled = enabled;
        self.update();
    }

    fn update(&mut self) {
        self.base.dirty = Some(self.base.geometry);
        let target_receiver = self.base.window_id.unwrap_or(self.base.object_data.id);
        let _ = qtrs_core::event_loop::post_event_to_thread(
            qtrs_core::object::ThreadId::current(),
            target_receiver,
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

    fn focus_policy(&self) -> FocusPolicy {
        self.base.focus_policy
    }

    fn set_focus_policy(&mut self, policy: FocusPolicy) {
        self.base.focus_policy = policy;
    }

    fn has_focus(&self) -> bool {
        self.base.has_focus
    }

    fn set_has_focus(&mut self, focus: bool) {
        self.base.has_focus = focus;
        self.cursor_visible = focus;
    }

    fn size_policy(&self) -> QSizePolicy {
        self.base.size_policy
    }

    fn set_size_policy(&mut self, policy: QSizePolicy) {
        self.base.size_policy = policy;
    }

    fn focus_in_event(&mut self, _reason: FocusReason) {
        self.cursor_visible = true;
        self.update();
    }

    fn focus_out_event(&mut self, _reason: FocusReason) {
        self.cursor_visible = false;
        self.deselect();
        self.editing_finished.emit(&());
        self.update();
    }

    fn mouse_press_event(&mut self, pos: Point, button: u32, _modifiers: u32) {
        if !self.base.enabled || button != 1 {
            return;
        }

        // Calculate cursor position from click X
        let pad_x = 8.0f32;
        let click_x = (pos.x as f32 - pad_x).max(0.0);
        let disp = self.display_text();
        let metrics = FontMetrics::from_font(&self.font);

        let mut closest_pos = 0;
        let mut cur_accum = 0.0f32;

        for (idx, g) in disp.graphemes(true).enumerate() {
            let g_w = metrics.horizontal_advance(g, &self.font);
            if click_x < cur_accum + g_w / 2.0 {
                break;
            }
            cur_accum += g_w;
            closest_pos = idx + 1;
        }

        self.cursor_pos = GraphemeIndex(closest_pos);
        self.selection_anchor = None;
        self.update();
    }

    fn key_press_event(&mut self, key: u32, modifiers: u32, _is_repeat: bool) {
        if !self.base.enabled {
            return;
        }

        let is_ctrl = (modifiers & 0x04000000 != 0) || (modifiers & 2 != 0);
        let is_shift = (modifiers & 0x02000000 != 0) || (modifiers & 1 != 0);

        // Ctrl Shortcuts
        if is_ctrl {
            match key {
                0x41 | 0x61 => {
                    self.select_all();
                    return;
                }
                0x43 | 0x63 => {
                    self.copy();
                    return;
                }
                0x58 | 0x78 => {
                    self.cut();
                    return;
                }
                0x56 | 0x76 => {
                    self.paste();
                    return;
                }
                _ => {}
            }
        }

        // Navigation and editing keys
        match key {
            0x25 | 0x01000012 => {
                if is_shift {
                    if self.selection_anchor.is_none() {
                        self.selection_anchor = Some(self.cursor_pos);
                    }
                } else {
                    self.selection_anchor = None;
                }
                if self.cursor_pos.0 > 0 {
                    self.cursor_pos = GraphemeIndex(self.cursor_pos.0 - 1);
                    self.update();
                }
            }
            // Right Arrow
            0x27 | 0x01000014 => {
                if is_shift {
                    if self.selection_anchor.is_none() {
                        self.selection_anchor = Some(self.cursor_pos);
                    }
                } else {
                    self.selection_anchor = None;
                }
                let total = grapheme_count(&self.text);
                if self.cursor_pos.0 < total {
                    self.cursor_pos = GraphemeIndex(self.cursor_pos.0 + 1);
                    self.update();
                }
            }
            // Home
            0x24 | 0x01000010 => {
                if is_shift {
                    if self.selection_anchor.is_none() {
                        self.selection_anchor = Some(self.cursor_pos);
                    }
                } else {
                    self.selection_anchor = None;
                }
                self.cursor_pos = GraphemeIndex(0);
                self.update();
            }
            // End
            0x23 | 0x01000011 => {
                if is_shift {
                    if self.selection_anchor.is_none() {
                        self.selection_anchor = Some(self.cursor_pos);
                    }
                } else {
                    self.selection_anchor = None;
                }
                self.cursor_pos = GraphemeIndex(grapheme_count(&self.text));
                self.update();
            }
            // Backspace
            0x08 | 0x01000003 => {
                if !self.read_only {
                    if !self.delete_selection() && self.cursor_pos.0 > 0 {
                        let cur_pos = self.cursor_text_position();
                        let prev_pos = cur_pos.prev_grapheme(&self.text);
                        let mut new_text = String::new();
                        new_text.push_str(&self.text[..prev_pos.byte.0]);
                        new_text.push_str(&self.text[cur_pos.byte.0..]);
                        self.text = new_text;
                        self.cursor_pos = prev_pos.grapheme;
                        self.update();
                        self.text_changed.emit(&self.text);
                    }
                }
            }
            // Delete
            0x2E | 0x01000007 => {
                if !self.read_only {
                    if !self.delete_selection() {
                        let cur_pos = self.cursor_text_position();
                        if !cur_pos.is_at_end(&self.text) {
                            let next_pos = cur_pos.next_grapheme(&self.text);
                            let mut new_text = String::new();
                            new_text.push_str(&self.text[..cur_pos.byte.0]);
                            new_text.push_str(&self.text[next_pos.byte.0..]);
                            self.text = new_text;
                            self.update();
                            self.text_changed.emit(&self.text);
                        }
                    }
                }
            }
            // Return / Enter
            0x0D | 0x01000004 | 0x01000005 => {
                self.return_pressed.emit(&());
                self.editing_finished.emit(&());
            }
            // Printable text characters
            c if !is_ctrl && (32..=126).contains(&c) => {
                if let Some(ch) = char::from_u32(c) {
                    self.insert_text(&ch.to_string());
                }
            }
            _ => {}
        }
    }

    fn paint_event(&mut self, painter: &mut Painter) {
        let geom = self.base.geometry;
        let rect_f = RectF::new(0.0, 0.0, geom.width as f32, geom.height as f32);

        // 1. Draw input box background
        painter.set_brush(Brush::Color(self.bg_color));
        let border_c = if self.has_focus() {
            self.focus_border_color
        } else {
            self.border_color
        };
        painter.set_pen(Pen::new(border_c, if self.has_focus() { 1.5 } else { 1.0 }));
        painter.draw_rounded_rect(rect_f, self.border_radius, self.border_radius);

        let pad_x = 8.0f32;
        let metrics = FontMetrics::from_font(&self.font);
        let baseline_y = ((geom.height as f32 - metrics.height) / 2.0).max(0.0) + metrics.ascent;

        let disp = self.display_text();

        // 2. Draw placeholder text if empty and not focused
        if disp.is_empty() && self.ime_preedit.is_empty() {
            if !self.placeholder_text.is_empty() {
                painter.set_pen(Pen::new(self.placeholder_color, 1.0));
                painter.draw_text(PointF::new(pad_x, baseline_y), &self.placeholder_text, &self.font);
            }
        } else {
            // 3. Draw selection background if active
            if let Some(range) = self.selection_range() {
                let start_pos = range.start();
                let prefix = &disp[..start_pos.byte.0.min(disp.len())];
                let sel_sub = range.slice_str(&disp);
                let sel_x = pad_x + metrics.horizontal_advance(prefix, &self.font);
                let sel_w = metrics.horizontal_advance(sel_sub, &self.font);
                let sel_h = metrics.height;
                let sel_y = (geom.height as f32 - sel_h) / 2.0;

                painter.set_brush(Brush::Color(self.selection_color));
                painter.set_pen(None);
                painter.draw_rect(RectF::new(sel_x, sel_y, sel_w, sel_h));
            }

            // 4. Draw text
            if !disp.is_empty() {
                painter.set_pen(Pen::new(self.text_color, 1.0));
                painter.draw_text(PointF::new(pad_x, baseline_y), &disp, &self.font);
            }

            // 5. Draw IME preedit text
            if !self.ime_preedit.is_empty() {
                let cur_pos = self.cursor_text_position();
                let prefix = &disp[..cur_pos.byte.0.min(disp.len())];
                let preedit_x = pad_x + metrics.horizontal_advance(prefix, &self.font);
                let preedit_w = metrics.horizontal_advance(&self.ime_preedit, &self.font);

                painter.set_pen(Pen::new(Color::from_rgba8(0, 100, 200, 255), 1.0));
                painter.draw_text(PointF::new(preedit_x, baseline_y), &self.ime_preedit, &self.font);

                // Dotted/dashed underline for composition
                painter.set_pen(Pen::new(Color::from_rgba8(0, 100, 200, 200), 1.5));
                painter.draw_line(
                    PointF::new(preedit_x, baseline_y + 2.0),
                    PointF::new(preedit_x + preedit_w, baseline_y + 2.0),
                );
            }
        }

        // 6. Draw blinking cursor line
        if self.has_focus() && self.cursor_visible {
            let cur_pos = self.cursor_text_position();
            let prefix = &disp[..cur_pos.byte.0.min(disp.len())];
            let cursor_x = pad_x + metrics.horizontal_advance(prefix, &self.font);
            let cur_h = metrics.height.min(geom.height as f32 - 8.0);
            let cur_y = (geom.height as f32 - cur_h) / 2.0;

            painter.set_pen(Pen::new(self.text_color, 1.2));
            painter.draw_line(
                PointF::new(cursor_x, cur_y),
                PointF::new(cursor_x, cur_y + cur_h),
            );
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
