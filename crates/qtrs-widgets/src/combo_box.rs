//! Drop-down selection widget (`QComboBox`).
//!
//! Items carry display text plus arbitrary user data ([`Variant`]). The popup
//! list is rendered directly below the combo box; while it is open the owner
//! should route mouse input to the combo box (e.g. via
//! `PopupManager::set_mouse_grabber`) so clicks on the list — which lie outside
//! the widget geometry — reach [`ComboBox::mouse_press_event`] in local
//! coordinates. Editable combo boxes embed a [`LineEdit`] and support the
//! `InsertPolicy` rules from `qcombobox.cpp`.

use std::time::{Duration, Instant};

use qtrs_core::event::{Event, EventKind, FocusReason};
use qtrs_core::object::{ObjectId, QObject};
use qtrs_core::signal::Signal;
use qtrs_core::variant::Variant;
use qtrs_gui::geometry::primitives::{Point, PointF, Rect, RectF, Size};
use qtrs_gui::paint::brush::Brush;
use qtrs_gui::paint::painter::{Painter, Pen};
use qtrs_gui::text::{Font, FontMetrics};
use qtrs_gui::tiny_skia::Color;

use crate::focus::FocusPolicy;
use crate::input_common;
use crate::input_keys;
use crate::line_edit::LineEdit;
use crate::size_policy::{Policy, QSizePolicy};
use crate::widget::{Widget, WidgetBase};

/// Item matching rules for [`ComboBox::find_text`] (`Qt::MatchFlags`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MatchFlags(pub u32);

impl MatchFlags {
    /// Whole-string equality; always case-sensitive (`Qt::MatchExactly`).
    pub const EXACTLY: MatchFlags = MatchFlags(0);
    pub const CONTAINS: MatchFlags = MatchFlags(1);
    pub const STARTS_WITH: MatchFlags = MatchFlags(2);
    pub const ENDS_WITH: MatchFlags = MatchFlags(3);
    /// Whole-string comparison that honours `CASE_SENSITIVE` (`Qt::MatchFixedString`).
    pub const FIXED_STRING: MatchFlags = MatchFlags(8);
    pub const CASE_SENSITIVE: MatchFlags = MatchFlags(16);
    const TYPE_MASK: u32 = 0x0F;

    pub fn contains(self, other: MatchFlags) -> bool {
        self.0 & other.0 == other.0
    }

    /// Returns `true` if `candidate` matches `needle` under these flags.
    pub fn matches(self, candidate: &str, needle: &str) -> bool {
        let kind = self.0 & Self::TYPE_MASK;
        if kind == Self::EXACTLY.0 {
            return candidate == needle;
        }
        let case_sensitive = self.contains(Self::CASE_SENSITIVE);
        let (c, n) = if case_sensitive {
            (candidate.to_string(), needle.to_string())
        } else {
            (candidate.to_lowercase(), needle.to_lowercase())
        };
        match kind {
            1 => c.contains(&n),
            2 => c.starts_with(&n),
            3 => c.ends_with(&n),
            _ => c == n,
        }
    }
}

impl Default for MatchFlags {
    /// `Qt::MatchExactly | Qt::MatchCaseSensitive`, the `findText` default.
    fn default() -> Self {
        MatchFlags(Self::EXACTLY.0 | Self::CASE_SENSITIVE.0)
    }
}

impl std::ops::BitOr for MatchFlags {
    type Output = MatchFlags;

    fn bitor(self, rhs: MatchFlags) -> MatchFlags {
        MatchFlags(self.0 | rhs.0)
    }
}

/// Where text entered in an editable combo box is inserted (`QComboBox::InsertPolicy`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InsertPolicy {
    NoInsert,
    InsertAtTop,
    InsertAtCurrent,
    #[default]
    InsertAtBottom,
    InsertAfterCurrent,
    InsertBeforeCurrent,
    InsertAlphabetically,
}

#[derive(Debug, Clone)]
struct ComboItem {
    text: String,
    data: Variant,
    enabled: bool,
}

const ARROW_WIDTH: i32 = 20;
const TEXT_PAD: f32 = 6.0;
const KEYBOARD_INPUT_INTERVAL: Duration = Duration::from_millis(400);

/// Combined button and popup list (`QComboBox`).
pub struct ComboBox {
    base: WidgetBase,
    items: Vec<ComboItem>,
    current: i32,
    editable: Option<LineEdit>,
    insert_policy: InsertPolicy,
    duplicates_enabled: bool,
    max_visible_items: i32,
    max_count: i32,
    placeholder_text: String,
    popup_visible: bool,
    highlight_row: i32,
    popup_scroll: i32,
    search_text: String,
    last_search: Option<Instant>,
    font: Font,

