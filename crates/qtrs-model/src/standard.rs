//! Generic tree/table model built from [`StandardItem`]s
//! (`QStandardItemModel`, `QStandardItem`).

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::rc::{Rc, Weak};

use qtrs_core::{Signal, Variant};

use crate::index::ModelIndex;
use crate::model::{
    default_header_data, delegate_item_model, next_unique_id, stable_sort_by_less,
    variant_less_than, AbstractItemModel, ModelBase,
};
use crate::role::{
    CaseSensitivity, CheckState, ItemDataRole, ItemFlags, MatchFlags, Orientation, SortOrder,
};

const DEFAULT_ITEM_FLAGS: ItemFlags = ItemFlags(
    ItemFlags::SELECTABLE.0
        | ItemFlags::ENABLED.0
        | ItemFlags::EDITABLE.0
        | ItemFlags::DRAG_ENABLED.0
        | ItemFlags::DROP_ENABLED.0,
);

struct ItemData {
    id: u64,
    values: Vec<(ItemDataRole, Variant)>,
    flags: ItemFlags,
    parent: Weak<RefCell<ItemData>>,
    model: Weak<StdInner>,
    rows: i32,
    columns: i32,
    /// Row-major `rows * columns` table of children.
    children: Vec<Option<StandardItem>>,
    /// Position hint inside the parent's `children`.
    last_known: Cell<usize>,
}

impl ItemData {
    fn new() -> Self {
        Self {
            id: next_unique_id(),
            values: Vec::new(),
            flags: DEFAULT_ITEM_FLAGS,
            parent: Weak::new(),
            model: Weak::new(),
            rows: 0,
            columns: 0,
            children: Vec::new(),
            last_known: Cell::new(0),
        }
    }

    fn slot(&self, row: i32, column: i32) -> Option<usize> {
        (row >= 0 && column >= 0 && row < self.rows && column < self.columns)
            .then(|| (row * self.columns + column) as usize)
    }
}

fn normalize_role(role: ItemDataRole) -> ItemDataRole {
    if role == ItemDataRole::Edit {
        ItemDataRole::Display
    } else {
        role
    }
}

/// Node of a [`StandardItemModel`] (`QStandardItem`).
///
/// `StandardItem` is a shared handle: clones refer to the same item. Changes
/// made to an item that belongs to a model are reported through the model's
/// signals.
#[derive(Clone)]
pub struct StandardItem(Rc<RefCell<ItemData>>);

impl PartialEq for StandardItem {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for StandardItem {}

impl Default for StandardItem {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for StandardItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let d = self.0.borrow();
        f.debug_struct("StandardItem")
            .field("id", &d.id)
            .field("text", &self.text())
            .field("rows", &d.rows)
            .field("columns", &d.columns)
            .finish()
    }
}

impl StandardItem {
    pub fn new() -> Self {
        Self(Rc::new(RefCell::new(ItemData::new())))
    }

    pub fn with_text(text: impl Into<String>) -> Self {
        let item = Self::new();
        item.0
            .borrow_mut()
            .values
            .push((ItemDataRole::Display, Variant::String(text.into())));
        item
    }

    /// Item with an empty `rows` x `columns` child table.
    pub fn with_size(rows: i32, columns: i32) -> Self {
        let item = Self::new();
        {
            let mut d = item.0.borrow_mut();
            d.rows = rows.max(0);
            d.columns = columns.max(0);
            d.children = vec![None; (d.rows * d.columns) as usize];
        }
        item
    }

    /// Copy of the item's data and flags, without children (`clone()`).
    pub fn clone_item(&self) -> StandardItem {
        let copy = Self::new();
        {
            let src = self.0.borrow();
            let mut dst = copy.0.borrow_mut();
            dst.values = src.values.clone();
            dst.flags = src.flags;
        }
        copy
    }

    fn id(&self) -> u64 {
        self.0.borrow().id
    }

    fn model(&self) -> Option<Rc<StdInner>> {
        self.0.borrow().model.upgrade()
    }

    // ----- data -------------------------------------------------------------

    /// Value stored for `role` (the edit role aliases the display role).
    pub fn data(&self, role: ItemDataRole) -> Variant {
        let role = normalize_role(role);
        self.0
            .borrow()
            .values
            .iter()
            .find(|(r, _)| *r == role)
            .map(|(_, v)| v.clone())
            .unwrap_or_default()
    }

    /// Stores `value` for `role`; an invalid value clears the role.
    pub fn set_data(&self, value: Variant, role: ItemDataRole) {
        let role = normalize_role(role);
        let changed = {
            let mut d = self.0.borrow_mut();
            match d.values.iter().position(|(r, _)| *r == role) {
                Some(pos) if !value.is_valid() => {
                    d.values.remove(pos);
                    true
                }
                Some(pos) if d.values[pos].1 == value => false,
                Some(pos) => {
                    d.values[pos].1 = value;
                    true
                }
                None if value.is_valid() => {
                    d.values.push((role, value));
                    true
                }
                None => false,
            }
        };
        if changed {
            if role == ItemDataRole::Display {
                self.notify_changed(&[ItemDataRole::Display, ItemDataRole::Edit]);
            } else {
                self.notify_changed(&[role]);
            }
        }
    }

    /// Removes all stored values.
    pub fn clear_data(&self) {
        let had_values = {
            let mut d = self.0.borrow_mut();
            let had = !d.values.is_empty();
            d.values.clear();
            had
        };
        if had_values {
            self.notify_changed(&[]);
        }
    }

    fn notify_changed(&self, roles: &[ItemDataRole]) {
        if let Some(model) = self.model() {
            model.item_data_changed(self, roles);
        }
    }

