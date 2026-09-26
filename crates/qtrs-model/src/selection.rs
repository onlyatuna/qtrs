//! Item selections (`QItemSelectionRange`, `QItemSelection`) and the
//! selection model shared between views (`QItemSelectionModel`).

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::ops::{Deref, DerefMut};
use std::rc::{Rc, Weak};

use qtrs_core::Signal;

use crate::index::{ModelIndex, PersistentModelIndex};
use crate::model::{AbstractItemModel, ModelConnection, ModelEvent, ModelListener, SharedModel};
use crate::role::{impl_flags, ItemFlags};

/// How a selection request updates the selection (`QItemSelectionModel::SelectionFlags`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct SelectionFlags(pub u32);

impl SelectionFlags {
    pub const NO_UPDATE: Self = Self(0);
    pub const CLEAR: Self = Self(1);
    pub const SELECT: Self = Self(2);
    pub const DESELECT: Self = Self(4);
    pub const TOGGLE: Self = Self(8);
    pub const CURRENT: Self = Self(16);
    pub const ROWS: Self = Self(32);
    pub const COLUMNS: Self = Self(64);
    pub const SELECT_CURRENT: Self = Self(2 | 16);
    pub const TOGGLE_CURRENT: Self = Self(8 | 16);
    pub const CLEAR_AND_SELECT: Self = Self(1 | 2);
}
impl_flags!(SelectionFlags);

fn selectable_and_enabled(flags: ItemFlags) -> bool {
    flags.contains(ItemFlags::SELECTABLE | ItemFlags::ENABLED)
}

/// Rectangular block of items sharing a parent (`QItemSelectionRange`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ItemSelectionRange {
    top_left: ModelIndex,
    bottom_right: ModelIndex,
    parent: ModelIndex,
}

impl ItemSelectionRange {
    /// Range spanning `top_left..=bottom_right`; both must share a parent in
    /// `model`, otherwise the range is invalid.
    pub fn new<M: AbstractItemModel + ?Sized>(
        model: &M,
        top_left: ModelIndex,
        bottom_right: ModelIndex,
    ) -> Self {
        if !top_left.is_valid()
            || !bottom_right.is_valid()
            || top_left.model_id != bottom_right.model_id
        {
            return Self::default();
        }
        let parent = model.parent(&top_left);
        if model.parent(&bottom_right) != parent {
            return Self::default();
        }
        Self {
            top_left,
            bottom_right,
            parent,
        }
    }

    /// Range covering the single `index`.
    pub fn from_index<M: AbstractItemModel + ?Sized>(model: &M, index: ModelIndex) -> Self {
        Self::new(model, index, index)
    }

    /// Range from already known corners and parent.
    pub fn from_parts(top_left: ModelIndex, bottom_right: ModelIndex, parent: ModelIndex) -> Self {
        Self {
            top_left,
            bottom_right,
            parent,
        }
    }

    pub fn top_left(&self) -> ModelIndex {
        self.top_left
    }

    pub fn bottom_right(&self) -> ModelIndex {
        self.bottom_right
    }

    pub fn parent(&self) -> ModelIndex {
        self.parent
    }

    pub fn top(&self) -> i32 {
        self.top_left.row
    }

    pub fn left(&self) -> i32 {
        self.top_left.column
    }

    pub fn bottom(&self) -> i32 {
        self.bottom_right.row
    }

    pub fn right(&self) -> i32 {
        self.bottom_right.column
    }

    pub fn width(&self) -> i32 {
        self.right() - self.left() + 1
    }

    pub fn height(&self) -> i32 {
        self.bottom() - self.top() + 1
    }

    pub fn model_id(&self) -> u64 {
        self.top_left.model_id
    }

    pub fn is_valid(&self) -> bool {
        self.top_left.is_valid()
            && self.bottom_right.is_valid()
            && self.top_left.model_id == self.bottom_right.model_id
            && self.top() <= self.bottom()
            && self.left() <= self.right()
    }

    /// `true` if `(row, column)` below `parent` lies inside the range.
    pub fn contains_at(&self, row: i32, column: i32, parent: &ModelIndex) -> bool {
        self.parent == *parent
            && self.top() <= row
            && row <= self.bottom()
            && self.left() <= column
            && column <= self.right()
    }

    /// `true` if `index` lies inside the range.
    pub fn contains<M: AbstractItemModel + ?Sized>(&self, index: &ModelIndex, model: &M) -> bool {
        index.is_valid()
            && index.model_id == self.model_id()
            && self.contains_at(index.row, index.column, &model.parent(index))
    }

    pub fn intersects(&self, other: &ItemSelectionRange) -> bool {
        self.model_id() == other.model_id()
            && ((self.top() <= other.top() && self.bottom() >= other.top())
                || (self.top() >= other.top() && self.top() <= other.bottom()))
            && ((self.left() <= other.left() && self.right() >= other.left())
                || (self.left() >= other.left() && self.left() <= other.right()))
            && self.parent == other.parent
            && self.is_valid()
            && other.is_valid()
    }

    /// Overlap of both ranges (invalid when they do not overlap).
    pub fn intersected<M: AbstractItemModel + ?Sized>(
        &self,
        other: &ItemSelectionRange,
        model: &M,
    ) -> ItemSelectionRange {
        if self.model_id() != other.model_id() || self.parent != other.parent {
            return Self::default();
        }
        let top_left = model.index(
            self.top().max(other.top()),
            self.left().max(other.left()),
            &other.parent,
        );
        let bottom_right = model.index(
            self.bottom().min(other.bottom()),
            self.right().min(other.right()),
            &other.parent,
        );
        Self::from_parts(top_left, bottom_right, other.parent)
    }