    bg_color: Color,
    text_color: Color,
    placeholder_color: Color,
    disabled_text_color: Color,
    border_color: Color,
    focus_border_color: Color,
    highlight_color: Color,
    highlight_text_color: Color,
    arrow_color: Color,

    /// Emitted when the current index changes, programmatically or by the user.
    pub current_index_changed: Signal<i32>,
    /// Emitted when the current text changes.
    pub current_text_changed: Signal<String>,
    /// Emitted when the user chooses an item (even if it was already current).
    pub activated: Signal<i32>,
    /// Text of the item passed to `activated` (`textActivated`).
    pub text_activated: Signal<String>,
    /// Emitted when the popup highlight moves to an item.
    pub highlighted: Signal<i32>,
    /// Text of the item passed to `highlighted` (`textHighlighted`).
    pub text_highlighted: Signal<String>,
    /// Emitted when the line edit text of an editable combo box changes.
    pub edit_text_changed: Signal<String>,
}

pub type QComboBox = ComboBox;

impl ComboBox {
    pub fn new() -> Self {
        let font = Font::new("Segoe UI", 13.0);
        let metrics = FontMetrics::from_font(&font);
        let h = (metrics.height.ceil() as i32 + 10).max(26);
        let mut base = WidgetBase::with_geometry(Rect::new(0, 0, 140, h));
        base.focus_policy = FocusPolicy::WheelFocus;
        base.size_policy = QSizePolicy::new(Policy::Preferred, Policy::Fixed);
        Self {
            base,
            items: Vec::new(),
            current: -1,
            editable: None,
            insert_policy: InsertPolicy::InsertAtBottom,
            duplicates_enabled: false,
            max_visible_items: 10,
            max_count: i32::MAX,
            placeholder_text: String::new(),
            popup_visible: false,
            highlight_row: -1,
            popup_scroll: 0,
            search_text: String::new(),
            last_search: None,
            font,
            bg_color: Color::from_rgba8(255, 255, 255, 255),
            text_color: Color::from_rgba8(20, 20, 20, 255),
            placeholder_color: Color::from_rgba8(140, 140, 140, 255),
            disabled_text_color: Color::from_rgba8(160, 160, 160, 255),
            border_color: Color::from_rgba8(160, 160, 160, 255),
            focus_border_color: Color::from_rgba8(0, 120, 215, 255),
            highlight_color: Color::from_rgba8(0, 120, 215, 255),
            highlight_text_color: Color::from_rgba8(255, 255, 255, 255),
            arrow_color: Color::from_rgba8(70, 70, 70, 255),
            current_index_changed: Signal::new(),
            current_text_changed: Signal::new(),
            activated: Signal::new(),
            text_activated: Signal::new(),
            highlighted: Signal::new(),
            text_highlighted: Signal::new(),
            edit_text_changed: Signal::new(),
        }
    }

    // ----- Items ------------------------------------------------------------------------------

    pub fn count(&self) -> i32 {
        self.items.len() as i32
    }

    pub fn add_item(&mut self, text: impl Into<String>) {
        self.insert_item(self.count(), text, Variant::Invalid);
    }

    pub fn add_item_with_data(&mut self, text: impl Into<String>, data: Variant) {
        self.insert_item(self.count(), text, data);
    }

