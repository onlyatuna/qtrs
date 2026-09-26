//! Flat model helpers (`QAbstractTableModel`, `QAbstractListModel`) and the
//! concrete [`StringListModel`] (`QStringListModel`).
//!
//! Implementing [`AbstractTableModel`] (or [`AbstractListModel`], which is a
//! single-column table) provides [`AbstractItemModel`] automatically. The
//! helper traits use `table_`/`list_` prefixed names so that they never clash
//! with [`AbstractItemModel`] methods when both traits are in scope.

use std::cell::RefCell;

use qtrs_core::Variant;

use crate::index::ModelIndex;
use crate::model::{default_header_data, AbstractItemModel, ModelBase};
use crate::role::{ItemDataRole, ItemFlags, Orientation, SortOrder};

/// Two-dimensional model without children (`QAbstractTableModel`).
pub trait AbstractTableModel {
    fn table_base(&self) -> &ModelBase;
    fn table_row_count(&self) -> i32;
    fn table_column_count(&self) -> i32;
    fn table_data(&self, row: i32, column: i32, role: ItemDataRole) -> Variant;

    fn table_set_data(
        &self,
        _row: i32,
        _column: i32,
        _value: Variant,
        _role: ItemDataRole,
    ) -> bool {
        false
    }

    fn table_flags(&self, _row: i32, _column: i32) -> ItemFlags {
        ItemFlags::SELECTABLE | ItemFlags::ENABLED | ItemFlags::NEVER_HAS_CHILDREN
    }

    fn table_header_data(
        &self,
        section: i32,
        _orientation: Orientation,
        role: ItemDataRole,
    ) -> Variant {
        default_header_data(section, role)
    }

    fn table_set_header_data(
        &self,
        _section: i32,
        _orientation: Orientation,
        _value: Variant,
        _role: ItemDataRole,
    ) -> bool {
        false
    }

    fn table_insert_rows(&self, _row: i32, _count: i32) -> bool {
        false
    }

    fn table_remove_rows(&self, _row: i32, _count: i32) -> bool {
        false
    }

    fn table_insert_columns(&self, _column: i32, _count: i32) -> bool {
        false
    }

    fn table_remove_columns(&self, _column: i32, _count: i32) -> bool {
        false
    }

    fn table_sort(&self, _column: i32, _order: SortOrder) {}
}

impl<T: AbstractTableModel> AbstractItemModel for T {
    fn base(&self) -> &ModelBase {
        self.table_base()
    }

    fn index(&self, row: i32, column: i32, parent: &ModelIndex) -> ModelIndex {
        if self.has_index(row, column, parent) {
            self.create_index(row, column, 0)
        } else {
            ModelIndex::INVALID
        }
    }

    fn parent(&self, _child: &ModelIndex) -> ModelIndex {
        ModelIndex::INVALID
    }

    fn row_count(&self, parent: &ModelIndex) -> i32 {
        if parent.is_valid() {
            0
        } else {
            self.table_row_count()
        }
    }

    fn column_count(&self, parent: &ModelIndex) -> i32 {
        if parent.is_valid() {
            0
        } else {
            self.table_column_count()
        }
    }

    fn has_children(&self, parent: &ModelIndex) -> bool {
        !parent.is_valid() && self.table_row_count() > 0 && self.table_column_count() > 0
    }

    fn sibling(&self, row: i32, column: i32, _index: &ModelIndex) -> ModelIndex {
        self.index(row, column, &ModelIndex::INVALID)
    }

    fn data(&self, index: &ModelIndex, role: ItemDataRole) -> Variant {
        if self.owns(index) {
            self.table_data(index.row, index.column, role)
        } else {
            Variant::Invalid
        }
    }

    fn set_data(&self, index: &ModelIndex, value: Variant, role: ItemDataRole) -> bool {
        self.owns(index) && self.table_set_data(index.row, index.column, value, role)
    }

    fn header_data(&self, section: i32, orientation: Orientation, role: ItemDataRole) -> Variant {
        self.table_header_data(section, orientation, role)
    }

    fn set_header_data(
        &self,
        section: i32,
        orientation: Orientation,
        value: Variant,
        role: ItemDataRole,
    ) -> bool {
        self.table_set_header_data(section, orientation, value, role)
    }