    fn string_role(&self, role: ItemDataRole) -> String {
        let value = self.data(role);
        if value.is_valid() {
            value.to_string_lossy()
        } else {
            String::new()
        }
    }

    pub fn text(&self) -> String {
        self.string_role(ItemDataRole::Display)
    }

    pub fn set_text(&self, text: impl Into<String>) {
        self.set_data(Variant::String(text.into()), ItemDataRole::Display);
    }

    pub fn tool_tip(&self) -> String {
        self.string_role(ItemDataRole::ToolTip)
    }

    pub fn set_tool_tip(&self, text: impl Into<String>) {
        self.set_data(Variant::String(text.into()), ItemDataRole::ToolTip);
    }

    pub fn status_tip(&self) -> String {
        self.string_role(ItemDataRole::StatusTip)
    }

    pub fn set_status_tip(&self, text: impl Into<String>) {
        self.set_data(Variant::String(text.into()), ItemDataRole::StatusTip);
    }

    pub fn whats_this(&self) -> String {
        self.string_role(ItemDataRole::WhatsThis)
    }

    pub fn set_whats_this(&self, text: impl Into<String>) {
        self.set_data(Variant::String(text.into()), ItemDataRole::WhatsThis);
    }

    pub fn check_state(&self) -> CheckState {
        self.data(ItemDataRole::CheckState)
            .to_int()
            .and_then(|v| CheckState::from_i32(v as i32))
            .unwrap_or_default()
    }

    pub fn set_check_state(&self, state: CheckState) {
        self.set_data(Variant::from(state), ItemDataRole::CheckState);
    }

    // ----- flags ------------------------------------------------------------

    pub fn flags(&self) -> ItemFlags {
        self.0.borrow().flags
    }

    pub fn set_flags(&self, flags: ItemFlags) {
        let changed = {
            let mut d = self.0.borrow_mut();
            let changed = d.flags != flags;
            d.flags = flags;
            changed
        };
        if changed {
            self.notify_changed(&[]);
        }
    }

    fn change_flag(&self, flag: ItemFlags, on: bool) {
        let mut flags = self.flags();
        flags.set(flag, on);
        self.set_flags(flags);
    }

    pub fn is_enabled(&self) -> bool {
        self.flags().contains(ItemFlags::ENABLED)
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.change_flag(ItemFlags::ENABLED, enabled);
    }

    pub fn is_editable(&self) -> bool {
        self.flags().contains(ItemFlags::EDITABLE)
    }

    pub fn set_editable(&self, editable: bool) {
        self.change_flag(ItemFlags::EDITABLE, editable);
    }

    pub fn is_selectable(&self) -> bool {
        self.flags().contains(ItemFlags::SELECTABLE)
    }

    pub fn set_selectable(&self, selectable: bool) {
        self.change_flag(ItemFlags::SELECTABLE, selectable);
    }

    pub fn is_checkable(&self) -> bool {
        self.flags().contains(ItemFlags::USER_CHECKABLE)
    }

    /// Makes the item user-checkable; a fresh checkable item starts unchecked.
    pub fn set_checkable(&self, checkable: bool) {
        if checkable && !self.is_checkable() && !self.data(ItemDataRole::CheckState).is_valid() {
            self.set_check_state(CheckState::Unchecked);
        }
        self.change_flag(ItemFlags::USER_CHECKABLE, checkable);
    }

    pub fn is_user_tristate(&self) -> bool {
        self.flags().contains(ItemFlags::USER_TRISTATE)
    }

    pub fn set_user_tristate(&self, tristate: bool) {
        self.change_flag(ItemFlags::USER_TRISTATE, tristate);
    }

    pub fn is_auto_tristate(&self) -> bool {
        self.flags().contains(ItemFlags::AUTO_TRISTATE)
    }

    pub fn set_auto_tristate(&self, tristate: bool) {
        self.change_flag(ItemFlags::AUTO_TRISTATE, tristate);
    }

    pub fn is_drag_enabled(&self) -> bool {
        self.flags().contains(ItemFlags::DRAG_ENABLED)
    }

    pub fn set_drag_enabled(&self, enabled: bool) {
        self.change_flag(ItemFlags::DRAG_ENABLED, enabled);
    }

    pub fn is_drop_enabled(&self) -> bool {
        self.flags().contains(ItemFlags::DROP_ENABLED)
    }

    pub fn set_drop_enabled(&self, enabled: bool) {
        self.change_flag(ItemFlags::DROP_ENABLED, enabled);
    }

    // ----- hierarchy queries -------------------------------------------------

    pub fn row_count(&self) -> i32 {
        self.0.borrow().rows
    }

    pub fn column_count(&self) -> i32 {
        self.0.borrow().columns
    }

    pub fn has_children(&self) -> bool {
        let d = self.0.borrow();
        d.rows > 0 && d.columns > 0
    }

    /// Child at `(row, column)`.
    pub fn child(&self, row: i32, column: i32) -> Option<StandardItem> {
        let d = self.0.borrow();
        d.slot(row, column).and_then(|i| d.children[i].clone())
    }

    /// Parent item; `None` for top-level items and detached items.
    pub fn parent(&self) -> Option<StandardItem> {
        let parent = StandardItem(self.0.borrow().parent.upgrade()?);
        match self.model() {
            Some(model) if model.root == parent => None,
            _ => Some(parent),
        }
    }

    /// `(parent, row, column)` of this item inside its parent.
    fn position(&self) -> Option<(StandardItem, i32, i32)> {
        let parent = self.0.borrow().parent.upgrade()?;
        let p = parent.borrow();
        let hint = self.0.borrow().last_known.get();
        let is_self =
            |slot: &Option<StandardItem>| slot.as_ref().is_some_and(|c| Rc::ptr_eq(&c.0, &self.0));
        let pos = if p.children.get(hint).is_some_and(is_self) {
            hint
        } else {
            let pos = p.children.iter().position(is_self)?;
            self.0.borrow().last_known.set(pos);
            pos
        };
        let columns = p.columns.max(1);
        let (row, column) = (pos as i32 / columns, pos as i32 % columns);
        drop(p);
        Some((StandardItem(parent), row, column))
    }