    pub fn add_items<I, S>(&mut self, texts: I)
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.insert_items(self.count(), texts);
    }

    /// Inserts an item at `index` (clamped to `0..=count`). Respects `max_count`.
    pub fn insert_item(&mut self, index: i32, text: impl Into<String>, data: Variant) {
        if self.count() >= self.max_count {
            return;
        }
        let index = index.clamp(0, self.count());
        self.items.insert(
            index as usize,
            ComboItem {
                text: text.into(),
                data,
                enabled: true,
            },
        );
        self.after_rows_inserted(index, 1);
    }

    pub fn insert_items<I, S>(&mut self, index: i32, texts: I)
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let start = index.clamp(0, self.count());
        let mut inserted = 0;
        for text in texts {
            if self.count() >= self.max_count {
                break;
            }
            self.items.insert(
                (start + inserted) as usize,
                ComboItem {
                    text: text.into(),
                    data: Variant::Invalid,
                    enabled: true,
                },
            );
            inserted += 1;
        }
        if inserted > 0 {
            self.after_rows_inserted(start, inserted);
        }
    }

    /// Removes the item at `index`; the current item moves to the nearest remaining one.
    pub fn remove_item(&mut self, index: i32) {
        if index < 0 || index >= self.count() {
            return;
        }
        let old_current = self.current;
        let old_text = self.current_text();
        self.items.remove(index as usize);
        let new_current = if self.items.is_empty() {
            -1
        } else if index < old_current {
            old_current - 1
        } else if index == old_current {
            old_current.min(self.count() - 1)
        } else {
            old_current
        };
        if self.highlight_row >= self.count() {
            self.highlight_row = self.count() - 1;
        }
        if self.items.is_empty() {
            self.hide_popup();
        }
        self.apply_current(new_current, old_current, old_text, index == old_current);
    }

    /// Removes every item; the current index becomes -1.
    pub fn clear(&mut self) {
        let old_current = self.current;
        let old_text = self.current_text();
        self.items.clear();
        self.hide_popup();
        self.highlight_row = -1;
        self.apply_current(-1, old_current, old_text, true);
    }

    /// Item text, or an empty string if `index` is out of range.
    pub fn item_text(&self, index: i32) -> String {
        self.item(index)
            .map(|it| it.text.clone())
            .unwrap_or_default()
    }

    pub fn set_item_text(&mut self, index: i32, text: impl Into<String>) {
        let text = text.into();
        let Some(item) = self.item_mut(index) else {
            return;
        };
        if item.text == text {
            return;
        }
        item.text = text.clone();
        if index == self.current {
            if let Some(le) = self.editable.as_mut() {
                le.set_text(text.clone());
                self.edit_text_changed.emit(&text);
            }
            self.current_text_changed.emit(&text);
        }
        self.update();
    }

    /// User data of the item, or `Variant::Invalid` if `index` is out of range.
    pub fn item_data(&self, index: i32) -> Variant {
        self.item(index)
            .map(|it| it.data.clone())
            .unwrap_or_default()
    }

    pub fn set_item_data(&mut self, index: i32, data: Variant) {
        if let Some(item) = self.item_mut(index) {
            item.data = data;
        }
    }

    pub fn is_item_enabled(&self, index: i32) -> bool {
        self.item(index).is_some_and(|it| it.enabled)
    }

    /// Disabled items are skipped by keyboard/wheel navigation and cannot be chosen in the popup.
    pub fn set_item_enabled(&mut self, index: i32, enabled: bool) {
        if let Some(item) = self.item_mut(index) {
            item.enabled = enabled;
            self.update();
        }
    }

    /// Index of the first item whose text matches, or -1 (`findText`).
    pub fn find_text(&self, text: &str, flags: MatchFlags) -> i32 {
        self.items
            .iter()
            .position(|it| flags.matches(&it.text, text))
            .map_or(-1, |i| i as i32)
    }

    /// Index of the first item whose data equals `data`, or -1 (`findData`).
    pub fn find_data(&self, data: &Variant) -> i32 {
        self.items
            .iter()
            .position(|it| &it.data == data)
            .map_or(-1, |i| i as i32)
    }

    // ----- Current item -----------------------------------------------------------------------

    pub fn current_index(&self) -> i32 {
        self.current
    }

    /// Sets the current item; out-of-range indices clear the selection (-1).
    pub fn set_current_index(&mut self, index: i32) {
        let index = if index >= 0 && index < self.count() {
            index
        } else {
            -1
        };
        let old_current = self.current;
        let old_text = self.current_text();
        self.apply_current(index, old_current, old_text, false);
    }

    /// Current text: the line edit text when editable, otherwise the current item's text.
    pub fn current_text(&self) -> String {
        match &self.editable {
            Some(le) => le.text().to_string(),
            None => self.item_text(self.current),
        }
    }

    /// Editable: sets the edit text. Otherwise selects the first exactly matching item, if any.
    pub fn set_current_text(&mut self, text: &str) {
        if self.editable.is_some() {
            self.set_edit_text(text);
        } else {
            let i = self.find_text(text, MatchFlags::default());
            if i >= 0 {
                self.set_current_index(i);
            }
        }
    }

    pub fn current_data(&self) -> Variant {
        self.item_data(self.current)
    }

    // ----- Editing ----------------------------------------------------------------------------

    pub fn is_editable(&self) -> bool {
        self.editable.is_some()
    }

    /// Switches between a read-only button and an embedded line edit.
    pub fn set_editable(&mut self, editable: bool) {
        if editable == self.editable.is_some() {
            return;
        }
        if editable {
            let mut le = LineEdit::new();
            le.set_window_id(self.base.window_id);
            le.set_text(self.item_text(self.current));
            le.set_placeholder_text(self.placeholder_text.clone());
            self.editable = Some(le);
            self.layout_line_edit();
        } else {
            let before = self.current_text();
            self.editable = None;
            let after = self.current_text();
            if before != after {
                self.current_text_changed.emit(&after);
            }
        }
        self.update();
    }

    pub fn line_edit(&self) -> Option<&LineEdit> {
        self.editable.as_ref()
    }

    pub fn line_edit_mut(&mut self) -> Option<&mut LineEdit> {
        self.editable.as_mut()
    }

    /// Text in the line edit (empty for non-editable combo boxes).
    pub fn edit_text(&self) -> String {
        self.editable
            .as_ref()
            .map(|le| le.text().to_string())
            .unwrap_or_default()
    }

    pub fn set_edit_text(&mut self, text: &str) {
        let Some(le) = self.editable.as_mut() else {
            return;
        };
        if le.text() == text {
            return;
        }
        le.set_text(text);
        let t = text.to_string();
        self.edit_text_changed.emit(&t);
        self.current_text_changed.emit(&t);
        self.update();
    }

    pub fn insert_policy(&self) -> InsertPolicy {
        self.insert_policy
    }

    pub fn set_insert_policy(&mut self, policy: InsertPolicy) {
        self.insert_policy = policy;
    }

    pub fn duplicates_enabled(&self) -> bool {
        self.duplicates_enabled
    }

    pub fn set_duplicates_enabled(&mut self, enabled: bool) {
        self.duplicates_enabled = enabled;
    }

    pub fn max_visible_items(&self) -> i32 {
        self.max_visible_items
    }

    pub fn set_max_visible_items(&mut self, items: i32) {
        if items >= 0 {
            self.max_visible_items = items;
        }
    }

    pub fn max_count(&self) -> i32 {
        self.max_count
    }

    /// Limits the number of items; surplus items are removed.
    pub fn set_max_count(&mut self, max: i32) {
        if max < 0 {
            return;
        }
        while self.count() > max {
            self.remove_item(self.count() - 1);
        }
        self.max_count = max;
    }

    pub fn placeholder_text(&self) -> &str {
        &self.placeholder_text
    }

    /// Text shown while no item is current.
    pub fn set_placeholder_text(&mut self, text: impl Into<String>) {
        self.placeholder_text = text.into();
        if let Some(le) = self.editable.as_mut() {
            le.set_placeholder_text(self.placeholder_text.clone());
        }
        self.update();
    }

    pub fn font(&self) -> &Font {
        &self.font
    }

    pub fn set_font(&mut self, font: Font) {
        self.font = font;
        self.update();
    }

    // ----- Popup ------------------------------------------------------------------------------

    pub fn is_popup_visible(&self) -> bool {
        self.popup_visible
    }

    /// Opens the popup list (no-op without items), highlighting the current item.
    pub fn show_popup(&mut self) {
        if self.items.is_empty() || self.popup_visible {
            return;
        }
        self.popup_visible = true;
        let start = if self.current >= 0 {
            self.current
        } else {
            self.next_enabled(-1, 1)
        };
        self.popup_scroll = 0;
        self.set_highlighted(start);
        self.update();
    }

    pub fn hide_popup(&mut self) {
        if self.popup_visible {
            // Repaint the area the popup covered before shrinking the dirty rect.
            self.update();
            self.popup_visible = false;
        }
    }

    /// Index highlighted in the open popup (-1 if none).
    pub fn highlighted_index(&self) -> i32 {
        self.highlight_row
    }

    /// Number of rows shown by the popup at once.
    pub fn popup_visible_rows(&self) -> i32 {
        self.count().min(self.max_visible_items.max(1))
    }

    /// Popup list rectangle in widget-local coordinates (directly below the box).
    pub fn popup_rect(&self) -> Rect {
        let g = self.base.geometry;
        Rect::new(
            0,
            g.height,
            g.width,
            self.popup_visible_rows() * self.row_height() + 2,
        )
    }

    /// Item index under a widget-local point inside the popup, or -1.
    pub fn popup_item_at(&self, pos: Point) -> i32 {
        let r = self.popup_rect();
        if !self.popup_visible || !r.contains(pos) {
            return -1;
        }
        let row = (pos.y - r.y - 1) / self.row_height();
        let index = self.popup_scroll + row;
        if row >= 0 && row < self.popup_visible_rows() && index < self.count() {
            index
        } else {
            -1
        }
    }

    // ----- Internals --------------------------------------------------------------------------

    fn item(&self, index: i32) -> Option<&ComboItem> {
        usize::try_from(index).ok().and_then(|i| self.items.get(i))
    }

    fn item_mut(&mut self, index: i32) -> Option<&mut ComboItem> {
        usize::try_from(index)
            .ok()
            .and_then(move |i| self.items.get_mut(i))
    }

    fn row_height(&self) -> i32 {
        let metrics = FontMetrics::from_font(&self.font);
        (metrics.height.ceil() as i32 + 6).max(18)
    }

    fn after_rows_inserted(&mut self, start: i32, n: i32) {
        let old_current = self.current;
        let old_text = self.current_text();
        let new_current = if self.current < 0 {
            // First items inserted into an empty combo box become current.
            if start == 0 && n == self.count() {
                0
            } else {
                -1
            }
        } else if start <= self.current {
            self.current + n
        } else {
            self.current
        };
        if self.popup_visible && self.highlight_row >= start {
            self.highlight_row += n;
        }
        self.apply_current(new_current, old_current, old_text, false);
    }

    /// Moves to `new_current`, syncs the line edit and emits change signals.
    fn apply_current(
        &mut self,
        new_current: i32,
        old_current: i32,
        old_text: String,
        force_text_sync: bool,
    ) {
        self.current = new_current;
        let index_changed = new_current != old_current;
        if index_changed || force_text_sync {
            let text = self.item_text(new_current);
            if let Some(le) = self.editable.as_mut() {
                if le.text() != text {
                    le.set_text(text.clone());
                    self.edit_text_changed.emit(&text);
                }
            }
        }
        if index_changed {
            self.current_index_changed.emit(&new_current);
        }
        let text = self.current_text();
        if text != old_text {
            self.current_text_changed.emit(&text);
        }
        self.update();
    }

    fn emit_activated(&self, index: i32) {
        if index < 0 {
            return;
        }
        self.activated.emit(&index);
        self.text_activated.emit(&self.item_text(index));
    }

    fn set_highlighted(&mut self, index: i32) {
        if index == self.highlight_row || index < 0 || index >= self.count() {
            if index >= 0 {
                self.ensure_highlight_visible();
            }
            return;
        }
        self.highlight_row = index;
        self.ensure_highlight_visible();
        self.emit_highlighted(self.highlight_row);
        self.update();
    }

    fn ensure_highlight_visible(&mut self) {
        let rows = self.popup_visible_rows();
        if self.highlight_row < self.popup_scroll {
            self.popup_scroll = self.highlight_row.max(0);
        } else if self.highlight_row >= self.popup_scroll + rows {
            self.popup_scroll = self.highlight_row - rows + 1;
        }
        self.popup_scroll = self.popup_scroll.clamp(0, (self.count() - rows).max(0));
    }

    /// Next enabled index from `from` moving by `dir` (±1), or -1 if none.
    fn next_enabled(&self, from: i32, dir: i32) -> i32 {
        let mut i = from + dir;
        while i >= 0 && i < self.count() {
            if self.is_item_enabled(i) {
                return i;
            }
            i += dir;
        }
        -1
    }

    /// Picks `index` as the user's choice: becomes current and `activated` fires.
    fn choose(&mut self, index: i32) {
        if index < 0 || index >= self.count() || !self.is_item_enabled(index) {
            return;
        }
        if index != self.current {
            self.set_current_index(index);
        }
        self.emit_activated(index);
    }

    fn popup_key(&mut self, key: u32, modifiers: u32) {
        let page = self.popup_visible_rows().max(1);
        let target = if input_keys::is_escape(key)
            || input_keys::is_f4(key)
            || (input_keys::has_alt(modifiers)
                && (input_keys::is_up(key) || input_keys::is_down(key)))
        {
            self.hide_popup();
            return;
        } else if input_keys::is_enter(key) || key == input_keys::KEY_SPACE {
            let h = self.highlight_row;
            self.hide_popup();
            self.choose(h);
            return;
        } else if input_keys::is_up(key) {
            self.next_enabled(self.highlight_row, -1)
        } else if input_keys::is_down(key) {
            self.next_enabled(self.highlight_row, 1)
        } else if input_keys::is_home(key) {
            self.next_enabled(-1, 1)
        } else if input_keys::is_end(key) {
            self.next_enabled(self.count(), -1)
        } else if input_keys::is_page_up(key) {
            let t = self.next_enabled((self.highlight_row - page + 1).max(0) - 1, 1);
            if t >= 0 && t < self.highlight_row {
                t
            } else {
                self.next_enabled(-1, 1)
            }
        } else if input_keys::is_page_down(key) {
            let t = self.next_enabled(
                (self.highlight_row + page - 1).min(self.count() - 1) + 1,
                -1,
            );
            if t > self.highlight_row {
                t
            } else {
                self.next_enabled(self.count(), -1)
            }
        } else {
            return;
        };
        if target >= 0 {
            self.set_highlighted(target);
        }
    }

    /// Type-ahead selection of the next item starting with the typed text (`keyboardSearch`).
    fn keyboard_search(&mut self, c: char) {
        let now = Instant::now();
        let continuing = self
            .last_search
            .is_some_and(|t| now.duration_since(t) < KEYBOARD_INPUT_INTERVAL);
        self.last_search = Some(now);
        if continuing {
            self.search_text.push(c);
        } else {
            self.search_text.clear();
            self.search_text.push(c);
        }
        let lower = self.search_text.to_lowercase();
        let first = lower.chars().next().unwrap_or(c);
        let repeated = lower.chars().all(|ch| ch == first);
        // Repeating one letter cycles through the items starting with it.
        let (needle, offset) = if repeated {
            (first.to_string(), 1)
        } else {
            (lower, 0)
        };
        let n = self.count();
        let origin = if self.popup_visible {
            self.highlight_row
        } else {
            self.current
        };
        let start = if origin < 0 { 0 } else { origin + offset };
        for k in 0..n {
            let i = (start + k).rem_euclid(n.max(1));
            if self.is_item_enabled(i) && self.item_text(i).to_lowercase().starts_with(&needle) {
                if self.popup_visible {
                    self.set_highlighted(i);
                } else if i != self.current {
                    self.set_current_index(i);
                    self.emit_activated(i);
                }
                return;
            }
        }
    }

    /// Return pressed in the editable line edit (`_q_returnPressed`).
    fn commit_edit_text(&mut self) {
        let text = self.edit_text();
        if text.is_empty() {
            return;
        }
        if self.count() >= self.max_count && self.insert_policy != InsertPolicy::InsertAtCurrent {
            return;
        }
        if !self.duplicates_enabled {
            let existing = self.find_text(&text, MatchFlags::FIXED_STRING);
            if existing >= 0 {
                self.set_current_index(existing);
                self.emit_activated(existing);
                return;
            }
        }
        let mut index = -1;
        match self.insert_policy {
            InsertPolicy::InsertAtTop => index = 0,
            InsertPolicy::InsertAtBottom => index = self.count(),
            InsertPolicy::InsertAtCurrent
            | InsertPolicy::InsertAfterCurrent
            | InsertPolicy::InsertBeforeCurrent => {
                if self.count() == 0 || self.current < 0 {
                    index = 0;
                } else if self.insert_policy == InsertPolicy::InsertAtCurrent {
                    let cur = self.current;
                    self.set_item_text(cur, text.clone());
                } else if self.insert_policy == InsertPolicy::InsertAfterCurrent {
                    index = self.current + 1;
                } else {
                    index = self.current;
                }
            }
            InsertPolicy::InsertAlphabetically => {
                let lower = text.to_lowercase();
                index = self
                    .items
                    .iter()
                    .position(|it| lower < it.text.to_lowercase())
                    .map_or(self.count(), |i| i as i32);
            }
            InsertPolicy::NoInsert => {}
        }
        if index >= 0 {
            self.insert_item(index, text.clone(), Variant::Invalid);
            self.set_current_index(index);
        }
        let cur = self.current;
        self.emit_activated(cur);
    }

    fn layout_line_edit(&mut self) {
        let g = self.base.geometry;
        if let Some(le) = self.editable.as_mut() {
            le.set_geometry(Rect::new(0, 0, (g.width - ARROW_WIDTH).max(0), g.height));
        }
    }

    fn forward_to_line_edit(&mut self, event: &mut Event) {
        self.layout_line_edit();
        let Some(le) = self.editable.as_mut() else {
            return;
        };
        let before = le.text().to_string();
        le.event(event);
        let after = le.text().to_string();
        if before != after {
            self.edit_text_changed.emit(&after);
            self.current_text_changed.emit(&after);
            self.update();
        }
    }

    fn arrow_rect(&self) -> Rect {
        let g = self.base.geometry;
        Rect::new((g.width - ARROW_WIDTH).max(0), 0, ARROW_WIDTH, g.height)
    }

    /// Emits `highlighted`/`text_highlighted` for `index`.
    fn emit_highlighted(&self, index: i32) {
        self.highlighted.emit(&index);
        self.text_highlighted.emit(&self.item_text(index));
    }
}

