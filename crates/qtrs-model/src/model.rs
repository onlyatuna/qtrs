//! The abstract item model interface (`QAbstractItemModel`) together with the
//! shared bookkeeping every model needs: change signals, persistent index
//! tracking and the begin/end notification protocol.

use std::cell::{Cell, RefCell};
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};
use std::rc::{Rc, Weak};
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

use qtrs_core::{ScopedConnection, Signal, Variant};

use crate::index::{ModelIndex, PersistentData};
use crate::pattern::Regex;
use crate::role::{CaseSensitivity, ItemDataRole, ItemFlags, MatchFlags, Orientation, SortOrder};

static NEXT_UNIQUE_ID: AtomicU64 = AtomicU64::new(1);

/// Process-wide unique, never reused, non-zero identifier.
pub(crate) fn next_unique_id() -> u64 {
    NEXT_UNIQUE_ID.fetch_add(1, AtomicOrdering::Relaxed)
}

/// Model handle shared between views, proxies and selection models.
///
/// All [`AbstractItemModel`] methods take `&self`; access models through
/// `borrow()` so that listeners reacting to a model's signals can query the
/// model re-entrantly.
pub type SharedModel = Rc<RefCell<dyn AbstractItemModel>>;

/// Wraps a model into a [`SharedModel`].
pub fn shared_model<M: AbstractItemModel + 'static>(model: M) -> SharedModel {
    Rc::new(RefCell::new(model))
}

/// Signal carrying `(parent, first, last)`.
pub type RangeSignal = Signal<(ModelIndex, i32, i32)>;

/// Change notifications emitted by every model (`QAbstractItemModel` signals).
///
/// Cloning yields handles to the same underlying signals.
#[derive(Clone, Default)]
pub struct ModelSignals {
    /// `(top_left, bottom_right, roles)`; an empty role list means "all roles".
    pub data_changed: Signal<(ModelIndex, ModelIndex, Vec<i32>)>,
    /// `(orientation, first, last)`.
    pub header_data_changed: Signal<(Orientation, i32, i32)>,
    pub rows_about_to_be_inserted: RangeSignal,
    pub rows_inserted: RangeSignal,
    pub rows_about_to_be_removed: RangeSignal,
    pub rows_removed: RangeSignal,
    pub columns_about_to_be_inserted: RangeSignal,
    pub columns_inserted: RangeSignal,
    pub columns_about_to_be_removed: RangeSignal,
    pub columns_removed: RangeSignal,
    pub layout_about_to_be_changed: Signal<()>,
    pub layout_changed: Signal<()>,
    pub model_about_to_be_reset: Signal<()>,
    pub model_reset: Signal<()>,
}

#[derive(Clone, Copy)]
struct Change {
    parent: ModelIndex,
    first: i32,
    last: i32,
}

/// State shared by all model implementations: identity, signals and the
/// persistent index registry (`QAbstractItemModelPrivate`).
pub struct ModelBase {
    id: u64,
    signals: ModelSignals,
    persistent: RefCell<Vec<Weak<PersistentData>>>,
    moved: RefCell<Vec<Vec<Rc<PersistentData>>>>,
    invalidated: RefCell<Vec<Vec<Rc<PersistentData>>>>,
    changes: RefCell<Vec<Change>>,
    resetting: Cell<bool>,
}

impl Default for ModelBase {
    fn default() -> Self {
        Self::new()
    }
}

impl ModelBase {
    pub fn new() -> Self {
        Self {
            id: next_unique_id(),
            signals: ModelSignals::default(),
            persistent: RefCell::new(Vec::new()),
            moved: RefCell::new(Vec::new()),
            invalidated: RefCell::new(Vec::new()),
            changes: RefCell::new(Vec::new()),
            resetting: Cell::new(false),
        }
    }

    /// Unique id stamped into every [`ModelIndex`] of the model.
    #[inline]
    pub fn id(&self) -> u64 {
        self.id
    }

    #[inline]
    pub fn signals(&self) -> &ModelSignals {
        &self.signals
    }

    /// `true` between `begin_reset_model` and `end_reset_model`.
    pub fn is_resetting(&self) -> bool {
        self.resetting.get()
    }

    pub(crate) fn register_persistent(&self, data: &Rc<PersistentData>) {
        let mut list = self.persistent.borrow_mut();
        if list.len() >= 32 && list.len().is_power_of_two() {
            list.retain(|w| w.upgrade().is_some_and(|d| d.index.get().is_valid()));
        }
        list.push(Rc::downgrade(data));
    }

    /// Live persistent entries tracking a valid index; prunes dead ones.
    pub(crate) fn live_persistent(&self) -> Vec<Rc<PersistentData>> {
        let mut list = self.persistent.borrow_mut();
        let mut out = Vec::with_capacity(list.len());
        list.retain(|w| match w.upgrade() {
            Some(d) if d.index.get().is_valid() => {
                out.push(d);
                true
            }
            _ => false,
        });
        out
    }