    /// `true` if the range holds no selectable, enabled item.
    pub fn is_empty<M: AbstractItemModel + ?Sized>(&self, model: &M) -> bool {
        if !self.is_valid() {
            return true;
        }
        for column in self.left()..=self.right() {
            for row in self.top()..=self.bottom() {
                if selectable_and_enabled(model.flags(&model.index(row, column, &self.parent))) {
                    return false;
                }
            }
        }
        true
    }

    /// Selectable, enabled indexes of the range in row-major order.
    pub fn indexes<M: AbstractItemModel + ?Sized>(&self, model: &M) -> Vec<ModelIndex> {
        let mut out = Vec::new();
        self.append_indexes(model, &mut out);
        out
    }

    fn append_indexes<M: AbstractItemModel + ?Sized>(&self, model: &M, out: &mut Vec<ModelIndex>) {
        if !self.is_valid() {
            return;
        }
        for row in self.top()..=self.bottom() {
            for column in self.left()..=self.right() {
                let index = model.index(row, column, &self.parent);
                if index.is_valid() && selectable_and_enabled(model.flags(&index)) {
                    out.push(index);
                }
            }
        }
    }
}

/// List of selection ranges (`QItemSelection`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ItemSelection {
    ranges: Vec<ItemSelectionRange>,
}

impl Deref for ItemSelection {
    type Target = Vec<ItemSelectionRange>;
    fn deref(&self) -> &Self::Target {
        &self.ranges
    }
}

impl DerefMut for ItemSelection {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.ranges
    }
}

impl FromIterator<ItemSelectionRange> for ItemSelection {
    fn from_iter<I: IntoIterator<Item = ItemSelectionRange>>(iter: I) -> Self {
        Self {
            ranges: iter.into_iter().collect(),
        }
    }
}

impl IntoIterator for ItemSelection {
    type Item = ItemSelectionRange;
    type IntoIter = std::vec::IntoIter<ItemSelectionRange>;
    fn into_iter(self) -> Self::IntoIter {
        self.ranges.into_iter()
    }
}

impl ItemSelection {
    pub fn new() -> Self {
        Self::default()
    }

    /// Selection holding the block `top_left..=bottom_right`.
    pub fn from_range<M: AbstractItemModel + ?Sized>(
        model: &M,
        top_left: ModelIndex,
        bottom_right: ModelIndex,
    ) -> Self {
        let mut selection = Self::new();
        selection.select(model, top_left, bottom_right);
        selection
    }

    /// Adds the block spanned by two corner indexes (in any order).
    pub fn select<M: AbstractItemModel + ?Sized>(
        &mut self,
        model: &M,
        top_left: ModelIndex,
        bottom_right: ModelIndex,
    ) {
        if !top_left.is_valid()
            || !bottom_right.is_valid()
            || top_left.model_id != bottom_right.model_id
        {
            return;
        }
        let parent = model.parent(&top_left);
        if model.parent(&bottom_right) != parent {
            return;
        }
        if top_left.row > bottom_right.row || top_left.column > bottom_right.column {
            let top = top_left.row.min(bottom_right.row);
            let bottom = top_left.row.max(bottom_right.row);
            let left = top_left.column.min(bottom_right.column);
            let right = top_left.column.max(bottom_right.column);
            let tl = model.index(top, left, &parent);
            let br = model.index(bottom, right, &parent);
            self.ranges
                .push(ItemSelectionRange::from_parts(tl, br, parent));
        } else {
            self.ranges.push(ItemSelectionRange::from_parts(
                top_left,
                bottom_right,
                parent,
            ));
        }
    }

    /// `true` if `index` is selectable, enabled and inside some range.
    pub fn contains<M: AbstractItemModel + ?Sized>(&self, model: &M, index: &ModelIndex) -> bool {
        if !index.is_valid() || !selectable_and_enabled(model.flags(index)) {
            return false;
        }
        let parent = model.parent(index);
        self.ranges.iter().any(|r| {
            r.model_id() == index.model_id && r.contains_at(index.row, index.column, &parent)
        })
    }

    /// Selectable, enabled indexes of all ranges.
    pub fn indexes<M: AbstractItemModel + ?Sized>(&self, model: &M) -> Vec<ModelIndex> {
        let mut out = Vec::new();
        for range in &self.ranges {
            range.append_indexes(model, &mut out);
        }
        out
    }

    /// Merges `other` into this selection according to `command`
    /// (select, deselect or toggle).
    pub fn merge<M: AbstractItemModel + ?Sized>(
        &mut self,
        other: &ItemSelection,
        command: SelectionFlags,
        model: &M,
    ) {
        if other.is_empty()
            || !command.intersects(
                SelectionFlags::SELECT | SelectionFlags::DESELECT | SelectionFlags::TOGGLE,
            )
        {
            return;
        }
        let mut new_selection: Vec<ItemSelectionRange> = Vec::new();
        let mut intersections: Vec<ItemSelectionRange> = Vec::new();
        for range in other.iter().filter(|r| r.is_valid()) {
            new_selection.push(*range);
            for existing in &self.ranges {
                if range.intersects(existing) {
                    intersections.push(existing.intersected(range, model));
                }
            }
        }
        let toggle = command.contains(SelectionFlags::TOGGLE);
        for intersection in &intersections {
            split_all(&mut self.ranges, intersection, model);
            if toggle {
                split_all(&mut new_selection, intersection, model);
            }
        }
        if !command.contains(SelectionFlags::DESELECT) {
            self.ranges.extend(new_selection);
        }
    }