    /// Row inside the parent, or -1.
    pub fn row(&self) -> i32 {
        self.position().map_or(-1, |(_, row, _)| row)
    }

    /// Column inside the parent, or -1.
    pub fn column(&self) -> i32 {
        self.position().map_or(-1, |(_, _, column)| column)
    }

    /// Model index of the item (invalid when not part of a model).
    pub fn index(&self) -> ModelIndex {
        self.model()
            .map_or(ModelIndex::INVALID, |model| model.index_from_item(self))
    }

    // ----- hierarchy mutation -----------------------------------------------

    /// `true` if `item` can become a child of `self` (detached and not an ancestor).
    fn can_adopt(&self, item: &StandardItem) -> bool {
        if Rc::ptr_eq(&self.0, &item.0) {
            return false;
        }
        {
            let d = item.0.borrow();
            if d.parent.upgrade().is_some() || d.model.upgrade().is_some() {
                return false;
            }
        }
        let mut current = self.0.borrow().parent.upgrade();
        while let Some(p) = current {
            if Rc::ptr_eq(&p, &item.0) {
                return false;
            }
            current = p.borrow().parent.upgrade();
        }
        true
    }

    fn adopt(&self, item: &StandardItem, model: Option<&Rc<StdInner>>) {
        item.0.borrow_mut().parent = Rc::downgrade(&self.0);
        set_model_recursive(item, model);
    }

    fn detach(&self) {
        self.0.borrow_mut().parent = Weak::new();
        set_model_recursive(self, None);
    }

    fn refresh_hints(&self, from: usize) {
        let d = self.0.borrow();
        for (i, child) in d.children.iter().enumerate().skip(from) {
            if let Some(child) = child {
                child.0.borrow().last_known.set(i);
            }
        }
    }

    /// Model and index of `self` when attached to a model.
    fn model_context(&self) -> Option<(Rc<StdInner>, ModelIndex)> {
        let model = self.model()?;
        let index = model.index_from_item(self);
        Some((model, index))
    }

    fn insert_rows_impl(&self, row: i32, count: i32, items: Vec<StandardItem>) -> bool {
        if count < 1 || row < 0 || row > self.row_count() {
            return false;
        }
        if self.row_count() == 0 && self.column_count() == 0 {
            self.set_column_count(1);
        }
        let context = self.model_context();
        if let Some((model, parent)) = &context {
            model.begin_insert_rows(parent, row, row + count - 1);
        }
        let columns = self.column_count() as usize;
        let slots = count as usize * columns;
        let model = context.as_ref().map(|(m, _)| m);
        let mut placed: Vec<Option<StandardItem>> = items
            .into_iter()
            .take(slots)
            .map(|item| {
                self.can_adopt(&item).then(|| {
                    self.adopt(&item, model);
                    item
                })
            })
            .collect();
        placed.resize(slots, None);
        let at = row as usize * columns;
        {
            let mut d = self.0.borrow_mut();
            d.children.splice(at..at, placed);
            d.rows += count;
        }
        self.refresh_hints(at);
        if let Some((model, _)) = &context {
            if model.root == *self {
                model.root_sections_inserted(Orientation::Vertical, row, count);
            }
            model.end_insert_rows();
        }
        true
    }

    fn insert_columns_impl(&self, column: i32, count: i32, items: Vec<StandardItem>) -> bool {
        if count < 1 || column < 0 || column > self.column_count() {
            return false;
        }
        let context = self.model_context();
        if let Some((model, parent)) = &context {
            model.begin_insert_columns(parent, column, column + count - 1);
        }
        let model = context.as_ref().map(|(m, _)| m);
        let rows = self.row_count() as usize;
        let count_u = count as usize;
        let placed: Vec<Option<StandardItem>> = items
            .into_iter()
            .take(rows * count_u)
            .map(|item| {
                self.can_adopt(&item).then(|| {
                    self.adopt(&item, model);
                    item
                })
            })
            .collect();
        {
            let mut d = self.0.borrow_mut();
            let old_columns = d.columns as usize;
            let new_columns = old_columns + count_u;
            let column_u = column as usize;
            let mut old = std::mem::take(&mut d.children).into_iter();
            let mut table: Vec<Option<StandardItem>> = Vec::with_capacity(rows * new_columns);
            for _ in 0..rows {
                table.extend(old.by_ref().take(column_u));
                table.extend(std::iter::repeat_n(None, count_u));
                table.extend(old.by_ref().take(old_columns - column_u));
            }
            for (i, item) in placed.into_iter().enumerate() {
                let (r, c) = (i / count_u, column_u + i % count_u);
                table[r * new_columns + c] = item;
            }
            d.children = table;
            d.columns = new_columns as i32;
        }
        self.refresh_hints(0);
        if let Some((model, _)) = &context {
            if model.root == *self {
                model.root_sections_inserted(Orientation::Horizontal, column, count);
            }
            model.end_insert_columns();
        }
        true
    }

    /// Inserts one row of `items` (one item per column) at `row`, growing the
    /// column count when needed.
    pub fn insert_row(&self, row: i32, items: Vec<StandardItem>) {
        if row < 0 {
            return;
        }
        if (self.column_count() as usize) < items.len() {
            self.set_column_count(items.len() as i32);
        }
        self.insert_rows_impl(row, 1, items);
    }

    /// Inserts `count` empty rows at `row`.
    pub fn insert_rows(&self, row: i32, count: i32) -> bool {
        self.insert_rows_impl(row, count, Vec::new())
    }