    /// Indexes currently tracked by persistent indexes (`persistentIndexList`).
    pub fn persistent_index_list(&self) -> Vec<ModelIndex> {
        self.live_persistent()
            .iter()
            .map(|d| d.index.get())
            .collect()
    }

    /// Redirects persistent indexes equal to `from` to `to` (`changePersistentIndex`).
    pub fn change_persistent_index(&self, from: &ModelIndex, to: &ModelIndex) {
        for d in self.live_persistent() {
            if d.index.get() == *from {
                d.index.set(*to);
            }
        }
    }

    /// Redirects every persistent index equal to `from[i]` to `to[i]`
    /// (`changePersistentIndexList`). All lookups use the state before the call.
    pub fn change_persistent_index_list(&self, from: &[ModelIndex], to: &[ModelIndex]) {
        let lookup: HashMap<ModelIndex, ModelIndex> =
            from.iter().zip(to.iter()).map(|(f, t)| (*f, *t)).collect();
        self.remap_persistent_indexes(|idx| lookup.get(idx).copied());
    }

    /// Applies `map` to every tracked index; `Some(new)` replaces the index.
    pub fn remap_persistent_indexes(&self, mut map: impl FnMut(&ModelIndex) -> Option<ModelIndex>) {
        let updates: Vec<(Rc<PersistentData>, ModelIndex)> = self
            .live_persistent()
            .into_iter()
            .filter_map(|d| map(&d.index.get()).map(|to| (d, to)))
            .collect();
        for (d, to) in updates {
            d.index.set(to);
        }
    }

    /// Invalidates every persistent index of the model.
    pub fn invalidate_persistent_indexes(&self) {
        for d in self.live_persistent() {
            d.index.set(ModelIndex::INVALID);
        }
        self.persistent.borrow_mut().clear();
    }

    /// Invalidates persistent indexes equal to `index`.
    pub fn invalidate_persistent_index(&self, index: &ModelIndex) {
        self.change_persistent_index(index, &ModelIndex::INVALID);
    }
}

impl Drop for ModelBase {
    fn drop(&mut self) {
        for weak in self.persistent.get_mut().drain(..) {
            if let Some(d) = weak.upgrade() {
                d.index.set(ModelIndex::INVALID);
            }
        }
    }
}

/// Default header text: the 1-based section number for the display role.
pub fn default_header_data(section: i32, role: ItemDataRole) -> Variant {
    if role == ItemDataRole::Display {
        Variant::I64(section as i64 + 1)
    } else {
        Variant::Invalid
    }
}

/// Interface of every item model (`QAbstractItemModel`).
///
/// All methods take `&self`: implementations use interior mutability and must
/// not hold internal borrows while emitting signals, so that listeners can
/// query the model from inside their slots.
///
/// The `begin_*`/`end_*` provided methods implement Qt's protected
/// notification protocol and keep persistent indexes up to date; models must
/// call them around structural changes.
pub trait AbstractItemModel {
    /// Shared model state (id, signals, persistent indexes).
    fn base(&self) -> &ModelBase;

    /// Index of the item at `(row, column)` below `parent`.
    fn index(&self, row: i32, column: i32, parent: &ModelIndex) -> ModelIndex;

    /// Parent of `child` (invalid for top-level items).
    fn parent(&self, child: &ModelIndex) -> ModelIndex;

    fn row_count(&self, parent: &ModelIndex) -> i32;

    fn column_count(&self, parent: &ModelIndex) -> i32;

    fn data(&self, index: &ModelIndex, role: ItemDataRole) -> Variant;

    #[inline]
    fn model_id(&self) -> u64 {
        self.base().id()
    }

    #[inline]
    fn signals(&self) -> &ModelSignals {
        self.base().signals()
    }

    /// Creates an index belonging to this model (`createIndex`).
    #[inline]
    fn create_index(&self, row: i32, column: i32, internal_id: u64) -> ModelIndex {
        ModelIndex::new(row, column, internal_id, self.model_id())
    }

    /// `true` if `(row, column)` exists below `parent`.
    fn has_index(&self, row: i32, column: i32, parent: &ModelIndex) -> bool {
        row >= 0
            && column >= 0
            && row < self.row_count(parent)
            && column < self.column_count(parent)
    }

    fn sibling(&self, row: i32, column: i32, index: &ModelIndex) -> ModelIndex {
        if row == index.row && column == index.column {
            *index
        } else {
            self.index(row, column, &self.parent(index))
        }
    }

    fn has_children(&self, parent: &ModelIndex) -> bool {
        if parent.is_valid() {
            if parent.model_id != self.model_id() {
                return false;
            }
            if self.flags(parent).contains(ItemFlags::NEVER_HAS_CHILDREN) {
                return false;
            }
        }
        self.row_count(parent) > 0 && self.column_count(parent) > 0
    }

    fn set_data(&self, _index: &ModelIndex, _value: Variant, _role: ItemDataRole) -> bool {
        false
    }