    /// Appends to `result` the parts of `range` not covered by `other`.
    pub fn split<M: AbstractItemModel + ?Sized>(
        range: &ItemSelectionRange,
        other: &ItemSelectionRange,
        model: &M,
        result: &mut Vec<ItemSelectionRange>,
    ) {
        if range.parent != other.parent || range.model_id() != other.model_id() {
            return;
        }
        let parent = other.parent;
        let (mut top, left, mut bottom, right) =
            (range.top(), range.left(), range.bottom(), range.right());
        let (other_top, other_left, other_bottom, other_right) =
            (other.top(), other.left(), other.bottom(), other.right());
        let mut push = |t: i32, l: i32, b: i32, r: i32| {
            result.push(ItemSelectionRange::from_parts(
                model.index(t, l, &parent),
                model.index(b, r, &parent),
                parent,
            ));
        };
        if other_top > top {
            push(top, left, other_top - 1, right);
            top = other_top;
        }
        if other_bottom < bottom {
            push(other_bottom + 1, left, bottom, right);
            bottom = other_bottom;
        }
        if other_left > left {
            push(top, left, bottom, other_left - 1);
        }
        if other_right < right {
            push(top, other_right + 1, bottom, right);
        }
    }
}

/// Replaces every range of `ranges` that intersects `cut` by its remainder.
fn split_all<M: AbstractItemModel + ?Sized>(
    ranges: &mut Vec<ItemSelectionRange>,
    cut: &ItemSelectionRange,
    model: &M,
) {
    let mut i = 0;
    while i < ranges.len() {
        if ranges[i].intersects(cut) {
            let range = ranges.remove(i);
            ItemSelection::split(&range, cut, model, ranges);
        } else {
            i += 1;
        }
    }
}

/// Computes `(selected, deselected)` between two selections, or `None` when
/// nothing changed (`QItemSelectionModel::emitSelectionChanged`).
fn selection_difference<M: AbstractItemModel + ?Sized>(
    model: &M,
    new_selection: &ItemSelection,
    old_selection: &ItemSelection,
) -> Option<(ItemSelection, ItemSelection)> {
    if (old_selection.is_empty() && new_selection.is_empty()) || old_selection == new_selection {
        return None;
    }
    if old_selection.is_empty() || new_selection.is_empty() {
        return Some((new_selection.clone(), old_selection.clone()));
    }
    let mut deselected = old_selection.clone();
    let mut selected = new_selection.clone();
    let mut o = 0;
    while o < deselected.len() {
        let mut advance = true;
        let mut s = 0;
        while s < selected.len() && o < deselected.len() {
            if deselected[o] == selected[s] {
                deselected.remove(o);
                selected.remove(s);
                advance = false;
            } else {
                s += 1;
            }
        }
        if advance {
            o += 1;
        }
    }
    let mut intersections = Vec::new();
    for d in deselected.iter() {
        for s in selected.iter() {
            if d.intersects(s) {
                intersections.push(d.intersected(s, model));
            }
        }
    }
    for intersection in &intersections {
        split_all(&mut deselected, intersection, model);
        split_all(&mut selected, intersection, model);
    }
    (!selected.is_empty() || !deselected.is_empty()).then_some((selected, deselected))
}

/// Expands ranges to whole rows and/or columns.
fn expand_selection<M: AbstractItemModel + ?Sized>(
    model: &M,
    selection: &ItemSelection,
    command: SelectionFlags,
) -> ItemSelection {
    let mut expanded = ItemSelection::new();
    if command.contains(SelectionFlags::ROWS) {
        for range in selection.iter() {
            let parent = range.parent();
            let columns = model.column_count(&parent);
            let tl = model.index(range.top(), 0, &parent);
            let br = model.index(range.bottom(), columns - 1, &parent);
            expanded.merge(
                &ItemSelection::from_range(model, tl, br),
                SelectionFlags::SELECT,
                model,
            );
        }
    }
    if command.contains(SelectionFlags::COLUMNS) {
        for range in selection.iter() {
            let parent = range.parent();
            let rows = model.row_count(&parent);
            let tl = model.index(0, range.left(), &parent);
            let br = model.index(rows - 1, range.right(), &parent);
            expanded.merge(
                &ItemSelection::from_range(model, tl, br),
                SelectionFlags::SELECT,
                model,
            );
        }
    }
    expanded
}

/// Selection range tracked through model changes.
#[derive(Clone)]
struct PersistentRange {
    top_left: PersistentModelIndex,
    bottom_right: PersistentModelIndex,
}

fn to_plain(model: &dyn AbstractItemModel, ranges: &[PersistentRange]) -> ItemSelection {
    ranges
        .iter()
        .map(|r| ItemSelectionRange::new(model, r.top_left.index(), r.bottom_right.index()))
        .filter(ItemSelectionRange::is_valid)
        .collect()
}

fn to_persistent(model: &dyn AbstractItemModel, selection: &ItemSelection) -> Vec<PersistentRange> {
    selection
        .iter()
        .filter(|r| r.is_valid())
        .map(|r| PersistentRange {
            top_left: PersistentModelIndex::new(model, &r.top_left),
            bottom_right: PersistentModelIndex::new(model, &r.bottom_right),
        })
        .collect()
}