    /// Appends one row of `items`.
    pub fn append_row(&self, items: Vec<StandardItem>) {
        self.insert_row(self.row_count(), items);
    }

    /// Appends one single-column row per item.
    pub fn append_rows(&self, items: Vec<StandardItem>) {
        let count = items.len() as i32;
        if count > 0 {
            self.insert_rows_impl(self.row_count(), count, items);
        }
    }

    /// Inserts one column of `items` (one item per row) at `column`, growing the
    /// row count when needed.
    pub fn insert_column(&self, column: i32, items: Vec<StandardItem>) {
        if column < 0 {
            return;
        }
        if (self.row_count() as usize) < items.len() {
            self.set_row_count(items.len() as i32);
        }
        self.insert_columns_impl(column, 1, items);
    }

    /// Inserts `count` empty columns at `column`.
    pub fn insert_columns(&self, column: i32, count: i32) -> bool {
        self.insert_columns_impl(column, count, Vec::new())
    }

    /// Appends one column of `items`.
    pub fn append_column(&self, items: Vec<StandardItem>) {
        self.insert_column(self.column_count(), items);
    }

    pub fn set_row_count(&self, rows: i32) {
        let current = self.row_count();
        if current < rows {
            self.insert_rows(current.max(0), rows - current);
        } else if current > rows {
            self.remove_rows(rows.max(0), current - rows);
        }
    }

    pub fn set_column_count(&self, columns: i32) {
        let current = self.column_count();
        if current < columns {
            self.insert_columns(current.max(0), columns - current);
        } else if current > columns {
            self.remove_columns(columns.max(0), current - columns);
        }
    }

    fn take_rows_impl(&self, row: i32, count: i32) -> Option<Vec<Option<StandardItem>>> {
        if count < 1 || row < 0 || row + count > self.row_count() {
            return None;
        }
        let context = self.model_context();
        if let Some((model, parent)) = &context {
            model.begin_remove_rows(parent, row, row + count - 1);
        }
        let (start, removed) = {
            let mut d = self.0.borrow_mut();
            let columns = d.columns as usize;
            let start = row as usize * columns;
            let removed: Vec<Option<StandardItem>> = d
                .children
                .drain(start..start + count as usize * columns)
                .collect();
            d.rows -= count;
            (start, removed)
        };
        for item in removed.iter().flatten() {
            item.detach();
        }
        self.refresh_hints(start);
        if let Some((model, _)) = &context {
            if model.root == *self {
                model.root_sections_removed(Orientation::Vertical, row, count);
            }
            model.end_remove_rows();
        }
        Some(removed)
    }

    fn take_columns_impl(&self, column: i32, count: i32) -> Option<Vec<Option<StandardItem>>> {
        if count < 1 || column < 0 || column + count > self.column_count() {
            return None;
        }
        let context = self.model_context();
        if let Some((model, parent)) = &context {
            model.begin_remove_columns(parent, column, column + count - 1);
        }
        let removed = {
            let mut d = self.0.borrow_mut();
            let old_columns = d.columns as usize;
            let (column_u, count_u) = (column as usize, count as usize);
            let mut removed = Vec::with_capacity(d.rows as usize * count_u);
            let mut kept = Vec::with_capacity(d.rows as usize * (old_columns - count_u));
            for (i, slot) in std::mem::take(&mut d.children).into_iter().enumerate() {
                let c = i % old_columns;
                if c >= column_u && c < column_u + count_u {
                    removed.push(slot);
                } else {
                    kept.push(slot);
                }
            }
            d.children = kept;
            d.columns -= count;
            removed
        };
        for item in removed.iter().flatten() {
            item.detach();
        }
        self.refresh_hints(0);
        if let Some((model, _)) = &context {
            if model.root == *self {
                model.root_sections_removed(Orientation::Horizontal, column, count);
            }
            model.end_remove_columns();
        }
        Some(removed)
    }

    pub fn remove_rows(&self, row: i32, count: i32) {
        self.take_rows_impl(row, count);
    }

    pub fn remove_row(&self, row: i32) {
        self.remove_rows(row, 1);
    }

    pub fn remove_columns(&self, column: i32, count: i32) {
        self.take_columns_impl(column, count);
    }

    pub fn remove_column(&self, column: i32) {
        self.remove_columns(column, 1);
    }

    /// Removes row `row` and returns its (detached) items, one per column.
    pub fn take_row(&self, row: i32) -> Vec<Option<StandardItem>> {
        self.take_rows_impl(row, 1).unwrap_or_default()
    }

    /// Removes column `column` and returns its (detached) items, one per row.
    pub fn take_column(&self, column: i32) -> Vec<Option<StandardItem>> {
        self.take_columns_impl(column, 1).unwrap_or_default()
    }

    /// Detaches and returns the child at `(row, column)`, leaving the cell empty.
    pub fn take_child(&self, row: i32, column: i32) -> Option<StandardItem> {
        let slot = self.0.borrow().slot(row, column)?;
        let item = self.0.borrow().children[slot].clone()?;
        let mut changed = ModelIndex::INVALID;
        if let Some(model) = self.model() {
            let item_index = model.index_from_item(&item);
            let (rows, columns, saved) = {
                let d = item.0.borrow();
                (d.rows, d.columns, d.children.clone())
            };
            if rows > 0 {
                model.begin_remove_rows(&item_index, 0, rows - 1);
                {
                    let mut d = item.0.borrow_mut();
                    d.rows = 0;
                    d.children.clear();
                }
                model.end_remove_rows();
            }
            if columns > 0 {
                model.begin_remove_columns(&item_index, 0, columns - 1);
                {
                    let mut d = item.0.borrow_mut();
                    d.columns = 0;
                    d.children.clear();
                }
                model.end_remove_columns();
            }
            {
                let mut d = item.0.borrow_mut();
                d.rows = rows;
                d.columns = columns;
                d.children = saved;
            }
            model.base.invalidate_persistent_index(&item_index);
            changed = item_index;
        }
        item.detach();
        self.0.borrow_mut().children[slot] = None;
        if changed.is_valid() {
            if let Some(model) = self.model() {
                model.emit_data_changed(&changed, &changed, &[]);
            }
        }
        Some(item)
    }