    fn header_data(&self, section: i32, _orientation: Orientation, role: ItemDataRole) -> Variant {
        default_header_data(section, role)
    }

    fn set_header_data(
        &self,
        _section: i32,
        _orientation: Orientation,
        _value: Variant,
        _role: ItemDataRole,
    ) -> bool {
        false
    }

    fn flags(&self, index: &ModelIndex) -> ItemFlags {
        if index.is_valid() {
            ItemFlags::SELECTABLE | ItemFlags::ENABLED
        } else {
            ItemFlags::NONE
        }
    }

    /// All non-null standard roles of `index` (`itemData`).
    fn item_data(&self, index: &ModelIndex) -> BTreeMap<ItemDataRole, Variant> {
        ItemDataRole::STANDARD_ROLES
            .iter()
            .filter_map(|&role| {
                let value = self.data(index, role);
                value.is_valid().then_some((role, value))
            })
            .collect()
    }

    /// Sets every role of `roles`; `true` only if all succeeded (`setItemData`).
    fn set_item_data(&self, index: &ModelIndex, roles: &BTreeMap<ItemDataRole, Variant>) -> bool {
        let mut ok = true;
        for (role, value) in roles {
            ok &= self.set_data(index, value.clone(), *role);
        }
        ok
    }

    fn insert_rows(&self, _row: i32, _count: i32, _parent: &ModelIndex) -> bool {
        false
    }

    fn remove_rows(&self, _row: i32, _count: i32, _parent: &ModelIndex) -> bool {
        false
    }

    fn insert_columns(&self, _column: i32, _count: i32, _parent: &ModelIndex) -> bool {
        false
    }

    fn remove_columns(&self, _column: i32, _count: i32, _parent: &ModelIndex) -> bool {
        false
    }

    fn insert_row(&self, row: i32, parent: &ModelIndex) -> bool {
        self.insert_rows(row, 1, parent)
    }

    fn remove_row(&self, row: i32, parent: &ModelIndex) -> bool {
        self.remove_rows(row, 1, parent)
    }

    fn insert_column(&self, column: i32, parent: &ModelIndex) -> bool {
        self.insert_columns(column, 1, parent)
    }

    fn remove_column(&self, column: i32, parent: &ModelIndex) -> bool {
        self.remove_columns(column, 1, parent)
    }

    /// Sorts the model by `column`; models that cannot sort ignore the call.
    fn sort(&self, _column: i32, _order: SortOrder) {}

    /// Index an editor should be opened on for `index` (`buddy`).
    fn buddy(&self, index: &ModelIndex) -> ModelIndex {
        *index
    }

    /// Searches the column of `start` (`match`). `hits == -1` returns all matches.
    fn match_indexes(
        &self,
        start: &ModelIndex,
        role: ItemDataRole,
        value: &Variant,
        hits: i32,
        flags: MatchFlags,
    ) -> Vec<ModelIndex> {
        match_indexes_impl(self, start, role, value, hits, flags)
    }

    /// Tracked persistent indexes.
    fn persistent_index_list(&self) -> Vec<ModelIndex> {
        self.base().persistent_index_list()
    }

    fn change_persistent_index(&self, from: &ModelIndex, to: &ModelIndex) {
        self.base().change_persistent_index(from, to);
    }

    fn change_persistent_index_list(&self, from: &[ModelIndex], to: &[ModelIndex]) {
        self.base().change_persistent_index_list(from, to);
    }

    /// Emits `data_changed` for `[top_left, bottom_right]`.
    fn emit_data_changed(
        &self,
        top_left: &ModelIndex,
        bottom_right: &ModelIndex,
        roles: &[ItemDataRole],
    ) {
        let roles: Vec<i32> = roles.iter().map(|r| r.to_i32()).collect();
        self.signals()
            .data_changed
            .emit(&(*top_left, *bottom_right, roles));
    }

    /// Emits `layout_about_to_be_changed`.
    fn begin_layout_change(&self) {
        self.signals().layout_about_to_be_changed.emit(&());
    }

    /// Emits `layout_changed`.
    fn end_layout_change(&self) {
        self.signals().layout_changed.emit(&());
    }

    /// Announces the insertion of rows `first..=last` below `parent` (`beginInsertRows`).
    fn begin_insert_rows(&self, parent: &ModelIndex, first: i32, last: i32) {
        let base = self.base();
        base.changes.borrow_mut().push(Change {
            parent: *parent,
            first,
            last,
        });
        base.signals
            .rows_about_to_be_inserted
            .emit(&(*parent, first, last));
        let mut moved = Vec::new();
        if first < self.row_count(parent) {
            for d in base.live_persistent() {
                let idx = d.index.get();
                if idx.row >= first && self.parent(&idx) == *parent {
                    moved.push(d);
                }
            }
        }
        base.moved.borrow_mut().push(moved);
    }