/// Merges `(parent, index)` pairs, sorted by parent then position, back into ranges.
fn merge_indexes(indexes: &[(ModelIndex, ModelIndex)]) -> ItemSelection {
    let mut column_spans: Vec<ItemSelectionRange> = Vec::new();
    let mut i = 0;
    while i < indexes.len() {
        let (parent, top_left) = indexes[i];
        let mut bottom_right = top_left;
        i += 1;
        while i < indexes.len() {
            let (next_parent, next) = indexes[i];
            if next_parent == parent
                && next.row == bottom_right.row
                && next.column == bottom_right.column + 1
            {
                bottom_right = next;
                i += 1;
            } else {
                break;
            }
        }
        column_spans.push(ItemSelectionRange::from_parts(
            top_left,
            bottom_right,
            parent,
        ));
    }
    let mut row_spans = ItemSelection::new();
    let mut i = 0;
    while i < column_spans.len() {
        let first = column_spans[i];
        let mut bottom_right = first.bottom_right();
        let mut previous_top_left = first.top_left();
        i += 1;
        while i < column_spans.len() {
            let next = column_spans[i];
            if next.parent() == first.parent()
                && next.left() == previous_top_left.column
                && next.right() == bottom_right.column
                && next.top() == previous_top_left.row + 1
                && next.bottom() == bottom_right.row + 1
            {
                bottom_right = next.bottom_right();
                previous_top_left = next.top_left();
                i += 1;
            } else {
                break;
            }
        }
        row_spans.push(ItemSelectionRange::from_parts(
            first.top_left(),
            bottom_right,
            first.parent(),
        ));
    }
    row_spans
}

struct SelectionInner {
    self_weak: Weak<SelectionInner>,
    model: RefCell<Option<SharedModel>>,
    connection: RefCell<Option<ModelConnection>>,
    ranges: RefCell<Vec<PersistentRange>>,
    current_selection: RefCell<Vec<PersistentRange>>,
    current_command: Cell<SelectionFlags>,
    current_index: RefCell<PersistentModelIndex>,
    saved_indexes: RefCell<Vec<PersistentModelIndex>>,
    saved_current_indexes: RefCell<Vec<PersistentModelIndex>>,
    selection_changed: Signal<(ItemSelection, ItemSelection)>,
    current_changed: Signal<(ModelIndex, ModelIndex)>,
    current_row_changed: Signal<(ModelIndex, ModelIndex)>,
    current_column_changed: Signal<(ModelIndex, ModelIndex)>,
    model_changed: Signal<()>,
}

impl SelectionInner {
    fn model(&self) -> Option<SharedModel> {
        self.model.borrow().clone()
    }

    fn set_model(&self, model: Option<SharedModel>) {
        let same = match (&*self.model.borrow(), &model) {
            (Some(a), Some(b)) => Rc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        };
        if same {
            return;
        }
        if self.model.borrow().is_some() {
            self.reset();
            *self.connection.borrow_mut() = None;
        }
        *self.model.borrow_mut() = model.clone();
        if let Some(model) = model {
            let listener: Weak<dyn ModelListener> = self.self_weak.clone();
            let connection = ModelConnection::new(model.borrow().signals(), listener);
            *self.connection.borrow_mut() = Some(connection);
        }
        self.model_changed.emit(&());
    }

    /// Silently clears selection and current index (`reset`).
    fn reset(&self) {
        self.ranges.borrow_mut().clear();
        self.current_selection.borrow_mut().clear();
        self.current_command.set(SelectionFlags::NO_UPDATE);
        *self.current_index.borrow_mut() = PersistentModelIndex::default();
        self.saved_indexes.borrow_mut().clear();
        self.saved_current_indexes.borrow_mut().clear();
    }

    /// Merges the in-progress selection into the committed ranges.
    fn finalize(&self, model: &dyn AbstractItemModel) {
        if self.current_selection.borrow().is_empty() {
            return;
        }
        let current = to_plain(model, &self.current_selection.borrow());
        let mut ranges = to_plain(model, &self.ranges.borrow());
        ranges.merge(&current, self.current_command.get(), model);
        *self.ranges.borrow_mut() = to_persistent(model, &ranges);
        self.current_selection.borrow_mut().clear();
    }

    /// Effective selection (committed ranges merged with the current one).
    fn effective_selection(&self, model: &dyn AbstractItemModel) -> ItemSelection {
        let mut selection = to_plain(model, &self.ranges.borrow());
        let current = to_plain(model, &self.current_selection.borrow());
        selection.merge(&current, self.current_command.get(), model);
        selection.retain(ItemSelectionRange::is_valid);
        selection
    }

