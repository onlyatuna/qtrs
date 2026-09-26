//! Convenience item widgets backed by the standard Qt item models.
use std::cell::RefCell;
use std::rc::Rc;

use qtrs_core::event::Event;
use qtrs_core::object::{ObjectId, QObject};
use qtrs_core::Variant;
use qtrs_gui::geometry::primitives::{Point, Rect, Size};
use qtrs_gui::paint::Painter;
use qtrs_model::{
    AbstractItemModel, ItemDataRole, ModelIndex, SharedModel, StandardItem, StandardItemModel,
    StringListModel,
};

use crate::focus::FocusPolicy;
use crate::item_view::{ListView, TableView, TreeView};
use crate::layout::Layout;
use crate::size_policy::QSizePolicy;
use crate::widget::{Widget, WidgetRef, WidgetWeak};

pub type QListWidget = ListWidget;
pub type QTreeWidget = TreeWidget;
pub type QTableWidget = TableWidget;
pub type QTreeWidgetItem = StandardItem;

/// Editable string list with an owned list model.
pub struct ListWidget {
    view: ListView,
    model: Rc<RefCell<StringListModel>>,
}

impl ListWidget {
    pub fn new() -> Self {
        let model = Rc::new(RefCell::new(StringListModel::new()));
        let shared: SharedModel = model.clone();
        Self {
            view: ListView::new(shared),
            model,
        }
    }
    pub fn count(&self) -> i32 {
        self.model.borrow().count()
    }
    pub fn item(&self, row: i32) -> Option<String> {
        self.model.borrow().string_at(row)
    }
    pub fn add_item(&mut self, text: impl Into<String>) {
        self.insert_item(self.count(), text);
    }
    pub fn insert_item(&mut self, row: i32, text: impl Into<String>) -> bool {
        if row < 0 || row > self.count() {
            return false;
        }
        let model = self.model.borrow();
        let root = ModelIndex::INVALID;
        if !model.insert_rows(row, 1, &root) {
            return false;
        }
        let index = model.index(row, 0, &root);
        let inserted = model.set_data(&index, Variant::String(text.into()), ItemDataRole::Edit);
        drop(model);
        self.view.view_mut().update();
        inserted
    }
    pub fn remove_item(&mut self, row: i32) -> bool {
        let model = self.model.borrow();
        let root = ModelIndex::INVALID;
        let removed = row >= 0 && row < model.count() && model.remove_rows(row, 1, &root);
        drop(model);
        if removed {
            self.view.view_mut().update();
        }
        removed
    }
    pub fn current_index(&self) -> ModelIndex {
        self.view.current_index()
    }
    pub fn view(&self) -> &ListView {
        &self.view
    }
}
impl Default for ListWidget {
    fn default() -> Self {
        Self::new()
    }
}

/// Hierarchical item widget backed by a standard item model.
pub struct TreeWidget {
    view: TreeView,
    model: Rc<RefCell<StandardItemModel>>,
}
impl TreeWidget {
    pub fn new() -> Self {
        let model = Rc::new(RefCell::new(StandardItemModel::new()));
        let shared: SharedModel = model.clone();
        Self {
            view: TreeView::new(shared),
            model,
        }
    }
    pub fn top_level_count(&self) -> i32 {
        self.model.borrow().row_count(&ModelIndex::invalid())
    }
    pub fn top_level_item(&self, row: i32) -> Option<StandardItem> {
        self.model.borrow().item(row, 0)
    }
    pub fn add_top_level_item(&mut self, item: StandardItem) {
        self.model.borrow().append_row(vec![item]);
        self.view.view_mut().update();
    }
    pub fn current_index(&self) -> ModelIndex {
        self.view.current_index()
    }
    pub fn view(&self) -> &TreeView {
        &self.view
    }
}
impl Default for TreeWidget {
    fn default() -> Self {
        Self::new()
    }
}

/// Table item widget backed by a standard item model.
pub struct TableWidget {
    view: TableView,
    model: Rc<RefCell<StandardItemModel>>,
}
impl TableWidget {
    pub fn new(rows: i32, columns: i32) -> Self {
        let model = Rc::new(RefCell::new(StandardItemModel::new()));
        let shared: SharedModel = model.clone();
        let result = Self {
            view: TableView::new(shared),
            model,
        };
        let root = ModelIndex::invalid();
        if columns > 0 {
            result.model.borrow().insert_columns(0, columns, &root);
        }
        if rows > 0 {
            result.model.borrow().insert_rows(0, rows, &root);
        }
        result
    }
    pub fn row_count(&self) -> i32 {
        self.model.borrow().row_count(&ModelIndex::invalid())
    }
    pub fn column_count(&self) -> i32 {
        self.model.borrow().column_count(&ModelIndex::invalid())
    }
    pub fn set_item(&mut self, row: i32, column: i32, item: StandardItem) -> bool {
        if row < 0 || column < 0 || row >= self.row_count() || column >= self.column_count() {
            return false;
        }
        self.model.borrow().set_item(row, column, item);
        self.view.view_mut().update();
        true
    }
    pub fn item(&self, row: i32, column: i32) -> Option<StandardItem> {
        self.model.borrow().item(row, column)
    }
    pub fn current_index(&self) -> ModelIndex {
        self.view.current_index()
    }
    pub fn view(&self) -> &TableView {
        &self.view
    }
}
impl Default for TableWidget {
    fn default() -> Self {
        Self::new(0, 0)
    }
}