impl Default for ComboBox {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for ComboBox {
    fn drop(&mut self) {
        // Registry cleanup only touches thread-locals through `try_with`.
        // SAFETY: Drop runs on the registration thread; no callbacks are active at this point.
        unsafe { qtrs_core::object::unregister_qobject(self.base.object_data.id) };
    }
}

impl QObject for ComboBox {
    leaf_qobject_common!();

    fn event(&mut self, event: &mut Event) -> bool {
        if let EventKind::InputMethod { .. } = &event.kind {
            if self.editable.is_some() {
                self.forward_to_line_edit(event);
                return true;
            }
            return false;
        }
        input_common::dispatch_input_event(self, event)
    }
}

impl Widget for ComboBox {
    leaf_widget_common!();

    fn size_hint(&self) -> Size {
        let metrics = FontMetrics::from_font(&self.font);
        let widest = self
            .items
            .iter()
            .map(|it| metrics.horizontal_advance(&it.text, &self.font))
            .fold(
                metrics.horizontal_advance(&self.placeholder_text, &self.font),
                f32::max,
            )
            .max(metrics.horizontal_advance("XXXXXXX", &self.font));
        let h = (metrics.height.ceil() as i32 + 10).max(26);
        Size::new(widest.ceil() as i32 + 2 * TEXT_PAD as i32 + ARROW_WIDTH, h)
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.base.enabled = enabled;
        if !enabled {
            self.hide_popup();
        }
        if let Some(le) = self.editable.as_mut() {
            le.set_enabled(enabled);
        }
        self.update();
    }