    fn select(&self, selection: &ItemSelection, command: SelectionFlags) {
        if command == SelectionFlags::NO_UPDATE {
            return;
        }
        let Some(shared) = self.model() else {
            return;
        };
        let change = {
            let model_ref = shared.borrow();
            let model: &dyn AbstractItemModel = &*model_ref;
            let model_id = model.model_id();
            let mut selection: ItemSelection = selection
                .iter()
                .filter(|r| r.is_valid() && r.model_id() == model_id)
                .copied()
                .collect();
            let mut ranges = to_plain(model, &self.ranges.borrow());
            let mut current = to_plain(model, &self.current_selection.borrow());
            let mut command_in_progress = self.current_command.get();

            let mut old = ranges.clone();
            old.merge(&current, command_in_progress, model);

            if command.intersects(SelectionFlags::ROWS | SelectionFlags::COLUMNS) {
                selection = expand_selection(model, &selection, command);
            }
            if command.contains(SelectionFlags::CLEAR) {
                ranges.clear();
                current.clear();
            }
            if !command.contains(SelectionFlags::CURRENT) {
                ranges.merge(&current, command_in_progress, model);
                current.clear();
            }
            if command.intersects(
                SelectionFlags::TOGGLE | SelectionFlags::SELECT | SelectionFlags::DESELECT,
            ) {
                command_in_progress = command;
                current = selection;
            }
            self.current_command.set(command_in_progress);

            let mut new_selection = ranges.clone();
            new_selection.merge(&current, command_in_progress, model);
            *self.ranges.borrow_mut() = to_persistent(model, &ranges);
            *self.current_selection.borrow_mut() = to_persistent(model, &current);
            selection_difference(model, &new_selection, &old)
        };
        if let Some(change) = change {
            self.selection_changed.emit(&change);
        }
    }

    fn select_index(&self, index: &ModelIndex, command: SelectionFlags) {
        let Some(shared) = self.model() else {
            return;
        };
        let selection = ItemSelection::from_range(&*shared.borrow(), *index, *index);
        self.select(&selection, command);
    }

    fn emit_current(&self, current: ModelIndex, previous: ModelIndex, model: &SharedModel) {
        let (row_changed, column_changed) = {
            let m = model.borrow();
            let same_parent = current.parent(&*m) == previous.parent(&*m);
            (
                current.row != previous.row || !same_parent,
                current.column != previous.column || !same_parent,
            )
        };
        self.current_changed.emit(&(current, previous));
        if row_changed {
            self.current_row_changed.emit(&(current, previous));
        }
        if column_changed {
            self.current_column_changed.emit(&(current, previous));
        }
    }

    fn set_current_index(&self, index: &ModelIndex, command: SelectionFlags) {
        let Some(shared) = self.model() else {
            return;
        };
        let previous = self.current_index.borrow().index();
        if *index == previous {
            if command != SelectionFlags::NO_UPDATE {
                self.select_index(index, command);
            }
            return;
        }
        *self.current_index.borrow_mut() = PersistentModelIndex::new(&*shared.borrow(), index);
        if command != SelectionFlags::NO_UPDATE {
            self.select_index(index, command);
        }
        let current = self.current_index.borrow().index();
        self.emit_current(current, previous, &shared);
    }

    fn clear_current_index(&self) {
        let previous = std::mem::take(&mut *self.current_index.borrow_mut()).index();
        if previous.is_valid() {
            self.current_changed.emit(&(ModelIndex::INVALID, previous));
            self.current_row_changed
                .emit(&(ModelIndex::INVALID, previous));
            self.current_column_changed
                .emit(&(ModelIndex::INVALID, previous));
        }
    }

    fn clear_selection(&self) {
        if self.ranges.borrow().is_empty() && self.current_selection.borrow().is_empty() {
            return;
        }
        self.select(&ItemSelection::new(), SelectionFlags::CLEAR);
    }

    fn is_selected(&self, model: &dyn AbstractItemModel, index: &ModelIndex) -> bool {
        if !index.is_valid() || index.model_id != model.model_id() {
            return false;
        }
        let parent = model.parent(index);
        let in_ranges = |ranges: &[PersistentRange]| {
            to_plain(model, ranges)
                .iter()
                .any(|r| r.contains_at(index.row, index.column, &parent))
        };
        let mut selected = in_ranges(&self.ranges.borrow());
        let current = self.current_selection.borrow();
        if !current.is_empty() {
            let command = self.current_command.get();
            let in_current = in_ranges(&current);
            if command.contains(SelectionFlags::DESELECT) && selected {
                selected = !in_current;
            } else if command.contains(SelectionFlags::TOGGLE) {
                selected ^= in_current;
            } else if command.contains(SelectionFlags::SELECT) && !selected {
                selected = in_current;
            }
        }
        selected && model.flags(index).contains(ItemFlags::SELECTABLE)
    }

    // ----- model change handlers -----------------------------------------------

