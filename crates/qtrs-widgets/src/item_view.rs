//! Model-backed item views (`QAbstractItemView`, list/table/tree/column views).
//!
//! The item model remains the source of truth; this module owns view geometry,
//! rendering and selection only.

use qtrs_core::event::{Event, EventKind};
use qtrs_core::object::{ObjectId, QObject};
use qtrs_core::signal::Signal;
use qtrs_gui::geometry::primitives::{Point, PointF, Rect, RectF, Size};
use qtrs_gui::paint::{Brush, Painter, Pen};
use qtrs_gui::text::Font;
use qtrs_gui::tiny_skia::Color;
use qtrs_model::{
    AbstractItemModel, ItemDataRole, ItemFlags, ItemSelectionModel, ModelIndex, Orientation,
    SelectionFlags, SharedModel,
};

use crate::focus::FocusPolicy;
use crate::input_common;
use crate::input_keys;
use crate::size_policy::{Policy, QSizePolicy};
use crate::widget::{Widget, WidgetBase};

const DEFAULT_ROW_HEIGHT: i32 = 24;
const HEADER_HEIGHT: i32 = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewMode {
    #[default]
    List,
    Table,
    Tree,
    Column,
}

/// Delegate contract for per-index paint/edit policy.
pub trait ItemDelegate {
    fn display_text(&self, model: &dyn AbstractItemModel, index: &ModelIndex) -> String {
        model.data(index, ItemDataRole::Display).to_string_lossy()
    }

    fn size_hint(&self, _model: &dyn AbstractItemModel, _index: &ModelIndex) -> Size {
        Size::new(120, DEFAULT_ROW_HEIGHT)
    }
}

/// Default text delegate (`QStyledItemDelegate` equivalent for the basic view).
#[derive(Default)]
pub struct StyledItemDelegate {
    pub font: Option<Font>,
}

impl ItemDelegate for StyledItemDelegate {}

/// Shared view controller and widget implementation.
pub struct AbstractItemView {
    pub base: WidgetBase,
    model: SharedModel,
    selection_model: ItemSelectionModel,
    mode: ViewMode,
    row_height: i32,
    header_height: i32,
    vertical_offset: i32,
    current_column: i32,
    font: Font,
    delegate: Box<dyn ItemDelegate>,
    pub activated: Signal<ModelIndex>,
    pub clicked: Signal<ModelIndex>,
    pub double_clicked: Signal<ModelIndex>,
}

pub type QAbstractItemView = AbstractItemView;
pub struct ListView(AbstractItemView);
pub type QListView = ListView;
pub struct TreeView(AbstractItemView);
pub type QTreeView = TreeView;
pub struct TableView(AbstractItemView);
pub type QTableView = TableView;
pub struct ColumnView(AbstractItemView);
pub type QColumnView = ColumnView;

impl AbstractItemView {
    pub fn new(model: SharedModel) -> Self {
        Self::with_mode(model, ViewMode::List)
    }

    pub fn with_mode(model: SharedModel, mode: ViewMode) -> Self {
        let mut base = WidgetBase::new();
        base.focus_policy = FocusPolicy::StrongFocus;
        base.size_policy = QSizePolicy::new(Policy::Expanding, Policy::Expanding);
        base.geometry = Rect::new(0, 0, 320, 240);
        let selection_model = ItemSelectionModel::new(Some(model.clone()));
        Self {
            base,
            model,
            selection_model,
            mode,
            row_height: DEFAULT_ROW_HEIGHT,
            header_height: HEADER_HEIGHT,
            vertical_offset: 0,
            current_column: 0,
            font: Font::new("Segoe UI", 12.0),
            delegate: Box::<StyledItemDelegate>::default(),
            activated: Signal::new(),
            clicked: Signal::new(),
            double_clicked: Signal::new(),
        }
    }

    pub fn model(&self) -> &SharedModel {
        &self.model
    }

    pub fn set_model(&mut self, model: SharedModel) {
        self.selection_model = ItemSelectionModel::new(Some(model.clone()));
        self.model = model;
        self.vertical_offset = 0;
        self.update();
    }