    /// Completes a row insertion (`endInsertRows`).
    fn end_insert_rows(&self) {
        let base = self.base();
        let change = base
            .changes
            .borrow_mut()
            .pop()
            .expect("end_insert_rows without begin_insert_rows");
        let moved = base.moved.borrow_mut().pop().unwrap_or_default();
        let count = change.last - change.first + 1;
        for d in moved {
            let old = d.index.get();
            d.index
                .set(self.index(old.row + count, old.column, &change.parent));
        }
        base.signals
            .rows_inserted
            .emit(&(change.parent, change.first, change.last));
    }

    /// Announces the removal of rows `first..=last` below `parent` (`beginRemoveRows`).
    fn begin_remove_rows(&self, parent: &ModelIndex, first: i32, last: i32) {
        let base = self.base();
        base.changes.borrow_mut().push(Change {
            parent: *parent,
            first,
            last,
        });
        base.signals
            .rows_about_to_be_removed
            .emit(&(*parent, first, last));
        let (moved, invalidated) = collect_removed_persistent(self, parent, first, last, |i| i.row);
        base.moved.borrow_mut().push(moved);
        base.invalidated.borrow_mut().push(invalidated);
    }

    /// Completes a row removal (`endRemoveRows`).
    fn end_remove_rows(&self) {
        let base = self.base();
        let change = base
            .changes
            .borrow_mut()
            .pop()
            .expect("end_remove_rows without begin_remove_rows");
        let moved = base.moved.borrow_mut().pop().unwrap_or_default();
        let invalidated = base.invalidated.borrow_mut().pop().unwrap_or_default();
        let count = change.last - change.first + 1;
        for d in moved {
            let old = d.index.get();
            d.index
                .set(self.index(old.row - count, old.column, &change.parent));
        }
        for d in invalidated {
            d.index.set(ModelIndex::INVALID);
        }
        base.signals
            .rows_removed
            .emit(&(change.parent, change.first, change.last));
    }

    /// Announces the insertion of columns `first..=last` below `parent` (`beginInsertColumns`).
    fn begin_insert_columns(&self, parent: &ModelIndex, first: i32, last: i32) {
        let base = self.base();
        base.changes.borrow_mut().push(Change {
            parent: *parent,
            first,
            last,
        });
        base.signals
            .columns_about_to_be_inserted
            .emit(&(*parent, first, last));
        let mut moved = Vec::new();
        if first < self.column_count(parent) {
            for d in base.live_persistent() {
                let idx = d.index.get();
                if idx.column >= first && self.parent(&idx) == *parent {
                    moved.push(d);
                }
            }
        }
        base.moved.borrow_mut().push(moved);
    }

    /// Completes a column insertion (`endInsertColumns`).
    fn end_insert_columns(&self) {
        let base = self.base();
        let change = base
            .changes
            .borrow_mut()
            .pop()
            .expect("end_insert_columns without begin_insert_columns");
        let moved = base.moved.borrow_mut().pop().unwrap_or_default();
        let count = change.last - change.first + 1;
        for d in moved {
            let old = d.index.get();
            d.index
                .set(self.index(old.row, old.column + count, &change.parent));
        }
        base.signals
            .columns_inserted
            .emit(&(change.parent, change.first, change.last));
    }

    /// Announces the removal of columns `first..=last` below `parent` (`beginRemoveColumns`).
    fn begin_remove_columns(&self, parent: &ModelIndex, first: i32, last: i32) {
        let base = self.base();
        base.changes.borrow_mut().push(Change {
            parent: *parent,
            first,
            last,
        });
        base.signals
            .columns_about_to_be_removed
            .emit(&(*parent, first, last));
        let (moved, invalidated) =
            collect_removed_persistent(self, parent, first, last, |i| i.column);
        base.moved.borrow_mut().push(moved);
        base.invalidated.borrow_mut().push(invalidated);
    }

    /// Completes a column removal (`endRemoveColumns`).
    fn end_remove_columns(&self) {
        let base = self.base();
        let change = base
            .changes
            .borrow_mut()
            .pop()
            .expect("end_remove_columns without begin_remove_columns");
        let moved = base.moved.borrow_mut().pop().unwrap_or_default();
        let invalidated = base.invalidated.borrow_mut().pop().unwrap_or_default();
        let count = change.last - change.first + 1;
        for d in moved {
            let old = d.index.get();
            d.index
                .set(self.index(old.row, old.column - count, &change.parent));
        }
        for d in invalidated {
            d.index.set(ModelIndex::INVALID);
        }
        base.signals
            .columns_removed
            .emit(&(change.parent, change.first, change.last));
    }

    /// Announces a model reset (`beginResetModel`).
    fn begin_reset_model(&self) {
        self.base().resetting.set(true);
        self.signals().model_about_to_be_reset.emit(&());
    }

    /// Completes a model reset, invalidating persistent indexes (`endResetModel`).
    fn end_reset_model(&self) {
        let base = self.base();
        base.invalidate_persistent_indexes();
        base.resetting.set(false);
        base.signals.model_reset.emit(&());
    }
}

