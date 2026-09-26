//! Font family selection combo box (`QFontComboBox`).

use qtrs_core::event::Event;
use qtrs_core::object::{ObjectData, ObjectId, QObject};
use qtrs_core::signal::Signal;
use qtrs_gui::geometry::primitives::{Rect, Size};
use qtrs_gui::paint::Painter;
use qtrs_gui::text::Font;

use crate::combo_box::ComboBox;
use crate::focus::FocusPolicy;
use crate::layout::Layout;
use crate::size_policy::QSizePolicy;
use crate::widget::{Widget, WidgetRef, WidgetWeak};

/// Filter flags for font family selection matching `QFontComboBox::FontFilter`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum FontFilter {
    #[default]
    AllFonts,
    MonospacedFonts,
    ProportionalFonts,
}

pub struct FontComboBox {
    combo: ComboBox,
    font_filter: FontFilter,
    pub current_font_changed: Signal<Font>,
}

pub type QFontComboBox = FontComboBox;

impl FontComboBox {
    /// Creates a new font combo box populated with available font families.
    pub fn new() -> Self {
        let mut combo = ComboBox::new();
        combo.set_editable(false);

        let mut font_combo = Self {
            combo,
            font_filter: FontFilter::AllFonts,
            current_font_changed: Signal::new(),
        };

        font_combo.populate_fonts();
        font_combo
    }

    /// Sets the font filter and repopulates the list.
    pub fn set_font_filters(&mut self, filter: FontFilter) {
        if self.font_filter != filter {
            self.font_filter = filter;
            self.populate_fonts();
        }
    }

    pub fn font_filters(&self) -> FontFilter {
        self.font_filter
    }

    /// Returns the currently selected font.
    pub fn current_font(&self) -> Font {
        let family = self.combo.current_text();
        Font::new(
            if family.is_empty() {
                "Segoe UI"
            } else {
                &family
            },
            12.0,
        )
    }

    /// Sets the current font by family name.
    pub fn set_current_font(&mut self, font: &Font) {
        let idx = self
            .combo
            .find_text(&font.family, crate::combo_box::MatchFlags::default());
        if idx >= 0 {
            self.combo.set_current_index(idx);
            self.current_font_changed.emit(font);
        }
    }

    /// Populates font list based on system/standard fonts.
    fn populate_fonts(&mut self) {
        let mut families =
            qtrs_gui::text::font_database::with_global_font_database(|db| match self.font_filter {
                FontFilter::AllFonts => db.families(),
                FontFilter::MonospacedFonts => db.monospaced_families(),
                FontFilter::ProportionalFonts => db.proportional_families(),
            });
        if families.is_empty() {
            families = vec![
                "Arial".to_string(),
                "Calibri".to_string(),
                "Consolas".to_string(),
                "Courier New".to_string(),
                "Georgia".to_string(),
                "Segoe UI".to_string(),
                "Tahoma".to_string(),
                "Times New Roman".to_string(),
                "Trebuchet MS".to_string(),
                "Verdana".to_string(),
            ];
        }

        self.combo.clear();

        for fam in families {
            let is_monospace = fam.eq_ignore_ascii_case("Consolas")
                || fam.eq_ignore_ascii_case("Courier New")
                || fam.to_ascii_lowercase().contains("mono");

            let matches = match self.font_filter {
                FontFilter::AllFonts => true,
                FontFilter::MonospacedFonts => is_monospace,
                FontFilter::ProportionalFonts => !is_monospace,
            };

            if matches {
                self.combo.add_item(fam);
            }
        }
    }
}

impl QObject for FontComboBox {
    fn object_data(&self) -> &ObjectData {
        self.combo.object_data()
    }

    fn object_data_mut(&mut self) -> &mut ObjectData {
        self.combo.object_data_mut()
    }

    fn event(&mut self, event: &mut Event) -> bool {
        let prev_font = self.current_font();
        let res = self.combo.event(event);
        let new_font = self.current_font();
        if prev_font.family != new_font.family {
            self.current_font_changed.emit(&new_font);
        }
        res
    }
}

impl Widget for FontComboBox {
    fn id(&self) -> ObjectId {
        self.combo.id()
    }

    fn geometry(&self) -> Rect {
        self.combo.geometry()
    }

    fn set_geometry(&mut self, rect: Rect) {
        self.combo.set_geometry(rect);
    }

    fn is_visible(&self) -> bool {
        self.combo.is_visible()
    }

    fn set_visible(&mut self, visible: bool) {
        self.combo.set_visible(visible);
    }

    fn is_enabled(&self) -> bool {
        self.combo.is_enabled()
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.combo.set_enabled(enabled);
    }

    fn update(&mut self) {
        self.combo.update();
    }

    fn dirty_rect(&self) -> Option<Rect> {
        self.combo.dirty_rect()
    }

    fn clear_dirty(&mut self) {
        self.combo.clear_dirty();
    }

    fn layout(&self) -> Option<&dyn Layout> {
        self.combo.layout()
    }

    fn layout_mut(&mut self) -> Option<&mut Box<dyn Layout>> {
        self.combo.layout_mut()
    }

    fn set_layout(&mut self, layout: Box<dyn Layout>) {
        self.combo.set_layout(layout);
    }

    fn parent_widget(&self) -> Option<WidgetWeak> {
        self.combo.parent_widget()
    }

    fn set_parent_widget(&mut self, parent: Option<WidgetWeak>) {
        self.combo.set_parent_widget(parent);
    }

    fn window_id(&self) -> Option<ObjectId> {
        self.combo.window_id()
    }

    fn set_window_id(&mut self, window_id: Option<ObjectId>) {
        self.combo.set_window_id(window_id);
    }

    fn children(&self) -> Vec<WidgetRef> {
        self.combo.children()
    }

    fn add_child(&mut self, child: WidgetRef) {
        self.combo.add_child(child);
    }

    fn remove_child(&mut self, child_id: ObjectId) {
        self.combo.remove_child(child_id);
    }

    fn size_hint(&self) -> Size {
        self.combo.size_hint()
    }

    fn size_policy(&self) -> QSizePolicy {
        self.combo.size_policy()
    }

    fn set_size_policy(&mut self, policy: QSizePolicy) {
        self.combo.set_size_policy(policy);
    }

    fn focus_policy(&self) -> FocusPolicy {
        self.combo.focus_policy()
    }

    fn set_focus_policy(&mut self, policy: FocusPolicy) {
        self.combo.set_focus_policy(policy);
    }

    fn has_focus(&self) -> bool {
        self.combo.has_focus()
    }

    fn set_has_focus(&mut self, focus: bool) {
        self.combo.set_has_focus(focus);
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn paint_event(&mut self, painter: &mut Painter) {
        self.combo.paint_event(painter);
    }
}