    pub fn selection_model(&self) -> &ItemSelectionModel {
        &self.selection_model
    }

    pub fn set_selection_model(&mut self, selection_model: ItemSelectionModel) {
        self.selection_model = selection_model;
        self.update();
    }

    pub fn view_mode(&self) -> ViewMode {
        self.mode
    }

    pub fn set_view_mode(&mut self, mode: ViewMode) {
        if self.mode != mode {
            self.mode = mode;
            self.update();
        }
    }

    pub fn set_row_height(&mut self, height: i32) {
        if height > 0 && height != self.row_height {
            self.row_height = height;
            self.update();
        }
    }

    pub fn set_delegate(&mut self, delegate: Box<dyn ItemDelegate>) {
        self.delegate = delegate;
        self.update();
    }

    pub fn set_current_index(&mut self, index: &ModelIndex) {
        self.selection_model
            .set_current_index(index, SelectionFlags::CLEAR_AND_SELECT);
        self.ensure_visible(index);
        self.update();
    }

    pub fn current_index(&self) -> ModelIndex {
        self.selection_model.current_index()
    }

    pub fn scroll_to(&mut self, index: &ModelIndex) {
        self.ensure_visible(index);
        self.update();
    }

    pub fn vertical_offset(&self) -> i32 {
        self.vertical_offset
    }

    pub fn set_vertical_offset(&mut self, offset: i32) {
        let count = self.visible_row_count();
        let page = self.page_rows();
        self.vertical_offset = offset.clamp(0, (count - page).max(0));
        self.update();
    }

    pub fn index_at(&self, pos: Point) -> ModelIndex {
        let header = if self.mode == ViewMode::Table {
            self.header_height
        } else {
            0
        };
        if pos.x < 0
            || pos.y < header
            || pos.x >= self.base.geometry.width
            || pos.y >= self.base.geometry.height
        {
            return ModelIndex::INVALID;
        }
        let visual_row = (pos.y - header) / self.row_height + self.vertical_offset;
        let model = self.model.borrow();
        let root = ModelIndex::INVALID;
        let columns = model.column_count(&root);
        let column = if self.mode == ViewMode::Table && columns > 1 {
            let col_width = (self.base.geometry.width / columns).max(1);
            (pos.x / col_width).min(columns - 1)
        } else {
            self.current_column.clamp(0, columns.saturating_sub(1))
        };
        if self.mode == ViewMode::Tree {
            self.tree_indexes(&*model)
                .get(visual_row as usize)
                .copied()
                .unwrap_or(ModelIndex::INVALID)
        } else {
            model.index(visual_row, column, &root)
        }
    }

    fn tree_indexes(&self, model: &dyn AbstractItemModel) -> Vec<ModelIndex> {
        fn append(model: &dyn AbstractItemModel, parent: &ModelIndex, rows: &mut Vec<ModelIndex>) {
            for row in 0..model.row_count(parent) {
                let index = model.index(row, 0, parent);
                if index.is_valid() {
                    rows.push(index);
                    append(model, &index, rows);
                }
            }
        }
        let mut rows = Vec::new();
        append(model, &ModelIndex::INVALID, &mut rows);
        rows
    }

    fn visible_row_count(&self) -> i32 {
        if self.mode == ViewMode::Tree {
            self.tree_indexes(&*self.model.borrow()).len() as i32
        } else {
            self.model.borrow().row_count(&ModelIndex::INVALID)
        }
    }

    fn page_rows(&self) -> i32 {
        let header = if self.mode == ViewMode::Table {
            self.header_height
        } else {
            0
        };
        ((self.base.geometry.height - header).max(0) / self.row_height).max(1)
    }