    fn flags(&self, index: &ModelIndex) -> ItemFlags {
        if self.owns(index) {
            self.table_flags(index.row, index.column)
        } else {
            ItemFlags::NONE
        }
    }

    fn insert_rows(&self, row: i32, count: i32, parent: &ModelIndex) -> bool {
        !parent.is_valid() && self.table_insert_rows(row, count)
    }

    fn remove_rows(&self, row: i32, count: i32, parent: &ModelIndex) -> bool {
        !parent.is_valid() && self.table_remove_rows(row, count)
    }

    fn insert_columns(&self, column: i32, count: i32, parent: &ModelIndex) -> bool {
        !parent.is_valid() && self.table_insert_columns(column, count)
    }

    fn remove_columns(&self, column: i32, count: i32, parent: &ModelIndex) -> bool {
        !parent.is_valid() && self.table_remove_columns(column, count)
    }

    fn sort(&self, column: i32, order: SortOrder) {
        self.table_sort(column, order);
    }
}

/// Validity check shared by the flat model blanket implementation.
trait OwnsIndex {
    fn owns(&self, index: &ModelIndex) -> bool;
}

impl<T: AbstractTableModel> OwnsIndex for T {
    fn owns(&self, index: &ModelIndex) -> bool {
        index.is_valid()
            && index.model_id == self.table_base().id()
            && index.row < self.table_row_count()
            && index.column < self.table_column_count()
    }
}

/// One-dimensional model (`QAbstractListModel`): a single-column table.
pub trait AbstractListModel {
    fn list_base(&self) -> &ModelBase;
    fn list_row_count(&self) -> i32;
    fn list_data(&self, row: i32, role: ItemDataRole) -> Variant;

    fn list_set_data(&self, _row: i32, _value: Variant, _role: ItemDataRole) -> bool {
        false
    }

    fn list_flags(&self, _row: i32) -> ItemFlags {
        ItemFlags::SELECTABLE | ItemFlags::ENABLED | ItemFlags::NEVER_HAS_CHILDREN
    }

    fn list_header_data(
        &self,
        section: i32,
        _orientation: Orientation,
        role: ItemDataRole,
    ) -> Variant {
        default_header_data(section, role)
    }

    fn list_insert_rows(&self, _row: i32, _count: i32) -> bool {
        false
    }

    fn list_remove_rows(&self, _row: i32, _count: i32) -> bool {
        false
    }

    fn list_sort(&self, _order: SortOrder) {}
}

impl<T: AbstractListModel> AbstractTableModel for T {
    fn table_base(&self) -> &ModelBase {
        self.list_base()
    }

    fn table_row_count(&self) -> i32 {
        self.list_row_count()
    }

    fn table_column_count(&self) -> i32 {
        1
    }

    fn table_data(&self, row: i32, column: i32, role: ItemDataRole) -> Variant {
        if column == 0 {
            self.list_data(row, role)
        } else {
            Variant::Invalid
        }
    }

    fn table_set_data(&self, row: i32, column: i32, value: Variant, role: ItemDataRole) -> bool {
        column == 0 && self.list_set_data(row, value, role)
    }

    fn table_flags(&self, row: i32, _column: i32) -> ItemFlags {
        self.list_flags(row)
    }

    fn table_header_data(
        &self,
        section: i32,
        orientation: Orientation,
        role: ItemDataRole,
    ) -> Variant {
        self.list_header_data(section, orientation, role)
    }

    fn table_insert_rows(&self, row: i32, count: i32) -> bool {
        self.list_insert_rows(row, count)
    }

    fn table_remove_rows(&self, row: i32, count: i32) -> bool {
        self.list_remove_rows(row, count)
    }

    fn table_sort(&self, column: i32, order: SortOrder) {
        if column == 0 {
            self.list_sort(order);
        }
    }
}

/// Editable list of strings (`QStringListModel`).
pub struct StringListModel {
    base: ModelBase,
    strings: RefCell<Vec<String>>,
}

impl Default for StringListModel {
    fn default() -> Self {
        Self::new()
    }
}

impl StringListModel {
    pub fn new() -> Self {
        Self {
            base: ModelBase::new(),
            strings: RefCell::new(Vec::new()),
        }
    }