    /// Places `item` at `(row, column)`, growing the table as needed and
    /// replacing (and detaching) any previous child.
    pub fn set_child(&self, row: i32, column: i32, item: StandardItem) {
        if row < 0 || column < 0 || item == *self {
            return;
        }
        if self.row_count() <= row {
            self.set_row_count(row + 1);
        }
        if self.column_count() <= column {
            self.set_column_count(column + 1);
        }
        let Some(slot) = self.0.borrow().slot(row, column) else {
            return;
        };
        let old = self.0.borrow().children[slot].clone();
        if old.as_ref() == Some(&item) || !self.can_adopt(&item) {
            return;
        }
        let model = self.model();
        if let Some(model) = &model {
            model.begin_layout_change();
            if let Some(old) = &old {
                model.invalidate_subtree_persistent(old);
            }
        }
        self.adopt(&item, model.as_ref());
        self.0.borrow_mut().children[slot] = Some(item.clone());
        item.0.borrow().last_known.set(slot);
        if let Some(old) = old {
            old.detach();
        }
        if let Some(model) = &model {
            model.end_layout_change();
            model.item_data_changed(&item, &[]);
        }
    }

    /// Sorts the children (recursively) by the items in `column`.
    pub fn sort_children(&self, column: i32, order: SortOrder) {
        if column < 0 || self.row_count() == 0 {
            return;
        }
        let model = self.model();
        if let Some(model) = &model {
            model.begin_layout_change();
        }
        let role = model
            .as_ref()
            .map_or(ItemDataRole::Display, |m| m.sort_role.get());
        self.sort_recursive(column, order, model.as_deref(), role);
        if let Some(model) = &model {
            model.end_layout_change();
        }
    }

    fn sort_recursive(
        &self,
        column: i32,
        order: SortOrder,
        model: Option<&StdInner>,
        role: ItemDataRole,
    ) {
        let (rows, columns) = {
            let d = self.0.borrow();
            (d.rows as usize, d.columns as usize)
        };
        if column as usize >= columns {
            return;
        }
        let keys: Vec<Option<Variant>> = (0..rows)
            .map(|r| self.child(r as i32, column).map(|item| item.data(role)))
            .collect();
        let mut sortable: Vec<usize> = (0..rows).filter(|&r| keys[r].is_some()).collect();
        let unsortable = (0..rows).filter(|&r| keys[r].is_none());
        let key = |r: usize| keys[r].as_ref().expect("sortable rows have a key");
        stable_sort_by_less(&mut sortable, &mut |a, b| match order {
            SortOrder::Ascending => variant_less_than(key(*a), key(*b), CaseSensitivity::Sensitive),
            SortOrder::Descending => {
                variant_less_than(key(*b), key(*a), CaseSensitivity::Sensitive)
            }
        });
        sortable.extend(unsortable);
        let new_order = sortable;
        let children: Vec<StandardItem> = {
            let mut d = self.0.borrow_mut();
            let old = std::mem::take(&mut d.children);
            let mut table = Vec::with_capacity(old.len());
            for &old_row in &new_order {
                table.extend_from_slice(&old[old_row * columns..(old_row + 1) * columns]);
            }
            d.children = table;
            d.children.iter().flatten().cloned().collect()
        };
        self.refresh_hints(0);
        if let Some(model) = model {
            let mut forwarding = vec![0i32; rows];
            for (new_row, &old_row) in new_order.iter().enumerate() {
                forwarding[old_row] = new_row as i32;
            }
            let (parent_id, model_id) = (self.id(), model.base.id());
            model.base.remap_persistent_indexes(|idx| {
                (idx.model_id == model_id && idx.internal_id == parent_id)
                    .then(|| forwarding.get(idx.row as usize))
                    .flatten()
                    .map(|&row| ModelIndex::new(row, idx.column, parent_id, model_id))
            });
        }
        for child in children {
            child.sort_recursive(column, order, model, role);
        }
    }
}

/// Points `item` and its subtree at `model`, keeping the id registries in sync.
fn set_model_recursive(item: &StandardItem, model: Option<&Rc<StdInner>>) {
    let new_model = model.map_or_else(Weak::new, Rc::downgrade);
    let mut stack = vec![item.clone()];
    while let Some(current) = stack.pop() {
        let (old, id) = {
            let mut d = current.0.borrow_mut();
            let old = std::mem::replace(&mut d.model, new_model.clone());
            stack.extend(d.children.iter().flatten().cloned());
            (old, d.id)
        };
        if let Some(old) = old.upgrade() {
            old.items.borrow_mut().remove(&id);
        }
        if let Some(model) = model {
            model
                .items
                .borrow_mut()
                .insert(id, Rc::downgrade(&current.0));
        }
    }
}

struct StdInner {
    base: ModelBase,
    self_weak: Weak<StdInner>,
    root: StandardItem,
    items: RefCell<HashMap<u64, Weak<RefCell<ItemData>>>>,
    column_headers: RefCell<Vec<Option<StandardItem>>>,
    row_headers: RefCell<Vec<Option<StandardItem>>>,
    sort_role: Cell<ItemDataRole>,
    item_changed: Signal<StandardItem>,
}