    fn ensure_visible(&mut self, index: &ModelIndex) {
        if !index.is_valid() {
            return;
        }
        let row = if self.mode == ViewMode::Tree {
            let indexes = self.tree_indexes(&*self.model.borrow());
            indexes
                .iter()
                .position(|visible| visible == index)
                .map(|row| row as i32)
        } else {
            Some(index.row)
        };
        let Some(row) = row else { return };
        let page = self.page_rows();
        if row < self.vertical_offset {
            self.vertical_offset = row;
        } else if row >= self.vertical_offset + page {
            self.vertical_offset = row - page + 1;
        }
        let max = (self.visible_row_count() - page).max(0);
        self.vertical_offset = self.vertical_offset.clamp(0, max);
    }

    fn move_current(&mut self, delta: i32) {
        let rows = self.visible_row_count();
        if rows <= 0 {
            return;
        }
        let current = self.selection_model.current_index();
        let row = if self.mode == ViewMode::Tree {
            let indexes = self.tree_indexes(&*self.model.borrow());
            current.is_valid().then(|| {
                indexes
                    .iter()
                    .position(|index| index == &current)
                    .unwrap_or(0) as i32
            })
        } else {
            current.is_valid().then_some(current.row)
        }
        .unwrap_or(0);
        let target = (row + delta).clamp(0, rows - 1);
        let index = if self.mode == ViewMode::Tree {
            self.tree_indexes(&*self.model.borrow())
                .get(target as usize)
                .copied()
                .unwrap_or(ModelIndex::INVALID)
        } else {
            self.model
                .borrow()
                .index(target, self.current_column, &ModelIndex::INVALID)
        };
        if index.is_valid() {
            self.set_current_index(&index);
        }
    }
}

impl QObject for AbstractItemView {
    fn object_data(&self) -> &qtrs_core::object::ObjectData {
        &self.base.object_data
    }
    fn object_data_mut(&mut self) -> &mut qtrs_core::object::ObjectData {
        &mut self.base.object_data
    }
    fn as_qobject_any(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }
    fn as_qobject_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
    fn event(&mut self, event: &mut Event) -> bool {
        if let EventKind::MouseButtonDblClick { x, y, button } = event.kind {
            if button == 1 {
                let index = self.index_at(Point::new(x, y));
                if index.is_valid() {
                    self.double_clicked.emit(&index);
                    self.activated.emit(&index);
                }
            }
            true
        } else {
            input_common::dispatch_input_event(self, event)
        }
    }
}

impl Widget for AbstractItemView {
    leaf_widget_common!();

    fn set_enabled(&mut self, enabled: bool) {
        if self.base.enabled != enabled {
            self.base.enabled = enabled;
            self.update();
        }
    }

    fn set_window_id(&mut self, id: Option<ObjectId>) {
        self.base.window_id = id;
    }

    fn size_hint(&self) -> Size {
        Size::new(320, 240)
    }

    fn update(&mut self) {
        self.base.dirty = Some(self.base.geometry);
    }