    fn update(&mut self) {
        let mut rect = input_common::local_rect(&self.base);
        if self.popup_visible {
            let p = self.popup_rect();
            rect = Rect::new(0, 0, rect.width.max(p.width), p.y + p.height);
        }
        input_common::request_update(&mut self.base, rect);
    }

    fn set_window_id(&mut self, window_id: Option<ObjectId>) {
        self.base.window_id = window_id;
        if let Some(le) = self.editable.as_mut() {
            le.set_window_id(window_id);
        }
    }

    fn set_has_focus(&mut self, focus: bool) {
        self.base.has_focus = focus;
        if let Some(le) = self.editable.as_mut() {
            le.set_has_focus(focus);
        }
        self.update();
    }

    fn focus_in_event(&mut self, reason: FocusReason) {
        if let Some(le) = self.editable.as_mut() {
            le.focus_in_event(reason);
        }
        self.update();
    }

    fn focus_out_event(&mut self, reason: FocusReason) {
        if reason != FocusReason::Popup {
            self.hide_popup();
        }
        if let Some(le) = self.editable.as_mut() {
            le.focus_out_event(reason);
        }
        self.update();
    }

    fn mouse_press_event(&mut self, pos: Point, button: u32, _modifiers: u32) {
        if !self.base.enabled || button != 1 {
            return;
        }
        if self.popup_visible {
            let index = self.popup_item_at(pos);
            if index >= 0 {
                if self.is_item_enabled(index) {
                    self.hide_popup();
                    self.choose(index);
                }
            } else {
                // Click on the box itself or anywhere outside the list closes the popup.
                self.hide_popup();
            }
            return;
        }
        let inside = pos.x >= 0
            && pos.y >= 0
            && pos.x < self.base.geometry.width
            && pos.y < self.base.geometry.height;
        if !inside {
            return;
        }
        if self.editable.is_some() && !self.arrow_rect().contains(pos) {
            let mut ev = Event::new_spontaneous(EventKind::MouseButtonPress {
                x: pos.x,
                y: pos.y,
                button,
            });
            self.forward_to_line_edit(&mut ev);
            return;
        }
        self.show_popup();
    }