/// Classifies persistent indexes for a removal of `first..=last` below `parent`:
/// entries after the range on the same level move, entries inside the range or
/// its subtrees are invalidated.
fn collect_removed_persistent<M: AbstractItemModel + ?Sized>(
    model: &M,
    parent: &ModelIndex,
    first: i32,
    last: i32,
    position: impl Fn(&ModelIndex) -> i32,
) -> (Vec<Rc<PersistentData>>, Vec<Rc<PersistentData>>) {
    let mut moved = Vec::new();
    let mut invalidated = Vec::new();
    for d in model.base().live_persistent() {
        let mut level_changed = false;
        let mut current = d.index.get();
        while current.is_valid() {
            let current_parent = model.parent(&current);
            if current_parent == *parent {
                let pos = position(&current);
                if !level_changed && pos > last {
                    moved.push(d.clone());
                } else if pos >= first && pos <= last {
                    invalidated.push(d.clone());
                }
                break;
            }
            current = current_parent;
            level_changed = true;
        }
    }
    (moved, invalidated)
}

fn fold_case(text: &str) -> String {
    text.to_lowercase()
}

fn match_indexes_impl<M: AbstractItemModel + ?Sized>(
    model: &M,
    start: &ModelIndex,
    role: ItemDataRole,
    value: &Variant,
    hits: i32,
    flags: MatchFlags,
) -> Vec<ModelIndex> {
    let mut result = Vec::new();
    if !start.is_valid() {
        return result;
    }
    let match_type = flags.match_type();
    let case = if flags.contains(MatchFlags::CASE_SENSITIVE) {
        CaseSensitivity::Sensitive
    } else {
        CaseSensitivity::Insensitive
    };
    let insensitive = case == CaseSensitivity::Insensitive;
    let recurse = flags.contains(MatchFlags::RECURSIVE);
    let wrap = flags.contains(MatchFlags::WRAP);
    let all_hits = hits == -1;
    let text = value.to_string_lossy();
    let folded_text = if insensitive {
        fold_case(&text)
    } else {
        text.clone()
    };
    let regex = if match_type == MatchFlags::REGULAR_EXPRESSION {
        match value {
            Variant::RegularExpression(pattern, ci) => Regex::new(pattern, *ci).ok(),
            _ => Regex::new(&text, insensitive).ok(),
        }
    } else if match_type == MatchFlags::WILDCARD {
        Regex::from_wildcard(&text, true, insensitive).ok()
    } else {
        None
    };

    let column = start.column;
    let parent = model.parent(start);
    let mut from = start.row;
    let mut to = model.row_count(&parent);
    let passes = if wrap { 2 } else { 1 };
    for _ in 0..passes {
        let mut row = from;
        while row < to && (all_hits || (result.len() as i32) < hits) {
            let idx = model.index(row, column, &parent);
            row += 1;
            if !idx.is_valid() {
                continue;
            }
            let v = model.data(&idx, role);
            let matched = if match_type == MatchFlags::EXACTLY {
                variant_equals(value, &v)
            } else if match_type == MatchFlags::REGULAR_EXPRESSION
                || match_type == MatchFlags::WILDCARD
            {
                regex
                    .as_ref()
                    .is_some_and(|rx| rx.is_match(&v.to_string_lossy()))
            } else {
                let t = v.to_string_lossy();
                let t = if insensitive { fold_case(&t) } else { t };
                if match_type == MatchFlags::STARTS_WITH {
                    t.starts_with(&folded_text)
                } else if match_type == MatchFlags::ENDS_WITH {
                    t.ends_with(&folded_text)
                } else if match_type == MatchFlags::FIXED_STRING {
                    t == folded_text
                } else {
                    t.contains(&folded_text)
                }
            };
            if matched {
                result.push(idx);
            }
            if recurse {
                let child_parent = if column != 0 {
                    model.sibling(idx.row, 0, &idx)
                } else {
                    idx
                };
                if model.has_children(&child_parent) {
                    let remaining = if all_hits {
                        -1
                    } else {
                        hits - result.len() as i32
                    };
                    let first_child = model.index(0, column, &child_parent);
                    result.extend(match_indexes_impl(
                        model,
                        &first_child,
                        role,
                        value,
                        remaining,
                        flags,
                    ));
                }
            }
        }
        from = 0;
        to = start.row;
    }
    result
}

#[derive(Clone, Copy)]
enum Number {
    Int(i128),
    Float(f64),
}

impl Number {
    fn of(v: &Variant) -> Option<Number> {
        match v {
            Variant::Bool(b) => Some(Number::Int(*b as i128)),
            Variant::I64(i) => Some(Number::Int(*i as i128)),
            Variant::U64(u) => Some(Number::Int(*u as i128)),
            Variant::F64(f) => Some(Number::Float(*f)),
            _ => None,
        }
    }