    fn paint_event(&mut self, painter: &mut Painter) {
        let rect = self.base.geometry;
        painter.set_brush(Brush::Color(Color::from_rgba8(255, 255, 255, 255)));
        painter.set_pen(Pen::new(Color::from_rgba8(190, 190, 190, 255), 1.0));
        painter.draw_rect(RectF::new(0.0, 0.0, rect.width as f32, rect.height as f32));

        let model = self.model.borrow();
        let root = ModelIndex::INVALID;
        let columns = model.column_count(&root).max(1);
        if self.mode == ViewMode::Table {
            let col_width = (rect.width / columns).max(1);
            for column in 0..columns {
                let x = column * col_width;
                painter.set_brush(Brush::Color(Color::from_rgba8(242, 242, 242, 255)));
                painter.set_pen(Pen::new(Color::from_rgba8(200, 200, 200, 255), 1.0));
                painter.draw_rect(RectF::new(
                    x as f32,
                    0.0,
                    col_width as f32,
                    self.header_height as f32,
                ));
                let value =
                    model.header_data(column, Orientation::Horizontal, ItemDataRole::Display);
                painter.set_pen(Pen::new(Color::from_rgba8(30, 30, 30, 255), 1.0));
                painter.draw_text(
                    PointF::new((x + 6) as f32, 17.0),
                    &value.to_string_lossy(),
                    &self.font,
                );
            }
        }

        let tree_indexes = (self.mode == ViewMode::Tree).then(|| self.tree_indexes(&*model));
        let count = tree_indexes
            .as_ref()
            .map_or_else(|| model.row_count(&root), |indexes| indexes.len() as i32);
        let columns = model.column_count(&root).max(1);
        let first = self.vertical_offset;
        let page = self.page_rows();
        let end = (first + page).min(count);
        for row in first..end {
            let visible_row = row - first;
            let y = if self.mode == ViewMode::Table {
                self.header_height
            } else {
                0
            };
            let top = y + visible_row * self.row_height;
            let row_index = tree_indexes.as_ref().map_or_else(
                || model.index(row, 0, &root),
                |indexes| indexes[row as usize],
            );
            let parent = if self.mode == ViewMode::Tree {
                row_index.parent(&*model)
            } else {
                root
            };
            let depth = if self.mode == ViewMode::Tree {
                let mut depth = 0;
                let mut ancestor = parent;
                while ancestor.is_valid() {
                    depth += 1;
                    ancestor = ancestor.parent(&*model);
                }
                depth
            } else {
                0
            };
            let selected = self.selection_model.is_selected(&row_index);
            let bg = if selected {
                Color::from_rgba8(0, 120, 215, 255)
            } else if visible_row % 2 == 1 {
                Color::from_rgba8(248, 248, 248, 255)
            } else {
                Color::from_rgba8(255, 255, 255, 255)
            };
            painter.set_brush(Brush::Color(bg));
            painter.set_pen(Pen::new(Color::from_rgba8(235, 235, 235, 255), 1.0));
            painter.draw_rect(RectF::new(
                1.0,
                top as f32,
                (rect.width - 2).max(0) as f32,
                self.row_height as f32,
            ));

            let visible_columns = if self.mode == ViewMode::Table {
                columns
            } else {
                1
            };
            let col_width = (rect.width / visible_columns).max(1);
            for column in 0..visible_columns {
                let index = if self.mode == ViewMode::Tree && column == 0 {
                    row_index
                } else {
                    model.index(row_index.row, column, &parent)
                };
                if !index.is_valid() {
                    continue;
                }
                let value = self.delegate.display_text(&*model, &index);
                let color = if selected {
                    Color::from_rgba8(255, 255, 255, 255)
                } else {
                    Color::from_rgba8(25, 25, 25, 255)
                };
                painter.set_pen(Pen::new(color, 1.0));
                let indent = if self.mode == ViewMode::Tree {
                    depth * 18 + 6
                } else {
                    6
                };
                painter.draw_text(
                    PointF::new(
                        (column * col_width + indent) as f32,
                        (top + self.row_height - 6) as f32,
                    ),
                    &value,
                    &self.font,
                );
                if self.mode == ViewMode::Table {
                    painter.set_pen(Pen::new(Color::from_rgba8(225, 225, 225, 255), 1.0));
                    painter.draw_line(
                        PointF::new((column * col_width) as f32, top as f32),
                        PointF::new((column * col_width) as f32, (top + self.row_height) as f32),
                    );
                }
            }
        }
    }

    fn mouse_press_event(&mut self, pos: Point, button: u32, modifiers: u32) {
        if button != 1 || !self.base.enabled {
            return;
        }
        let index = self.index_at(pos);
        if !index.is_valid() {
            return;
        }
        let command = if input_keys::has_ctrl(modifiers) {
            SelectionFlags::TOGGLE
        } else {
            SelectionFlags::CLEAR_AND_SELECT
        };
        self.selection_model.set_current_index(&index, command);
        self.clicked.emit(&index);
        self.base.has_focus = true;
        self.update();
    }

    fn key_press_event(&mut self, key: u32, _modifiers: u32, _is_repeat: bool) {
        if input_keys::is_up(key) {
            self.move_current(-1);
        } else if input_keys::is_down(key) {
            self.move_current(1);
        } else if input_keys::is_home(key) {
            self.move_current(-self.visible_row_count());
        } else if input_keys::is_end(key) {
            self.move_current(self.visible_row_count());
        } else if input_keys::is_enter(key) {
            let index = self.selection_model.current_index();
            if index.is_valid() {
                self.activated.emit(&index);
            }
        }
    }