macro_rules! delegate_widget {
    ($type:ty, $view:ident) => {
        impl QObject for $type {
            fn object_data(&self) -> &qtrs_core::object::ObjectData {
                self.$view.view().object_data()
            }
            fn object_data_mut(&mut self) -> &mut qtrs_core::object::ObjectData {
                self.$view.view_mut().object_data_mut()
            }
            fn as_qobject_any(&self) -> Option<&dyn std::any::Any> {
                Some(self)
            }
            fn as_qobject_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
                Some(self)
            }
            fn event(&mut self, event: &mut Event) -> bool {
                self.$view.view_mut().event(event)
            }
        }
        impl Widget for $type {
            fn id(&self) -> ObjectId {
                self.$view.id()
            }
            fn geometry(&self) -> Rect {
                self.$view.geometry()
            }
            fn set_geometry(&mut self, value: Rect) {
                self.$view.set_geometry(value);
            }
            fn is_visible(&self) -> bool {
                self.$view.is_visible()
            }
            fn set_visible(&mut self, value: bool) {
                self.$view.set_visible(value);
            }
            fn is_enabled(&self) -> bool {
                self.$view.is_enabled()
            }
            fn set_enabled(&mut self, value: bool) {
                self.$view.set_enabled(value);
            }
            fn update(&mut self) {
                self.$view.update();
            }
            fn dirty_rect(&self) -> Option<Rect> {
                self.$view.dirty_rect()
            }
            fn clear_dirty(&mut self) {
                self.$view.clear_dirty();
            }
            fn layout(&self) -> Option<&dyn Layout> {
                self.$view.layout()
            }
            fn layout_mut(&mut self) -> Option<&mut Box<dyn Layout>> {
                self.$view.layout_mut()
            }
            fn set_layout(&mut self, value: Box<dyn Layout>) {
                self.$view.set_layout(value);
            }
            fn parent_widget(&self) -> Option<WidgetWeak> {
                self.$view.parent_widget()
            }
            fn set_parent_widget(&mut self, value: Option<WidgetWeak>) {
                self.$view.set_parent_widget(value);
            }
            fn window_id(&self) -> Option<ObjectId> {
                self.$view.window_id()
            }
            fn set_window_id(&mut self, value: Option<ObjectId>) {
                self.$view.set_window_id(value);
            }
            fn children(&self) -> Vec<WidgetRef> {
                self.$view.children()
            }
            fn add_child(&mut self, value: WidgetRef) {
                self.$view.add_child(value);
            }
            fn remove_child(&mut self, id: ObjectId) {
                self.$view.remove_child(id);
            }
            fn size_hint(&self) -> Size {
                self.$view.size_hint()
            }
            fn size_policy(&self) -> QSizePolicy {
                self.$view.size_policy()
            }
            fn set_size_policy(&mut self, value: QSizePolicy) {
                self.$view.set_size_policy(value);
            }
            fn focus_policy(&self) -> FocusPolicy {
                self.$view.focus_policy()
            }
            fn set_focus_policy(&mut self, value: FocusPolicy) {
                self.$view.set_focus_policy(value);
            }
            fn has_focus(&self) -> bool {
                self.$view.has_focus()
            }
            fn set_has_focus(&mut self, value: bool) {
                self.$view.set_has_focus(value);
            }
            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
            fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
                self
            }
            fn paint_event(&mut self, painter: &mut Painter) {
                self.$view.paint_event(painter);
            }
            fn mouse_press_event(&mut self, pos: Point, button: u32, modifiers: u32) {
                self.$view.mouse_press_event(pos, button, modifiers);
            }
            fn key_press_event(&mut self, key: u32, modifiers: u32, repeat: bool) {
                self.$view.key_press_event(key, modifiers, repeat);
            }
            fn wheel_event(&mut self, pos: Point, delta: i32, modifiers: u32) {
                self.$view.wheel_event(pos, delta, modifiers);
            }
        }
    };
}
delegate_widget!(ListWidget, view);
delegate_widget!(TreeWidget, view);
delegate_widget!(TableWidget, view);