    fn parse(v: &Variant) -> Option<Number> {
        Number::of(v).or_else(|| match v {
            Variant::String(s) => {
                let s = s.trim();
                s.parse::<i128>()
                    .map(Number::Int)
                    .ok()
                    .or_else(|| s.parse::<f64>().ok().map(Number::Float))
            }
            _ => None,
        })
    }

    fn as_f64(self) -> f64 {
        match self {
            Number::Int(i) => i as f64,
            Number::Float(f) => f,
        }
    }

    fn compare(self, other: Number) -> Option<Ordering> {
        match (self, other) {
            (Number::Int(a), Number::Int(b)) => Some(a.cmp(&b)),
            _ => self.as_f64().partial_cmp(&other.as_f64()),
        }
    }
}

/// Variant equality with numeric promotion (`QVariant::operator==`).
pub fn variant_equals(a: &Variant, b: &Variant) -> bool {
    if a == b {
        return true;
    }
    match (Number::of(a), Number::of(b)) {
        (Some(x), Some(y)) => x.compare(y) == Some(Ordering::Equal),
        (Some(x), None) if matches!(b, Variant::String(_)) => {
            Number::parse(b).is_some_and(|y| x.compare(y) == Some(Ordering::Equal))
        }
        (None, Some(y)) if matches!(a, Variant::String(_)) => {
            Number::parse(a).is_some_and(|x| x.compare(y) == Some(Ordering::Equal))
        }
        _ => false,
    }
}

/// Compares two strings honouring `case`.
pub fn compare_strings(a: &str, b: &str, case: CaseSensitivity) -> Ordering {
    match case {
        CaseSensitivity::Sensitive => a.cmp(b),
        CaseSensitivity::Insensitive => a
            .chars()
            .flat_map(char::to_lowercase)
            .cmp(b.chars().flat_map(char::to_lowercase)),
    }
}

/// Ordering used by sorting models (`QAbstractItemModelPrivate::isVariantLessThan`):
/// invalid values sort last, numbers and temporal values compare by value,
/// everything else compares by string representation.
pub fn variant_less_than(left: &Variant, right: &Variant, case: CaseSensitivity) -> bool {
    if !left.is_valid() {
        return false;
    }
    if !right.is_valid() {
        return true;
    }
    match left {
        Variant::Bool(_) | Variant::I64(_) | Variant::U64(_) | Variant::F64(_) => {
            let l = Number::of(left).unwrap_or(Number::Int(0));
            let r = Number::parse(right).unwrap_or(Number::Int(0));
            l.compare(r) == Some(Ordering::Less)
        }
        Variant::Date(..) | Variant::Time(..) | Variant::DateTime(..) => match (left, right) {
            (Variant::Date(y1, m1, d1), Variant::Date(y2, m2, d2)) => (y1, m1, d1) < (y2, m2, d2),
            (Variant::Time(h1, m1, s1, ms1), Variant::Time(h2, m2, s2, ms2)) => {
                (h1, m1, s1, ms1) < (h2, m2, s2, ms2)
            }
            (Variant::DateTime(a), Variant::DateTime(b)) => a < b,
            _ => {
                compare_strings(&left.to_string_lossy(), &right.to_string_lossy(), case)
                    == Ordering::Less
            }
        },
        _ => {
            compare_strings(&left.to_string_lossy(), &right.to_string_lossy(), case)
                == Ordering::Less
        }
    }
}

/// Stable merge sort driven by a strict "less than" predicate.
///
/// Unlike `slice::sort_by`, an inconsistent predicate never panics.
pub(crate) fn stable_sort_by_less<T: Copy>(values: &mut [T], less: &mut dyn FnMut(&T, &T) -> bool) {
    let mut buffer = Vec::with_capacity(values.len() / 2 + 1);
    merge_sort(values, &mut buffer, less);
}

fn merge_sort<T: Copy>(
    values: &mut [T],
    buffer: &mut Vec<T>,
    less: &mut dyn FnMut(&T, &T) -> bool,
) {
    let n = values.len();
    if n <= 1 {
        return;
    }
    if n <= 16 {
        for i in 1..n {
            let mut j = i;
            while j > 0 && less(&values[j], &values[j - 1]) {
                values.swap(j, j - 1);
                j -= 1;
            }
        }
        return;
    }
    let mid = n / 2;
    merge_sort(&mut values[..mid], buffer, less);
    merge_sort(&mut values[mid..], buffer, less);
    if !less(&values[mid], &values[mid - 1]) {
        return;
    }
    buffer.clear();
    buffer.extend_from_slice(&values[..mid]);
    let (mut i, mut j, mut k) = (0, mid, 0);
    while i < buffer.len() && j < n {
        if less(&values[j], &buffer[i]) {
            values[k] = values[j];
            j += 1;
        } else {
            values[k] = buffer[i];
            i += 1;
        }
        k += 1;
    }
    while i < buffer.len() {
        values[k] = buffer[i];
        i += 1;
        k += 1;
    }
}