    fn wheel_event(&mut self, _pos: Point, delta_y: i32, _modifiers: u32) {
        if delta_y != 0 {
            self.set_vertical_offset(self.vertical_offset - delta_y.signum() * 3);
        }
    }
}

macro_rules! item_view_wrapper {
    ($name:ident, $mode:ident) => {
        impl $name {
            pub fn new(model: SharedModel) -> Self {
                Self(AbstractItemView::with_mode(model, ViewMode::$mode))
            }
            pub fn view(&self) -> &AbstractItemView {
                &self.0
            }
            pub fn view_mut(&mut self) -> &mut AbstractItemView {
                &mut self.0
            }
            pub fn model(&self) -> &SharedModel {
                self.0.model()
            }
            pub fn selection_model(&self) -> &ItemSelectionModel {
                self.0.selection_model()
            }
            pub fn current_index(&self) -> ModelIndex {
                self.0.current_index()
            }
            pub fn set_current_index(&mut self, index: &ModelIndex) {
                self.0.set_current_index(index);
            }
        }

        impl QObject for $name {
            fn object_data(&self) -> &qtrs_core::object::ObjectData {
                self.0.object_data()
            }
            fn object_data_mut(&mut self) -> &mut qtrs_core::object::ObjectData {
                self.0.object_data_mut()
            }
            fn as_qobject_any(&self) -> Option<&dyn std::any::Any> {
                Some(self)
            }
            fn as_qobject_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
                Some(self)
            }
            fn event(&mut self, event: &mut Event) -> bool {
                self.0.event(event)
            }
        }

        impl Widget for $name {
            fn id(&self) -> ObjectId {
                self.0.id()
            }
            fn geometry(&self) -> Rect {
                self.0.geometry()
            }
            fn set_geometry(&mut self, rect: Rect) {
                self.0.set_geometry(rect);
            }
            fn is_visible(&self) -> bool {
                self.0.is_visible()
            }
            fn set_visible(&mut self, value: bool) {
                self.0.set_visible(value);
            }
            fn is_enabled(&self) -> bool {
                self.0.is_enabled()
            }
            fn set_enabled(&mut self, value: bool) {
                self.0.set_enabled(value);
            }
            fn update(&mut self) {
                self.0.update();
            }
            fn dirty_rect(&self) -> Option<Rect> {
                self.0.dirty_rect()
            }
            fn clear_dirty(&mut self) {
                self.0.clear_dirty();
            }
            fn layout(&self) -> Option<&dyn crate::layout::Layout> {
                self.0.layout()
            }
            fn layout_mut(&mut self) -> Option<&mut Box<dyn crate::layout::Layout>> {
                self.0.layout_mut()
            }
            fn set_layout(&mut self, layout: Box<dyn crate::layout::Layout>) {
                self.0.set_layout(layout);
            }
            fn parent_widget(&self) -> Option<crate::widget::WidgetWeak> {
                self.0.parent_widget()
            }
            fn set_parent_widget(&mut self, parent: Option<crate::widget::WidgetWeak>) {
                self.0.set_parent_widget(parent);
            }
            fn window_id(&self) -> Option<ObjectId> {
                self.0.window_id()
            }
            fn set_window_id(&mut self, id: Option<ObjectId>) {
                self.0.set_window_id(id);
            }
            fn children(&self) -> Vec<crate::widget::WidgetRef> {
                self.0.children()
            }
            fn add_child(&mut self, child: crate::widget::WidgetRef) {
                self.0.add_child(child);
            }
            fn remove_child(&mut self, id: ObjectId) {
                self.0.remove_child(id);
            }
            fn size_hint(&self) -> Size {
                self.0.size_hint()
            }
            fn size_policy(&self) -> QSizePolicy {
                self.0.size_policy()
            }
            fn set_size_policy(&mut self, policy: QSizePolicy) {
                self.0.set_size_policy(policy);
            }
            fn focus_policy(&self) -> FocusPolicy {
                self.0.focus_policy()
            }
            fn set_focus_policy(&mut self, policy: FocusPolicy) {
                self.0.set_focus_policy(policy);
            }
            fn has_focus(&self) -> bool {
                self.0.has_focus()
            }
            fn set_has_focus(&mut self, value: bool) {
                self.0.set_has_focus(value);
            }
            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
            fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
                self
            }
            fn paint_event(&mut self, painter: &mut Painter) {
                self.0.paint_event(painter);
            }
            fn mouse_press_event(&mut self, pos: Point, button: u32, modifiers: u32) {
                self.0.mouse_press_event(pos, button, modifiers);
            }
            fn key_press_event(&mut self, key: u32, modifiers: u32, repeat: bool) {
                self.0.key_press_event(key, modifiers, repeat);
            }
            fn wheel_event(&mut self, pos: Point, delta: i32, modifiers: u32) {
                self.0.wheel_event(pos, delta, modifiers);
            }
        }
    };
}