    fn rows_about_to_be_removed(
        &self,
        model: &dyn AbstractItemModel,
        parent: &ModelIndex,
        start: i32,
        end: i32,
    ) {
        self.finalize(model);
        let mut current_change = None;
        let current = self.current_index.borrow().index();
        if current.is_valid()
            && *parent == model.parent(&current)
            && current.row >= start
            && current.row <= end
        {
            let replacement = if start > 0 {
                model.index(start - 1, current.column, parent)
            } else if end < model.row_count(parent) - 1 {
                model.index(end + 1, current.column, parent)
            } else {
                ModelIndex::INVALID
            };
            *self.current_index.borrow_mut() = PersistentModelIndex::new(model, &replacement);
            current_change = Some((replacement, current));
        }

        let ranges = to_plain(model, &self.ranges.borrow());
        let mut kept: Vec<ItemSelectionRange> = Vec::new();
        let mut deselected = ItemSelection::new();
        let mut new_parts: Vec<ItemSelectionRange> = Vec::new();
        let mut indexes_changed = false;
        for range in ranges.iter() {
            let r = *range;
            if r.parent() != *parent {
                let mut ancestor = r.parent();
                while ancestor.is_valid() && model.parent(&ancestor) != *parent {
                    ancestor = model.parent(&ancestor);
                }
                if ancestor.is_valid() && start <= ancestor.row && ancestor.row <= end {
                    deselected.push(r);
                } else {
                    if ancestor.is_valid() && end < ancestor.row {
                        indexes_changed = true;
                    }
                    kept.push(r);
                }
            } else if start <= r.bottom() && r.bottom() <= end && start <= r.top() && r.top() <= end
            {
                deselected.push(r);
            } else if start <= r.top() && r.top() <= end {
                deselected.push(ItemSelectionRange::from_parts(
                    r.top_left(),
                    model.index(end, r.right(), parent),
                    *parent,
                ));
                kept.push(ItemSelectionRange::from_parts(
                    model.index(end + 1, r.left(), parent),
                    r.bottom_right(),
                    *parent,
                ));
            } else if start <= r.bottom() && r.bottom() <= end {
                deselected.push(ItemSelectionRange::from_parts(
                    model.index(start, r.left(), parent),
                    r.bottom_right(),
                    *parent,
                ));
                kept.push(ItemSelectionRange::from_parts(
                    r.top_left(),
                    model.index(start - 1, r.right(), parent),
                    *parent,
                ));
            } else if r.top() < start && end < r.bottom() {
                let removed = ItemSelectionRange::from_parts(
                    model.index(start, r.left(), parent),
                    model.index(end, r.right(), parent),
                    *parent,
                );
                deselected.push(removed);
                ItemSelection::split(&r, &removed, model, &mut new_parts);
            } else {
                if end < r.top() {
                    indexes_changed = true;
                }
                kept.push(r);
            }
        }
        kept.extend(new_parts);
        *self.ranges.borrow_mut() = to_persistent(model, &kept.into_iter().collect());

        if let Some((current, previous)) = current_change {
            self.current_changed.emit(&(current, previous));
            self.current_row_changed.emit(&(current, previous));
            if current.column != previous.column {
                self.current_column_changed.emit(&(current, previous));
            }
        }
        if !deselected.is_empty() || indexes_changed {
            self.selection_changed
                .emit(&(ItemSelection::new(), deselected));
        }
    }

    fn columns_about_to_be_removed(
        &self,
        model: &dyn AbstractItemModel,
        parent: &ModelIndex,
        start: i32,
        end: i32,
    ) {
        let current = self.current_index.borrow().index();
        if current.is_valid()
            && *parent == model.parent(&current)
            && current.column >= start
            && current.column <= end
        {
            let replacement = if start > 0 {
                model.index(current.row, start - 1, parent)
            } else if end < model.column_count(parent) - 1 {
                model.index(current.row, end + 1, parent)
            } else {
                ModelIndex::INVALID
            };
            *self.current_index.borrow_mut() = PersistentModelIndex::new(model, &replacement);
            self.current_changed.emit(&(replacement, current));
            if replacement.row != current.row {
                self.current_row_changed.emit(&(replacement, current));
            }
            self.current_column_changed.emit(&(replacement, current));
        }
        let tl = model.index(0, start, parent);
        let br = model.index(model.row_count(parent) - 1, end, parent);
        let removed = ItemSelection::from_range(model, tl, br);
        let change = {
            let mut old = to_plain(model, &self.ranges.borrow());
            old.merge(
                &to_plain(model, &self.current_selection.borrow()),
                self.current_command.get(),
                model,
            );
            let mut ranges = old.clone();
            ranges.merge(&removed, SelectionFlags::DESELECT, model);
            *self.ranges.borrow_mut() = to_persistent(model, &ranges);
            self.current_selection.borrow_mut().clear();
            selection_difference(model, &ranges, &old)
        };
        if let Some(change) = change {
            self.selection_changed.emit(&change);
        }
    }

    fn items_about_to_be_inserted(
        &self,
        model: &dyn AbstractItemModel,
        parent: &ModelIndex,
        start: i32,
        rows: bool,
    ) {
        self.finalize(model);
        let ranges = to_plain(model, &self.ranges.borrow());
        let mut result: Vec<ItemSelectionRange> = Vec::with_capacity(ranges.len());
        let mut split: Vec<ItemSelectionRange> = Vec::new();
        let mut indexes_changed = false;
        for range in ranges.iter() {
            let r = *range;
            let same_parent = r.is_valid() && r.parent() == *parent;
            let (first, last) = if rows {
                (r.top(), r.bottom())
            } else {
                (r.left(), r.right())
            };
            if same_parent && first < start && last >= start {
                let (end_of_first, begin_of_second) = if rows {
                    (
                        model.index(start - 1, r.right(), parent),
                        model.index(start, r.left(), parent),
                    )
                } else {
                    (
                        model.index(r.bottom(), start - 1, parent),
                        model.index(r.top(), start, parent),
                    )
                };
                split.push(ItemSelectionRange::from_parts(
                    r.top_left(),
                    end_of_first,
                    *parent,
                ));
                split.push(ItemSelectionRange::from_parts(
                    begin_of_second,
                    r.bottom_right(),
                    *parent,
                ));
            } else {
                if rows && same_parent && first >= start {
                    indexes_changed = true;
                }
                result.push(r);
            }
        }
        result.extend(split);
        *self.ranges.borrow_mut() = to_persistent(model, &result.into_iter().collect());
        if indexes_changed {
            self.selection_changed
                .emit(&(ItemSelection::new(), ItemSelection::new()));
        }
    }