impl StdInner {
    fn new() -> Rc<Self> {
        Rc::new_cyclic(|weak: &Weak<StdInner>| {
            let root = StandardItem::new();
            let root_id = {
                let mut d = root.0.borrow_mut();
                d.model = weak.clone();
                d.flags = ItemFlags::DROP_ENABLED;
                d.id
            };
            let mut items = HashMap::new();
            items.insert(root_id, Rc::downgrade(&root.0));
            StdInner {
                base: ModelBase::new(),
                self_weak: weak.clone(),
                root,
                items: RefCell::new(items),
                column_headers: RefCell::new(Vec::new()),
                row_headers: RefCell::new(Vec::new()),
                sort_role: Cell::new(ItemDataRole::Display),
                item_changed: Signal::new(),
            }
        })
    }

    fn item_by_id(&self, id: u64) -> Option<StandardItem> {
        self.items
            .borrow()
            .get(&id)
            .and_then(Weak::upgrade)
            .map(StandardItem)
    }

    fn owns(&self, item: &StandardItem) -> bool {
        std::ptr::eq(item.0.borrow().model.as_ptr(), self)
    }

    /// Item for `index` without creating missing cells; the invalid index maps to the root.
    fn lookup(&self, index: &ModelIndex) -> Option<StandardItem> {
        if !index.is_valid() {
            return Some(self.root.clone());
        }
        if index.model_id != self.base.id() {
            return None;
        }
        self.item_by_id(index.internal_id)?
            .child(index.row, index.column)
    }

    /// Item for `index`, lazily creating an empty item for an existing empty cell.
    fn item_from_index(&self, index: &ModelIndex) -> Option<StandardItem> {
        if !index.is_valid() || index.model_id != self.base.id() {
            return None;
        }
        let parent = self.item_by_id(index.internal_id)?;
        if let Some(item) = parent.child(index.row, index.column) {
            return Some(item);
        }
        let slot = parent.0.borrow().slot(index.row, index.column)?;
        let item = StandardItem::new();
        let model = self.self_weak.upgrade();
        parent.adopt(&item, model.as_ref());
        parent.0.borrow_mut().children[slot] = Some(item.clone());
        item.0.borrow().last_known.set(slot);
        Some(item)
    }

    fn index_from_item(&self, item: &StandardItem) -> ModelIndex {
        if !self.owns(item) {
            return ModelIndex::INVALID;
        }
        match item.position() {
            Some((parent, row, column)) => self.create_index(row, column, parent.id()),
            None => ModelIndex::INVALID,
        }
    }

    fn headers(&self, orientation: Orientation) -> &RefCell<Vec<Option<StandardItem>>> {
        match orientation {
            Orientation::Horizontal => &self.column_headers,
            Orientation::Vertical => &self.row_headers,
        }
    }

    fn section_count(&self, orientation: Orientation) -> i32 {
        match orientation {
            Orientation::Horizontal => self.root.column_count(),
            Orientation::Vertical => self.root.row_count(),
        }
    }

    fn root_sections_inserted(&self, orientation: Orientation, first: i32, count: i32) {
        let mut headers = self.headers(orientation).borrow_mut();
        let at = (first as usize).min(headers.len());
        headers.splice(at..at, std::iter::repeat_n(None, count as usize));
    }

    fn root_sections_removed(&self, orientation: Orientation, first: i32, count: i32) {
        let removed: Vec<StandardItem> = {
            let mut headers = self.headers(orientation).borrow_mut();
            let start = (first as usize).min(headers.len());
            let end = ((first + count) as usize).min(headers.len());
            headers.drain(start..end).flatten().collect()
        };
        for item in removed {
            set_model_recursive(&item, None);
        }
    }

    /// Qt's `itemChanged`: data change of a regular item or a header item.
    fn item_data_changed(&self, item: &StandardItem, roles: &[ItemDataRole]) {
        if item.0.borrow().parent.upgrade().is_none() {
            for orientation in [Orientation::Horizontal, Orientation::Vertical] {
                let section = self
                    .headers(orientation)
                    .borrow()
                    .iter()
                    .position(|h| h.as_ref() == Some(item));
                if let Some(section) = section {
                    let section = section as i32;
                    self.signals()
                        .header_data_changed
                        .emit(&(orientation, section, section));
                    return;
                }
            }
            return;
        }
        let index = self.index_from_item(item);
        if index.is_valid() {
            self.emit_data_changed(&index, &index, roles);
            self.item_changed.emit(item);
        }
    }

    /// Invalidates persistent indexes pointing at `item` or inside its subtree.
    fn invalidate_subtree_persistent(&self, item: &StandardItem) {
        let own_index = self.index_from_item(item);
        let mut ids = HashSet::new();
        let mut stack = vec![item.clone()];
        while let Some(current) = stack.pop() {
            let d = current.0.borrow();
            ids.insert(d.id);
            stack.extend(d.children.iter().flatten().cloned());
        }
        let model_id = self.base.id();
        self.base.remap_persistent_indexes(|idx| {
            (idx.model_id == model_id && (ids.contains(&idx.internal_id) || *idx == own_index))
                .then_some(ModelIndex::INVALID)
        });
    }

    fn header_item(&self, orientation: Orientation, section: i32) -> Option<StandardItem> {
        let section = usize::try_from(section).ok()?;
        self.headers(orientation)
            .borrow()
            .get(section)
            .cloned()
            .flatten()
    }