item_view_wrapper!(ListView, List);
item_view_wrapper!(TreeView, Tree);
item_view_wrapper!(TableView, Table);
item_view_wrapper!(ColumnView, Column);

/// Simple horizontal header model paired with table views.
#[derive(Default)]
pub struct HeaderView {
    labels: Vec<String>,
    pub section_clicked: Signal<i32>,
}

pub type QHeaderView = HeaderView;

impl HeaderView {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn count(&self) -> usize {
        self.labels.len()
    }
    pub fn set_labels<I, S>(&mut self, labels: I)
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.labels = labels.into_iter().map(Into::into).collect();
    }
    pub fn label(&self, section: usize) -> Option<&str> {
        self.labels.get(section).map(String::as_str)
    }
    pub fn click_section(&self, section: i32) {
        if section >= 0 && (section as usize) < self.labels.len() {
            self.section_clicked.emit(&section);
        }
    }
}

/// Synchronizes a model index with a value widget via role-based reads/writes.
pub struct DataWidgetMapper {
    model: Option<SharedModel>,
    current_index: i32,
    mapped_column: i32,
    mapped_role: ItemDataRole,
}

pub type QDataWidgetMapper = DataWidgetMapper;

impl Default for DataWidgetMapper {
    fn default() -> Self {
        Self {
            model: None,
            current_index: -1,
            mapped_column: 0,
            mapped_role: ItemDataRole::Edit,
        }
    }
}

impl DataWidgetMapper {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn set_model(&mut self, model: SharedModel) {
        self.model = Some(model);
    }
    pub fn current_index(&self) -> i32 {
        self.current_index
    }
    pub fn set_current_index(&mut self, index: i32) {
        let Some(model) = self.model.as_ref() else {
            return;
        };
        let count = model.borrow().row_count(&ModelIndex::INVALID);
        if index >= 0 && index < count {
            self.current_index = index;
        }
    }
    pub fn set_mapped_column(&mut self, column: i32) {
        self.mapped_column = column.max(0);
    }
    pub fn set_mapped_role(&mut self, role: ItemDataRole) {
        self.mapped_role = role;
    }
    pub fn current_data(&self) -> Option<qtrs_core::Variant> {
        let model = self.model.as_ref()?;
        let borrowed = model.borrow();
        let index = borrowed.index(self.current_index, self.mapped_column, &ModelIndex::INVALID);
        index
            .is_valid()
            .then(|| borrowed.data(&index, self.mapped_role))
    }
    pub fn set_current_data(&mut self, value: qtrs_core::Variant) -> bool {
        let Some(model) = self.model.as_ref() else {
            return false;
        };
        let borrowed = model.borrow();
        let index = borrowed.index(self.current_index, self.mapped_column, &ModelIndex::INVALID);
        index.is_valid() && borrowed.set_data(&index, value, self.mapped_role)
    }
}

// Import the bitset type to make flags available to delegate consumers.
const _SELECTABLE: ItemFlags = ItemFlags::SELECTABLE;