    fn layout_about_to_be_changed(&self, model: &dyn AbstractItemModel) {
        let persistent = |ranges: &[PersistentRange]| -> Vec<PersistentModelIndex> {
            to_plain(model, ranges)
                .indexes(model)
                .iter()
                .map(|index| PersistentModelIndex::new(model, index))
                .collect()
        };
        let saved = persistent(&self.ranges.borrow());
        let saved_current = persistent(&self.current_selection.borrow());
        *self.saved_indexes.borrow_mut() = saved;
        *self.saved_current_indexes.borrow_mut() = saved_current;
    }

    fn layout_changed(&self, model: &dyn AbstractItemModel) {
        let saved = std::mem::take(&mut *self.saved_indexes.borrow_mut());
        let saved_current = std::mem::take(&mut *self.saved_current_indexes.borrow_mut());
        if saved.is_empty() && saved_current.is_empty() {
            return;
        }
        let sorted = |list: &[PersistentModelIndex]| -> Vec<(ModelIndex, ModelIndex)> {
            let mut pairs: Vec<(ModelIndex, ModelIndex)> = list
                .iter()
                .map(PersistentModelIndex::index)
                .filter(ModelIndex::is_valid)
                .map(|index| (model.parent(&index), index))
                .collect();
            pairs.sort();
            pairs
        };
        let ranges = merge_indexes(&sorted(&saved));
        let current = merge_indexes(&sorted(&saved_current));
        *self.ranges.borrow_mut() = to_persistent(model, &ranges);
        *self.current_selection.borrow_mut() = to_persistent(model, &current);
    }
}

impl ModelListener for SelectionInner {
    fn model_event(&self, event: &ModelEvent) {
        let Some(shared) = self.model() else {
            return;
        };
        if let ModelEvent::ModelReset = event {
            self.reset();
            return;
        }
        let model_ref = shared.borrow();
        let model: &dyn AbstractItemModel = &*model_ref;
        match event {
            ModelEvent::RowsAboutToBeRemoved {
                parent,
                first,
                last,
            } => {
                self.rows_about_to_be_removed(model, parent, *first, *last);
            }
            ModelEvent::ColumnsAboutToBeRemoved {
                parent,
                first,
                last,
            } => {
                self.columns_about_to_be_removed(model, parent, *first, *last);
            }
            ModelEvent::RowsAboutToBeInserted { parent, first, .. } => {
                self.items_about_to_be_inserted(model, parent, *first, true);
            }
            ModelEvent::ColumnsAboutToBeInserted { parent, first, .. } => {
                self.items_about_to_be_inserted(model, parent, *first, false);
            }
            ModelEvent::LayoutAboutToBeChanged => self.layout_about_to_be_changed(model),
            ModelEvent::LayoutChanged => self.layout_changed(model),
            _ => {}
        }
    }
}

/// Keeps track of the selected items and the current item of a model
/// (`QItemSelectionModel`).
pub struct ItemSelectionModel {
    inner: Rc<SelectionInner>,
    /// `(selected, deselected)` whenever the selection changes.
    pub selection_changed: Signal<(ItemSelection, ItemSelection)>,
    /// `(current, previous)` whenever the current index changes.
    pub current_changed: Signal<(ModelIndex, ModelIndex)>,
    /// `(current, previous)` when the current row (or its parent) changes.
    pub current_row_changed: Signal<(ModelIndex, ModelIndex)>,
    /// `(current, previous)` when the current column (or its parent) changes.
    pub current_column_changed: Signal<(ModelIndex, ModelIndex)>,
    /// Emitted after `set_model`.
    pub model_changed: Signal<()>,
}

impl ItemSelectionModel {
    pub fn new(model: Option<SharedModel>) -> Self {
        let inner = Rc::new_cyclic(|weak| SelectionInner {
            self_weak: weak.clone(),
            model: RefCell::new(None),
            connection: RefCell::new(None),
            ranges: RefCell::new(Vec::new()),
            current_selection: RefCell::new(Vec::new()),
            current_command: Cell::new(SelectionFlags::NO_UPDATE),
            current_index: RefCell::new(PersistentModelIndex::default()),
            saved_indexes: RefCell::new(Vec::new()),
            saved_current_indexes: RefCell::new(Vec::new()),
            selection_changed: Signal::new(),
            current_changed: Signal::new(),
            current_row_changed: Signal::new(),
            current_column_changed: Signal::new(),
            model_changed: Signal::new(),
        });
        let this = Self {
            selection_changed: inner.selection_changed.clone(),
            current_changed: inner.current_changed.clone(),
            current_row_changed: inner.current_row_changed.clone(),
            current_column_changed: inner.current_column_changed.clone(),
            model_changed: inner.model_changed.clone(),
            inner,
        };
        this.inner.set_model(model);
        this
    }

    pub fn model(&self) -> Option<SharedModel> {
        self.inner.model()
    }

    /// Switches to `model`, silently clearing the selection.
    pub fn set_model(&self, model: Option<SharedModel>) {
        self.inner.set_model(model);
    }

    pub fn current_index(&self) -> ModelIndex {
        self.inner.current_index.borrow().index()
    }

    /// Makes `index` current and applies `command` to it.
    pub fn set_current_index(&self, index: &ModelIndex, command: SelectionFlags) {
        self.inner.set_current_index(index, command);
    }

    pub fn clear_current_index(&self) {
        self.inner.clear_current_index();
    }

    /// Applies `command` to the single `index`.
    pub fn select_index(&self, index: &ModelIndex, command: SelectionFlags) {
        self.inner.select_index(index, command);
    }