    fn set_header_item(&self, orientation: Orientation, section: i32, item: Option<StandardItem>) {
        if section < 0 {
            return;
        }
        if self.section_count(orientation) <= section {
            match orientation {
                Orientation::Horizontal => self.root.set_column_count(section + 1),
                Orientation::Vertical => self.root.set_row_count(section + 1),
            }
        }
        let old = self.header_item(orientation, section);
        if old == item {
            return;
        }
        if let Some(item) = &item {
            let attached = {
                let d = item.0.borrow();
                d.parent.upgrade().is_some() || d.model.upgrade().is_some()
            };
            if attached {
                return;
            }
            set_model_recursive(item, self.self_weak.upgrade().as_ref());
        }
        if let Some(old) = &old {
            set_model_recursive(old, None);
        }
        if let Some(slot) = self
            .headers(orientation)
            .borrow_mut()
            .get_mut(section as usize)
        {
            *slot = item;
        }
        self.signals()
            .header_data_changed
            .emit(&(orientation, section, section));
    }

    fn take_header_item(&self, orientation: Orientation, section: i32) -> Option<StandardItem> {
        let item = {
            let mut headers = self.headers(orientation).borrow_mut();
            headers.get_mut(usize::try_from(section).ok()?)?.take()?
        };
        set_model_recursive(&item, None);
        Some(item)
    }

    fn set_header_labels(&self, orientation: Orientation, labels: &[&str]) {
        let count = labels.len() as i32;
        if self.section_count(orientation) < count {
            match orientation {
                Orientation::Horizontal => self.root.set_column_count(count),
                Orientation::Vertical => self.root.set_row_count(count),
            }
        }
        for (section, label) in labels.iter().enumerate() {
            let section = section as i32;
            match self.header_item(orientation, section) {
                Some(item) => item.set_text(*label),
                None => {
                    let item = StandardItem::with_text(*label);
                    self.set_header_item(orientation, section, Some(item));
                }
            }
        }
    }

    fn clear(&self) {
        self.begin_reset_model();
        let children: Vec<StandardItem> = {
            let mut d = self.root.0.borrow_mut();
            d.rows = 0;
            d.columns = 0;
            std::mem::take(&mut d.children)
                .into_iter()
                .flatten()
                .collect()
        };
        for child in children {
            child.detach();
        }
        let headers: Vec<StandardItem> = self
            .column_headers
            .borrow_mut()
            .drain(..)
            .chain(self.row_headers.borrow_mut().drain(..))
            .flatten()
            .collect();
        for header in headers {
            set_model_recursive(&header, None);
        }
        self.end_reset_model();
    }
}

impl AbstractItemModel for StdInner {
    fn base(&self) -> &ModelBase {
        &self.base
    }

    fn index(&self, row: i32, column: i32, parent: &ModelIndex) -> ModelIndex {
        match self.lookup(parent) {
            Some(parent_item) if parent_item.0.borrow().slot(row, column).is_some() => {
                self.create_index(row, column, parent_item.id())
            }
            _ => ModelIndex::INVALID,
        }
    }

    fn parent(&self, child: &ModelIndex) -> ModelIndex {
        if !child.is_valid() || child.model_id != self.base.id() {
            return ModelIndex::INVALID;
        }
        self.item_by_id(child.internal_id)
            .map_or(ModelIndex::INVALID, |parent| self.index_from_item(&parent))
    }

    fn row_count(&self, parent: &ModelIndex) -> i32 {
        self.lookup(parent).map_or(0, |item| item.row_count())
    }

    fn column_count(&self, parent: &ModelIndex) -> i32 {
        self.lookup(parent).map_or(0, |item| item.column_count())
    }

    fn has_children(&self, parent: &ModelIndex) -> bool {
        self.lookup(parent).is_some_and(|item| item.has_children())
    }

    fn data(&self, index: &ModelIndex, role: ItemDataRole) -> Variant {
        if !index.is_valid() {
            return Variant::Invalid;
        }
        self.lookup(index)
            .map_or(Variant::Invalid, |item| item.data(role))
    }

    fn set_data(&self, index: &ModelIndex, value: Variant, role: ItemDataRole) -> bool {
        match self.item_from_index(index) {
            Some(item) => {
                item.set_data(value, role);
                true
            }
            None => false,
        }
    }

    fn header_data(&self, section: i32, orientation: Orientation, role: ItemDataRole) -> Variant {
        if section < 0 || section >= self.section_count(orientation) {
            return Variant::Invalid;
        }
        match self.header_item(orientation, section) {
            Some(item) => item.data(role),
            None => default_header_data(section, role),
        }
    }

    fn set_header_data(
        &self,
        section: i32,
        orientation: Orientation,
        value: Variant,
        role: ItemDataRole,
    ) -> bool {
        if section < 0 || section >= self.section_count(orientation) {
            return false;
        }
        let item = match self.header_item(orientation, section) {
            Some(item) => item,
            None => {
                let item = StandardItem::new();
                set_model_recursive(&item, self.self_weak.upgrade().as_ref());
                if let Some(slot) = self
                    .headers(orientation)
                    .borrow_mut()
                    .get_mut(section as usize)
                {
                    *slot = Some(item.clone());
                }
                item
            }
        };
        item.set_data(value, role);
        true
    }

    fn flags(&self, index: &ModelIndex) -> ItemFlags {
        if !index.is_valid() {
            return self.root.flags();
        }
        match self.lookup(index) {
            Some(item) => item.flags(),
            None if self.index(index.row, index.column, &self.parent(index)) == *index => {
                DEFAULT_ITEM_FLAGS
            }
            None => ItemFlags::NONE,
        }
    }

    fn insert_rows(&self, row: i32, count: i32, parent: &ModelIndex) -> bool {
        let item = if parent.is_valid() {
            self.item_from_index(parent)
        } else {
            Some(self.root.clone())
        };
        item.is_some_and(|item| item.insert_rows(row, count))
    }

    fn remove_rows(&self, row: i32, count: i32, parent: &ModelIndex) -> bool {
        self.lookup(parent)
            .is_some_and(|item| item.take_rows_impl(row, count).is_some())
    }