    fn mouse_move_event(&mut self, pos: Point) {
        if self.popup_visible {
            let index = self.popup_item_at(pos);
            if index >= 0 && self.is_item_enabled(index) {
                self.set_highlighted(index);
            }
        }
    }

    fn wheel_event(&mut self, _pos: Point, delta_y: i32, _modifiers: u32) {
        if !self.base.enabled || delta_y == 0 {
            return;
        }
        if self.popup_visible {
            let max_scroll = (self.count() - self.popup_visible_rows()).max(0);
            let lines = if delta_y > 0 { -3 } else { 3 };
            self.popup_scroll = (self.popup_scroll + lines).clamp(0, max_scroll);
            self.update();
            return;
        }
        let dir = if delta_y > 0 { -1 } else { 1 };
        let target = self.next_enabled(self.current, dir);
        if target >= 0 && target != self.current {
            self.set_current_index(target);
            self.emit_activated(target);
        }
    }

    fn key_press_event(&mut self, key: u32, modifiers: u32, is_repeat: bool) {
        if !self.base.enabled {
            return;
        }
        if self.popup_visible {
            self.popup_key(key, modifiers);
            return;
        }
        let editable = self.editable.is_some();
        let alt = input_keys::has_alt(modifiers);
        let ctrl = input_keys::has_ctrl(modifiers);
        if (alt && (input_keys::is_up(key) || input_keys::is_down(key)))
            || (input_keys::is_f4(key) && modifiers == 0)
        {
            self.show_popup();
            return;
        }
        if key == input_keys::KEY_SPACE && !editable {
            self.show_popup();
            return;
        }
        let target = if (input_keys::is_up(key) && !ctrl) || input_keys::is_page_up(key) {
            Some(self.next_enabled(self.current, -1))
        } else if (input_keys::is_down(key) && !ctrl) || input_keys::is_page_down(key) {
            Some(self.next_enabled(self.current, 1))
        } else if input_keys::is_home(key) && !editable {
            Some(self.next_enabled(-1, 1))
        } else if input_keys::is_end(key) && !editable {
            Some(self.next_enabled(self.count(), -1))
        } else {
            None
        };
        if let Some(t) = target {
            if t >= 0 && t != self.current {
                self.set_current_index(t);
                self.emit_activated(t);
            }
            return;
        }
        if editable {
            if input_keys::is_enter(key) {
                self.commit_edit_text();
                return;
            }
            let mut ev = Event::new_spontaneous(EventKind::KeyPress {
                key,
                modifiers,
                is_repeat,
            });
            self.forward_to_line_edit(&mut ev);
            return;
        }
        if let Some(c) = input_keys::typed_char(key, modifiers) {
            self.keyboard_search(c);
        }
    }