    /// Applies `command` to `selection`.
    pub fn select(&self, selection: &ItemSelection, command: SelectionFlags) {
        self.inner.select(selection, command);
    }

    /// Clears selection and current index, emitting signals.
    pub fn clear(&self) {
        self.inner.clear_selection();
        self.inner.clear_current_index();
    }

    pub fn clear_selection(&self) {
        self.inner.clear_selection();
    }

    /// Clears selection and current index without emitting signals.
    pub fn reset(&self) {
        self.inner.reset();
    }

    fn with_model<R>(&self, default: R, f: impl FnOnce(&dyn AbstractItemModel) -> R) -> R {
        match self.inner.model() {
            Some(shared) => {
                let model = shared.borrow();
                f(&*model)
            }
            None => default,
        }
    }

    pub fn is_selected(&self, index: &ModelIndex) -> bool {
        self.with_model(false, |model| self.inner.is_selected(model, index))
    }

    /// `true` if every selectable item of `row` below `parent` is selected.
    pub fn is_row_selected(&self, row: i32, parent: &ModelIndex) -> bool {
        self.with_model(false, |model| {
            let mut any = false;
            for column in 0..model.column_count(parent) {
                let index = model.index(row, column, parent);
                if !model.flags(&index).contains(ItemFlags::SELECTABLE) {
                    continue;
                }
                if !self.inner.is_selected(model, &index) {
                    return false;
                }
                any = true;
            }
            any
        })
    }

    /// `true` if every selectable item of `column` below `parent` is selected.
    pub fn is_column_selected(&self, column: i32, parent: &ModelIndex) -> bool {
        self.with_model(false, |model| {
            let mut any = false;
            for row in 0..model.row_count(parent) {
                let index = model.index(row, column, parent);
                if !model.flags(&index).contains(ItemFlags::SELECTABLE) {
                    continue;
                }
                if !self.inner.is_selected(model, &index) {
                    return false;
                }
                any = true;
            }
            any
        })
    }

    /// `true` if any item of `row` below `parent` is selected.
    pub fn row_intersects_selection(&self, row: i32, parent: &ModelIndex) -> bool {
        self.with_model(false, |model| {
            (0..model.column_count(parent)).any(|column| {
                let index = model.index(row, column, parent);
                selectable_and_enabled(model.flags(&index)) && self.inner.is_selected(model, &index)
            })
        })
    }

    /// `true` if any item of `column` below `parent` is selected.
    pub fn column_intersects_selection(&self, column: i32, parent: &ModelIndex) -> bool {
        self.with_model(false, |model| {
            (0..model.row_count(parent)).any(|row| {
                let index = model.index(row, column, parent);
                selectable_and_enabled(model.flags(&index)) && self.inner.is_selected(model, &index)
            })
        })
    }

    pub fn has_selection(&self) -> bool {
        self.with_model(false, |model| {
            let command = self.inner.current_command.get();
            if command.intersects(SelectionFlags::DESELECT | SelectionFlags::TOGGLE) {
                !self.inner.effective_selection(model).is_empty()
            } else {
                !(self.inner.ranges.borrow().is_empty()
                    && self.inner.current_selection.borrow().is_empty())
            }
        })
    }

    /// The effective selection.
    pub fn selection(&self) -> ItemSelection {
        self.with_model(ItemSelection::new(), |model| {
            self.inner.effective_selection(model)
        })
    }

    /// All selected (selectable and enabled) indexes.
    pub fn selected_indexes(&self) -> Vec<ModelIndex> {
        self.with_model(Vec::new(), |model| {
            self.inner.effective_selection(model).indexes(model)
        })
    }

    /// Index in `column` of every fully selected row.
    pub fn selected_rows(&self, column: i32) -> Vec<ModelIndex> {
        let candidates: Vec<(ModelIndex, i32)> = self.with_model(Vec::new(), |model| {
            let mut seen = HashSet::new();
            let mut out = Vec::new();
            for range in self.inner.effective_selection(model).iter() {
                for row in range.top()..=range.bottom() {
                    if seen.insert((range.parent(), row)) {
                        out.push((range.parent(), row));
                    }
                }
            }
            out
        });
        candidates
            .into_iter()
            .filter(|(parent, row)| self.is_row_selected(*row, parent))
            .filter_map(|(parent, row)| {
                self.with_model(None, |model| Some(model.index(row, column, &parent)))
            })
            .collect()
    }

    /// Index in `row` of every fully selected column.
    pub fn selected_columns(&self, row: i32) -> Vec<ModelIndex> {
        let candidates: Vec<(ModelIndex, i32)> = self.with_model(Vec::new(), |model| {
            let mut seen = HashSet::new();
            let mut out = Vec::new();
            for range in self.inner.effective_selection(model).iter() {
                for column in range.left()..=range.right() {
                    if seen.insert((range.parent(), column)) {
                        out.push((range.parent(), column));
                    }
                }
            }
            out
        });
        candidates
            .into_iter()
            .filter(|(parent, column)| self.is_column_selected(*column, parent))
            .filter_map(|(parent, column)| {
                self.with_model(None, |model| Some(model.index(row, column, &parent)))
            })
            .collect()
    }
}

/// Canonical Qt aliases.
pub type QItemSelectionModel = ItemSelectionModel;
pub type QItemSelection = ItemSelection;
pub type QItemSelectionRange = ItemSelectionRange;