    fn insert_columns(&self, column: i32, count: i32, parent: &ModelIndex) -> bool {
        let item = if parent.is_valid() {
            self.item_from_index(parent)
        } else {
            Some(self.root.clone())
        };
        item.is_some_and(|item| item.insert_columns(column, count))
    }

    fn remove_columns(&self, column: i32, count: i32, parent: &ModelIndex) -> bool {
        self.lookup(parent)
            .is_some_and(|item| item.take_columns_impl(column, count).is_some())
    }

    fn sort(&self, column: i32, order: SortOrder) {
        self.root.sort_children(column, order);
    }
}

/// Tree/table model of [`StandardItem`]s (`QStandardItemModel`).
pub struct StandardItemModel {
    inner: Rc<StdInner>,
    /// Emitted with the item whenever an item's data or flags change.
    pub item_changed: Signal<StandardItem>,
}

delegate_item_model!(StandardItemModel, inner);

impl Default for StandardItemModel {
    fn default() -> Self {
        Self::new()
    }
}

impl StandardItemModel {
    pub fn new() -> Self {
        let inner = StdInner::new();
        let item_changed = inner.item_changed.clone();
        Self {
            inner,
            item_changed,
        }
    }

    /// Model with an empty `rows` x `columns` table.
    pub fn with_size(rows: i32, columns: i32) -> Self {
        let model = Self::new();
        model.inner.root.insert_columns(0, columns);
        model.inner.root.insert_rows(0, rows);
        model
    }

    /// The hidden root whose children are the top-level items.
    pub fn invisible_root_item(&self) -> StandardItem {
        self.inner.root.clone()
    }

    /// Top-level item at `(row, column)`.
    pub fn item(&self, row: i32, column: i32) -> Option<StandardItem> {
        self.inner.root.child(row, column)
    }

    /// Places a top-level item at `(row, column)`.
    pub fn set_item(&self, row: i32, column: i32, item: StandardItem) {
        self.inner.root.set_child(row, column, item);
    }

    /// Item for `index`; an existing but empty cell gets a fresh item.
    pub fn item_from_index(&self, index: &ModelIndex) -> Option<StandardItem> {
        self.inner.item_from_index(index)
    }

    pub fn index_from_item(&self, item: &StandardItem) -> ModelIndex {
        self.inner.index_from_item(item)
    }

    pub fn append_row(&self, items: Vec<StandardItem>) {
        self.inner.root.append_row(items);
    }

    pub fn append_column(&self, items: Vec<StandardItem>) {
        self.inner.root.append_column(items);
    }

    /// Inserts a top-level row of `items` at `row`.
    pub fn insert_row_items(&self, row: i32, items: Vec<StandardItem>) {
        self.inner.root.insert_row(row, items);
    }

    /// Inserts a top-level column of `items` at `column`.
    pub fn insert_column_items(&self, column: i32, items: Vec<StandardItem>) {
        self.inner.root.insert_column(column, items);
    }

    pub fn take_item(&self, row: i32, column: i32) -> Option<StandardItem> {
        self.inner.root.take_child(row, column)
    }

    pub fn take_row(&self, row: i32) -> Vec<Option<StandardItem>> {
        self.inner.root.take_row(row)
    }

    pub fn take_column(&self, column: i32) -> Vec<Option<StandardItem>> {
        self.inner.root.take_column(column)
    }

    pub fn set_row_count(&self, rows: i32) {
        self.inner.root.set_row_count(rows);
    }

    pub fn set_column_count(&self, columns: i32) {
        self.inner.root.set_column_count(columns);
    }

    /// Removes all items and headers, resetting the model.
    pub fn clear(&self) {
        self.inner.clear();
    }

    /// Items in `column` whose display text matches `text` (`findItems`).
    pub fn find_items(&self, text: &str, flags: MatchFlags, column: i32) -> Vec<StandardItem> {
        let start = self.inner.index(0, column, &ModelIndex::INVALID);
        self.inner
            .match_indexes(
                &start,
                ItemDataRole::Display,
                &Variant::String(text.to_string()),
                -1,
                flags,
            )
            .iter()
            .filter_map(|index| self.inner.item_from_index(index))
            .collect()
    }

    pub fn sort_role(&self) -> ItemDataRole {
        self.inner.sort_role.get()
    }

    pub fn set_sort_role(&self, role: ItemDataRole) {
        self.inner.sort_role.set(role);
    }

    pub fn horizontal_header_item(&self, column: i32) -> Option<StandardItem> {
        self.inner.header_item(Orientation::Horizontal, column)
    }

    pub fn set_horizontal_header_item(&self, column: i32, item: StandardItem) {
        self.inner
            .set_header_item(Orientation::Horizontal, column, Some(item));
    }

    pub fn take_horizontal_header_item(&self, column: i32) -> Option<StandardItem> {
        self.inner.take_header_item(Orientation::Horizontal, column)
    }

    pub fn set_horizontal_header_labels(&self, labels: &[&str]) {
        self.inner
            .set_header_labels(Orientation::Horizontal, labels);
    }

    pub fn vertical_header_item(&self, row: i32) -> Option<StandardItem> {
        self.inner.header_item(Orientation::Vertical, row)
    }

    pub fn set_vertical_header_item(&self, row: i32, item: StandardItem) {
        self.inner
            .set_header_item(Orientation::Vertical, row, Some(item));
    }

    pub fn take_vertical_header_item(&self, row: i32) -> Option<StandardItem> {
        self.inner.take_header_item(Orientation::Vertical, row)
    }

    pub fn set_vertical_header_labels(&self, labels: &[&str]) {
        self.inner.set_header_labels(Orientation::Vertical, labels);
    }
}

/// Canonical Qt aliases.
pub type QStandardItemModel = StandardItemModel;
pub type QStandardItem = StandardItem;