/// A model notification delivered to a [`ModelListener`].
#[derive(Debug, Clone, PartialEq)]
pub enum ModelEvent {
    DataChanged {
        top_left: ModelIndex,
        bottom_right: ModelIndex,
        roles: Vec<i32>,
    },
    HeaderDataChanged {
        orientation: Orientation,
        first: i32,
        last: i32,
    },
    RowsAboutToBeInserted {
        parent: ModelIndex,
        first: i32,
        last: i32,
    },
    RowsInserted {
        parent: ModelIndex,
        first: i32,
        last: i32,
    },
    RowsAboutToBeRemoved {
        parent: ModelIndex,
        first: i32,
        last: i32,
    },
    RowsRemoved {
        parent: ModelIndex,
        first: i32,
        last: i32,
    },
    ColumnsAboutToBeInserted {
        parent: ModelIndex,
        first: i32,
        last: i32,
    },
    ColumnsInserted {
        parent: ModelIndex,
        first: i32,
        last: i32,
    },
    ColumnsAboutToBeRemoved {
        parent: ModelIndex,
        first: i32,
        last: i32,
    },
    ColumnsRemoved {
        parent: ModelIndex,
        first: i32,
        last: i32,
    },
    LayoutAboutToBeChanged,
    LayoutChanged,
    ModelAboutToBeReset,
    ModelReset,
}

/// Single-threaded receiver of all signals of a model.
///
/// Signal slots must be `Send + Sync`, which rules out capturing `Rc` state.
/// [`ModelConnection`] bridges that gap: its slots capture only a key and look
/// the listener up in a thread-local registry.
pub trait ModelListener {
    fn model_event(&self, event: &ModelEvent);
}

thread_local! {
    static MODEL_LISTENERS: RefCell<HashMap<u64, Weak<dyn ModelListener>>> = RefCell::new(HashMap::new());
}

fn dispatch_model_event(key: u64, event: ModelEvent) {
    let listener = MODEL_LISTENERS
        .try_with(|map| map.borrow().get(&key).and_then(Weak::upgrade))
        .ok()
        .flatten();
    if let Some(listener) = listener {
        listener.model_event(&event);
    }
}

/// Connection of every [`ModelSignals`] signal to a [`ModelListener`];
/// disconnects when dropped.
pub struct ModelConnection {
    key: u64,
    _connections: Vec<ScopedConnection>,
}

impl ModelConnection {
    /// Connects `listener` to all of `signals`. Events are delivered only on the
    /// connecting thread and only while the listener is alive.
    pub fn new(signals: &ModelSignals, listener: Weak<dyn ModelListener>) -> Self {
        let key = next_unique_id();
        MODEL_LISTENERS.with(|map| {
            map.borrow_mut().insert(key, listener);
        });
        fn range(
            signal: &RangeSignal,
            key: u64,
            make: fn(ModelIndex, i32, i32) -> ModelEvent,
        ) -> ScopedConnection {
            signal.connect_scoped(move |(parent, first, last)| {
                dispatch_model_event(key, make(*parent, *first, *last))
            })
        }
        fn unit(signal: &Signal<()>, key: u64, event: ModelEvent) -> ScopedConnection {
            signal.connect_scoped(move |_| dispatch_model_event(key, event.clone()))
        }
        let connections = vec![
            signals
                .data_changed
                .connect_scoped(move |(top_left, bottom_right, roles)| {
                    dispatch_model_event(
                        key,
                        ModelEvent::DataChanged {
                            top_left: *top_left,
                            bottom_right: *bottom_right,
                            roles: roles.clone(),
                        },
                    )
                }),
            signals
                .header_data_changed
                .connect_scoped(move |(orientation, first, last)| {
                    dispatch_model_event(
                        key,
                        ModelEvent::HeaderDataChanged {
                            orientation: *orientation,
                            first: *first,
                            last: *last,
                        },
                    )
                }),
            range(
                &signals.rows_about_to_be_inserted,
                key,
                |parent, first, last| ModelEvent::RowsAboutToBeInserted {
                    parent,
                    first,
                    last,
                },
            ),
            range(&signals.rows_inserted, key, |parent, first, last| {
                ModelEvent::RowsInserted {
                    parent,
                    first,
                    last,
                }
            }),
            range(
                &signals.rows_about_to_be_removed,
                key,
                |parent, first, last| ModelEvent::RowsAboutToBeRemoved {
                    parent,
                    first,
                    last,
                },
            ),
            range(&signals.rows_removed, key, |parent, first, last| {
                ModelEvent::RowsRemoved {
                    parent,
                    first,
                    last,
                }
            }),
            range(
                &signals.columns_about_to_be_inserted,
                key,
                |parent, first, last| ModelEvent::ColumnsAboutToBeInserted {
                    parent,
                    first,
                    last,
                },
            ),
            range(&signals.columns_inserted, key, |parent, first, last| {
                ModelEvent::ColumnsInserted {
                    parent,
                    first,
                    last,
                }
            }),
            range(
                &signals.columns_about_to_be_removed,
                key,
                |parent, first, last| ModelEvent::ColumnsAboutToBeRemoved {
                    parent,
                    first,
                    last,
                },
            ),
            range(&signals.columns_removed, key, |parent, first, last| {
                ModelEvent::ColumnsRemoved {
                    parent,
                    first,
                    last,
                }
            }),
            unit(
                &signals.layout_about_to_be_changed,
                key,
                ModelEvent::LayoutAboutToBeChanged,
            ),
            unit(&signals.layout_changed, key, ModelEvent::LayoutChanged),
            unit(
                &signals.model_about_to_be_reset,
                key,
                ModelEvent::ModelAboutToBeReset,
            ),
            unit(&signals.model_reset, key, ModelEvent::ModelReset),
        ];
        Self {
            key,
            _connections: connections,
        }
    }
}