    pub fn with_strings<I, S>(strings: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            base: ModelBase::new(),
            strings: RefCell::new(strings.into_iter().map(Into::into).collect()),
        }
    }

    /// Copy of the stored strings.
    pub fn string_list(&self) -> Vec<String> {
        self.strings.borrow().clone()
    }

    /// Replaces all strings, resetting the model.
    pub fn set_string_list<I, S>(&self, strings: I)
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let new_list: Vec<String> = strings.into_iter().map(Into::into).collect();
        self.begin_reset_model();
        *self.strings.borrow_mut() = new_list;
        self.end_reset_model();
    }

    /// String at `row`, if any.
    pub fn string_at(&self, row: i32) -> Option<String> {
        usize::try_from(row)
            .ok()
            .and_then(|r| self.strings.borrow().get(r).cloned())
    }

    /// Number of strings.
    pub fn count(&self) -> i32 {
        self.strings.borrow().len() as i32
    }
}

impl AbstractListModel for StringListModel {
    fn list_base(&self) -> &ModelBase {
        &self.base
    }

    fn list_row_count(&self) -> i32 {
        self.count()
    }

    fn list_data(&self, row: i32, role: ItemDataRole) -> Variant {
        match role {
            ItemDataRole::Display | ItemDataRole::Edit => self
                .string_at(row)
                .map_or(Variant::Invalid, Variant::String),
            _ => Variant::Invalid,
        }
    }

    fn list_set_data(&self, row: i32, value: Variant, role: ItemDataRole) -> bool {
        if !matches!(role, ItemDataRole::Display | ItemDataRole::Edit)
            || row < 0
            || row >= self.count()
        {
            return false;
        }
        let text = value.to_string_lossy();
        {
            let mut strings = self.strings.borrow_mut();
            let slot = &mut strings[row as usize];
            if *slot == text {
                return true;
            }
            *slot = text;
        }
        let idx = self.create_index(row, 0, 0);
        self.emit_data_changed(&idx, &idx, &[ItemDataRole::Display, ItemDataRole::Edit]);
        true
    }

    fn list_flags(&self, _row: i32) -> ItemFlags {
        ItemFlags::SELECTABLE
            | ItemFlags::ENABLED
            | ItemFlags::EDITABLE
            | ItemFlags::DRAG_ENABLED
            | ItemFlags::NEVER_HAS_CHILDREN
    }

    fn list_insert_rows(&self, row: i32, count: i32) -> bool {
        if count < 1 || row < 0 || row > self.count() {
            return false;
        }
        self.begin_insert_rows(&ModelIndex::INVALID, row, row + count - 1);
        {
            let mut strings = self.strings.borrow_mut();
            let at = row as usize;
            strings.splice(at..at, std::iter::repeat_n(String::new(), count as usize));
        }
        self.end_insert_rows();
        true
    }

    fn list_remove_rows(&self, row: i32, count: i32) -> bool {
        if count <= 0 || row < 0 || row + count > self.count() {
            return false;
        }
        self.begin_remove_rows(&ModelIndex::INVALID, row, row + count - 1);
        self.strings
            .borrow_mut()
            .drain(row as usize..(row + count) as usize);
        self.end_remove_rows();
        true
    }

    fn list_sort(&self, order: SortOrder) {
        self.begin_layout_change();
        let forwarding = {
            let mut strings = self.strings.borrow_mut();
            let mut decorated: Vec<(String, usize)> =
                strings.drain(..).enumerate().map(|(i, s)| (s, i)).collect();
            match order {
                SortOrder::Ascending => decorated.sort_by(|a, b| a.0.cmp(&b.0)),
                SortOrder::Descending => decorated.sort_by(|a, b| b.0.cmp(&a.0)),
            }
            let mut forwarding = vec![0i32; decorated.len()];
            for (new_row, (text, old_row)) in decorated.into_iter().enumerate() {
                forwarding[old_row] = new_row as i32;
                strings.push(text);
            }
            forwarding
        };
        let model_id = self.base.id();
        self.base.remap_persistent_indexes(|idx| {
            (idx.model_id == model_id)
                .then(|| {
                    forwarding
                        .get(idx.row as usize)
                        .map(|&row| ModelIndex::new(row, idx.column, 0, model_id))
                })
                .flatten()
        });
        self.end_layout_change();
    }
}

/// Canonical Qt alias.
pub type QStringListModel = StringListModel;