    fn paint_event(&mut self, painter: &mut Painter) {
        let g = self.base.geometry;
        let (w, h) = (g.width as f32, g.height as f32);
        let focused = self.base.has_focus;
        let metrics = FontMetrics::from_font(&self.font);

        // 1. Box frame
        painter.set_brush(Brush::Color(self.bg_color));
        painter.set_pen(Pen::new(
            if focused || self.popup_visible {
                self.focus_border_color
            } else {
                self.border_color
            },
            if focused { 1.5 } else { 1.0 },
        ));
        painter.draw_rounded_rect(
            RectF::new(0.5, 0.5, (w - 1.0).max(0.0), (h - 1.0).max(0.0)),
            3.0,
            3.0,
        );

        // 2. Content: embedded line edit, current text or placeholder
        self.layout_line_edit();
        if let Some(le) = self.editable.as_mut() {
            le.paint_event(painter);
        } else {
            let baseline = ((h - metrics.height) / 2.0).max(0.0) + metrics.ascent;
            if self.current >= 0 {
                let color = if self.base.enabled {
                    self.text_color
                } else {
                    self.disabled_text_color
                };
                painter.set_pen(Pen::new(color, 1.0));
                let text = self.item_text(self.current);
                painter.draw_text(PointF::new(TEXT_PAD, baseline), &text, &self.font);
            } else if !self.placeholder_text.is_empty() {
                painter.set_pen(Pen::new(self.placeholder_color, 1.0));
                painter.draw_text(
                    PointF::new(TEXT_PAD, baseline),
                    &self.placeholder_text,
                    &self.font,
                );
            }
        }

        // 3. Drop-down arrow
        let arrow = self.arrow_rect();
        let cx = arrow.x as f32 + arrow.width as f32 / 2.0;
        let cy = h / 2.0;
        painter.set_pen(Pen::new(
            if self.base.enabled {
                self.arrow_color
            } else {
                self.disabled_text_color
            },
            1.5,
        ));
        painter.draw_polyline(&[
            PointF::new(cx - 4.0, cy - 2.0),
            PointF::new(cx, cy + 2.0),
            PointF::new(cx + 4.0, cy - 2.0),
        ]);

        // 4. Popup list
        if self.popup_visible {
            let p = self.popup_rect();
            let row_h = self.row_height();
            painter.set_brush(Brush::Color(self.bg_color));
            painter.set_pen(Pen::new(self.border_color, 1.0));
            painter.draw_rect(RectF::new(
                0.5,
                p.y as f32 + 0.5,
                p.width as f32 - 1.0,
                p.height as f32 - 1.0,
            ));
            for row in 0..self.popup_visible_rows() {
                let index = self.popup_scroll + row;
                if index >= self.count() {
                    break;
                }
                let y = (p.y + 1 + row * row_h) as f32;
                let enabled = self.is_item_enabled(index);
                let color = if index == self.highlight_row && enabled {
                    painter.set_pen(None);
                    painter.set_brush(Brush::Color(self.highlight_color));
                    painter.draw_rect(RectF::new(1.0, y, p.width as f32 - 2.0, row_h as f32));
                    self.highlight_text_color
                } else if enabled {
                    self.text_color
                } else {
                    self.disabled_text_color
                };
                let baseline =
                    y + ((row_h as f32 - metrics.height) / 2.0).max(0.0) + metrics.ascent;
                painter.set_pen(Pen::new(color, 1.0));
                let text = self.item_text(index);
                painter.draw_text(PointF::new(TEXT_PAD, baseline), &text, &self.font);
            }
        }
    }
}