impl Drop for ModelConnection {
    fn drop(&mut self) {
        let key = self.key;
        let _ = MODEL_LISTENERS.try_with(|map| {
            if let Ok(mut map) = map.try_borrow_mut() {
                map.remove(&key);
            }
        });
    }
}

/// Implements [`AbstractItemModel`] for a wrapper type by forwarding every
/// method to an inner model stored in `$field` (typically an `Rc<Inner>`).
macro_rules! delegate_item_model {
    ($ty:ty, $field:ident) => {
        impl $crate::model::AbstractItemModel for $ty {
            fn base(&self) -> &$crate::model::ModelBase {
                self.$field.base()
            }
            fn index(
                &self,
                row: i32,
                column: i32,
                parent: &$crate::ModelIndex,
            ) -> $crate::ModelIndex {
                self.$field.index(row, column, parent)
            }
            fn parent(&self, child: &$crate::ModelIndex) -> $crate::ModelIndex {
                self.$field.parent(child)
            }
            fn row_count(&self, parent: &$crate::ModelIndex) -> i32 {
                self.$field.row_count(parent)
            }
            fn column_count(&self, parent: &$crate::ModelIndex) -> i32 {
                self.$field.column_count(parent)
            }
            fn data(
                &self,
                index: &$crate::ModelIndex,
                role: $crate::ItemDataRole,
            ) -> qtrs_core::Variant {
                self.$field.data(index, role)
            }
            fn has_index(&self, row: i32, column: i32, parent: &$crate::ModelIndex) -> bool {
                self.$field.has_index(row, column, parent)
            }
            fn sibling(
                &self,
                row: i32,
                column: i32,
                index: &$crate::ModelIndex,
            ) -> $crate::ModelIndex {
                self.$field.sibling(row, column, index)
            }
            fn has_children(&self, parent: &$crate::ModelIndex) -> bool {
                self.$field.has_children(parent)
            }
            fn set_data(
                &self,
                index: &$crate::ModelIndex,
                value: qtrs_core::Variant,
                role: $crate::ItemDataRole,
            ) -> bool {
                self.$field.set_data(index, value, role)
            }
            fn header_data(
                &self,
                section: i32,
                orientation: $crate::Orientation,
                role: $crate::ItemDataRole,
            ) -> qtrs_core::Variant {
                self.$field.header_data(section, orientation, role)
            }
            fn set_header_data(
                &self,
                section: i32,
                orientation: $crate::Orientation,
                value: qtrs_core::Variant,
                role: $crate::ItemDataRole,
            ) -> bool {
                self.$field
                    .set_header_data(section, orientation, value, role)
            }
            fn flags(&self, index: &$crate::ModelIndex) -> $crate::ItemFlags {
                self.$field.flags(index)
            }
            fn insert_rows(&self, row: i32, count: i32, parent: &$crate::ModelIndex) -> bool {
                self.$field.insert_rows(row, count, parent)
            }
            fn remove_rows(&self, row: i32, count: i32, parent: &$crate::ModelIndex) -> bool {
                self.$field.remove_rows(row, count, parent)
            }
            fn insert_columns(&self, column: i32, count: i32, parent: &$crate::ModelIndex) -> bool {
                self.$field.insert_columns(column, count, parent)
            }
            fn remove_columns(&self, column: i32, count: i32, parent: &$crate::ModelIndex) -> bool {
                self.$field.remove_columns(column, count, parent)
            }
            fn sort(&self, column: i32, order: $crate::SortOrder) {
                self.$field.sort(column, order)
            }
            fn buddy(&self, index: &$crate::ModelIndex) -> $crate::ModelIndex {
                self.$field.buddy(index)
            }
            fn match_indexes(
                &self,
                start: &$crate::ModelIndex,
                role: $crate::ItemDataRole,
                value: &qtrs_core::Variant,
                hits: i32,
                flags: $crate::MatchFlags,
            ) -> Vec<$crate::ModelIndex> {
                self.$field.match_indexes(start, role, value, hits, flags)
            }
        }
    };
}
pub(crate) use delegate_item_model;
