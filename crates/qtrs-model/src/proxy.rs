//! Proxy models (`QAbstractProxyModel`, `QIdentityProxyModel`,
//! `QSortFilterProxyModel`).

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::{Rc, Weak};

use qtrs_core::{RegularExpression, Variant};

use crate::index::{ModelIndex, PersistentModelIndex};
use crate::model::{
    delegate_item_model, stable_sort_by_less, variant_less_than, AbstractItemModel, ModelBase,
    ModelConnection, ModelEvent, ModelListener, SharedModel,
};
use crate::pattern::Regex;
use crate::role::{CaseSensitivity, ItemDataRole, ItemFlags, Orientation, SortOrder};
use crate::selection::{ItemSelection, ItemSelectionRange};

/// Model that presents another ("source") model (`QAbstractProxyModel`).
pub trait AbstractProxyModel: AbstractItemModel {
    fn source_model(&self) -> Option<SharedModel>;

    /// Replaces the source model, resetting the proxy.
    fn set_source_model(&self, source: Option<SharedModel>);

    /// Source index corresponding to `proxy_index`.
    fn map_to_source(&self, proxy_index: &ModelIndex) -> ModelIndex;

    /// Proxy index corresponding to `source_index` (invalid when hidden).
    fn map_from_source(&self, source_index: &ModelIndex) -> ModelIndex;

    /// Maps a proxy selection to the source model.
    fn map_selection_to_source(&self, selection: &ItemSelection) -> ItemSelection {
        let Some(source) = self.source_model() else {
            return ItemSelection::new();
        };
        let source = source.borrow();
        selection
            .indexes(self)
            .iter()
            .map(|index| self.map_to_source(index))
            .filter(ModelIndex::is_valid)
            .map(|index| ItemSelectionRange::from_index(&*source, index))
            .collect()
    }

    /// Maps a source selection to the proxy model.
    fn map_selection_from_source(&self, selection: &ItemSelection) -> ItemSelection {
        let Some(source) = self.source_model() else {
            return ItemSelection::new();
        };
        let indexes = selection.indexes(&*source.borrow());
        indexes
            .iter()
            .map(|index| self.map_from_source(index))
            .filter(ModelIndex::is_valid)
            .map(|index| ItemSelectionRange::from_index(self, index))
            .collect()
    }
}

/// Source model slot shared by the proxies.
struct SourceSlot {
    model: RefCell<Option<SharedModel>>,
    model_id: Cell<u64>,
    connection: RefCell<Option<ModelConnection>>,
}

impl SourceSlot {
    fn new() -> Self {
        Self {
            model: RefCell::new(None),
            model_id: Cell::new(0),
            connection: RefCell::new(None),
        }
    }

    fn get(&self) -> Option<SharedModel> {
        self.model.borrow().clone()
    }

    fn replace(&self, source: Option<SharedModel>, listener: Weak<dyn ModelListener>) {
        *self.connection.borrow_mut() = None;
        self.model_id
            .set(source.as_ref().map_or(0, |s| s.borrow().model_id()));
        if let Some(source) = &source {
            let connection = ModelConnection::new(source.borrow().signals(), listener);
            *self.connection.borrow_mut() = Some(connection);
        }
        *self.model.borrow_mut() = source;
    }
}

const SOURCE_BORROWED: &str =
    "source model is mutably borrowed; access shared models through borrow()";

// ===========================================================================
// IdentityProxyModel
// ===========================================================================

struct IdentityInner {
    base: ModelBase,
    self_weak: Weak<IdentityInner>,
    source: SourceSlot,
    layout_saved: RefCell<Vec<(ModelIndex, PersistentModelIndex)>>,
}

impl IdentityInner {
    fn to_source(&self, index: &ModelIndex) -> ModelIndex {
        if !index.is_valid() || index.model_id != self.base.id() {
            return ModelIndex::INVALID;
        }
        ModelIndex::new(
            index.row,
            index.column,
            index.internal_id,
            self.source.model_id.get(),
        )
    }

    fn from_source(&self, index: &ModelIndex) -> ModelIndex {
        if !index.is_valid() || index.model_id != self.source.model_id.get() {
            return ModelIndex::INVALID;
        }
        ModelIndex::new(index.row, index.column, index.internal_id, self.base.id())
    }

    fn with_source<R>(&self, default: R, f: impl FnOnce(&dyn AbstractItemModel) -> R) -> R {
        match self.source.get() {
            Some(source) => f(&*source.try_borrow().expect(SOURCE_BORROWED)),
            None => default,
        }
    }

    fn set_source(&self, source: Option<SharedModel>) {
        self.begin_reset_model();
        let listener: Weak<dyn ModelListener> = self.self_weak.clone();
        self.source.replace(source, listener);
        self.end_reset_model();
    }
}

impl AbstractItemModel for IdentityInner {
    fn base(&self) -> &ModelBase {
        &self.base
    }

    fn index(&self, row: i32, column: i32, parent: &ModelIndex) -> ModelIndex {
        if row < 0 || column < 0 {
            return ModelIndex::INVALID;
        }
        self.with_source(ModelIndex::INVALID, |s| {
            self.from_source(&s.index(row, column, &self.to_source(parent)))
        })
    }

    fn parent(&self, child: &ModelIndex) -> ModelIndex {
        self.with_source(ModelIndex::INVALID, |s| {
            self.from_source(&s.parent(&self.to_source(child)))
        })
    }

    fn row_count(&self, parent: &ModelIndex) -> i32 {
        if parent.is_valid() && parent.model_id != self.base.id() {
            return 0;
        }
        self.with_source(0, |s| s.row_count(&self.to_source(parent)))
    }

    fn column_count(&self, parent: &ModelIndex) -> i32 {
        if parent.is_valid() && parent.model_id != self.base.id() {
            return 0;
        }
        self.with_source(0, |s| s.column_count(&self.to_source(parent)))
    }

    fn data(&self, index: &ModelIndex, role: ItemDataRole) -> Variant {
        self.with_source(Variant::Invalid, |s| s.data(&self.to_source(index), role))
    }

    fn sibling(&self, row: i32, column: i32, index: &ModelIndex) -> ModelIndex {
        self.with_source(ModelIndex::INVALID, |s| {
            self.from_source(&s.sibling(row, column, &self.to_source(index)))
        })
    }

    fn has_children(&self, parent: &ModelIndex) -> bool {
        self.with_source(false, |s| s.has_children(&self.to_source(parent)))
    }

    fn set_data(&self, index: &ModelIndex, value: Variant, role: ItemDataRole) -> bool {
        self.with_source(false, |s| s.set_data(&self.to_source(index), value, role))
    }

    fn header_data(&self, section: i32, orientation: Orientation, role: ItemDataRole) -> Variant {
        self.with_source(Variant::Invalid, |s| {
            s.header_data(section, orientation, role)
        })
    }

    fn set_header_data(
        &self,
        section: i32,
        orientation: Orientation,
        value: Variant,
        role: ItemDataRole,
    ) -> bool {
        self.with_source(false, |s| {
            s.set_header_data(section, orientation, value, role)
        })
    }

    fn flags(&self, index: &ModelIndex) -> ItemFlags {
        self.with_source(ItemFlags::NONE, |s| s.flags(&self.to_source(index)))
    }

    fn insert_rows(&self, row: i32, count: i32, parent: &ModelIndex) -> bool {
        self.with_source(false, |s| {
            s.insert_rows(row, count, &self.to_source(parent))
        })
    }

    fn remove_rows(&self, row: i32, count: i32, parent: &ModelIndex) -> bool {
        self.with_source(false, |s| {
            s.remove_rows(row, count, &self.to_source(parent))
        })
    }

    fn insert_columns(&self, column: i32, count: i32, parent: &ModelIndex) -> bool {
        self.with_source(false, |s| {
            s.insert_columns(column, count, &self.to_source(parent))
        })
    }

    fn remove_columns(&self, column: i32, count: i32, parent: &ModelIndex) -> bool {
        self.with_source(false, |s| {
            s.remove_columns(column, count, &self.to_source(parent))
        })
    }

    fn sort(&self, column: i32, order: SortOrder) {
        self.with_source((), |s| s.sort(column, order));
    }

    fn buddy(&self, index: &ModelIndex) -> ModelIndex {
        self.with_source(ModelIndex::INVALID, |s| {
            self.from_source(&s.buddy(&self.to_source(index)))
        })
    }
}

impl ModelListener for IdentityInner {
    fn model_event(&self, event: &ModelEvent) {
        match event {
            ModelEvent::DataChanged {
                top_left,
                bottom_right,
                roles,
            } => {
                let payload = (
                    self.from_source(top_left),
                    self.from_source(bottom_right),
                    roles.clone(),
                );
                self.signals().data_changed.emit(&payload);
            }
            ModelEvent::HeaderDataChanged {
                orientation,
                first,
                last,
            } => self
                .signals()
                .header_data_changed
                .emit(&(*orientation, *first, *last)),
            ModelEvent::RowsAboutToBeInserted {
                parent,
                first,
                last,
            } => self.begin_insert_rows(&self.from_source(parent), *first, *last),
            ModelEvent::RowsInserted { .. } => self.end_insert_rows(),
            ModelEvent::RowsAboutToBeRemoved {
                parent,
                first,
                last,
            } => self.begin_remove_rows(&self.from_source(parent), *first, *last),
            ModelEvent::RowsRemoved { .. } => self.end_remove_rows(),
            ModelEvent::ColumnsAboutToBeInserted {
                parent,
                first,
                last,
            } => self.begin_insert_columns(&self.from_source(parent), *first, *last),
            ModelEvent::ColumnsInserted { .. } => self.end_insert_columns(),
            ModelEvent::ColumnsAboutToBeRemoved {
                parent,
                first,
                last,
            } => self.begin_remove_columns(&self.from_source(parent), *first, *last),
            ModelEvent::ColumnsRemoved { .. } => self.end_remove_columns(),
            ModelEvent::LayoutAboutToBeChanged => {
                self.begin_layout_change();
                let saved = self.with_source(Vec::new(), |s| {
                    self.base
                        .persistent_index_list()
                        .into_iter()
                        .map(|proxy| (proxy, PersistentModelIndex::new(s, &self.to_source(&proxy))))
                        .collect()
                });
                *self.layout_saved.borrow_mut() = saved;
            }
            ModelEvent::LayoutChanged => {
                let saved = std::mem::take(&mut *self.layout_saved.borrow_mut());
                let (from, to): (Vec<ModelIndex>, Vec<ModelIndex>) = saved
                    .iter()
                    .map(|(proxy, source)| (*proxy, self.from_source(&source.index())))
                    .unzip();
                self.base.change_persistent_index_list(&from, &to);
                self.end_layout_change();
            }
            ModelEvent::ModelAboutToBeReset => self.begin_reset_model(),
            ModelEvent::ModelReset => self.end_reset_model(),
        }
    }
}

/// Proxy that forwards its source unchanged (`QIdentityProxyModel`); a base
/// for proxies that only alter data, not structure.
pub struct IdentityProxyModel {
    inner: Rc<IdentityInner>,
}

delegate_item_model!(IdentityProxyModel, inner);

impl Default for IdentityProxyModel {
    fn default() -> Self {
        Self::new()
    }
}

impl IdentityProxyModel {
    pub fn new() -> Self {
        Self {
            inner: Rc::new_cyclic(|weak| IdentityInner {
                base: ModelBase::new(),
                self_weak: weak.clone(),
                source: SourceSlot::new(),
                layout_saved: RefCell::new(Vec::new()),
            }),
        }
    }

    pub fn with_source(source: SharedModel) -> Self {
        let proxy = Self::new();
        proxy.inner.set_source(Some(source));
        proxy
    }
}

impl AbstractProxyModel for IdentityProxyModel {
    fn source_model(&self) -> Option<SharedModel> {
        self.inner.source.get()
    }

    fn set_source_model(&self, source: Option<SharedModel>) {
        self.inner.set_source(source);
    }

    fn map_to_source(&self, proxy_index: &ModelIndex) -> ModelIndex {
        self.inner.to_source(proxy_index)
    }

    fn map_from_source(&self, source_index: &ModelIndex) -> ModelIndex {
        self.inner.from_source(source_index)
    }
}

// ===========================================================================
// SortFilterProxyModel
// ===========================================================================

/// Interpretation of the filter pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum FilterSyntax {
    /// Substring match.
    #[default]
    FixedString,
    /// Shell wildcard (`*`, `?`, `[...]`) matched anywhere in the text.
    Wildcard,
    /// Regular expression matched anywhere in the text.
    RegularExpression,
}

struct Filter {
    syntax: FilterSyntax,
    pattern: String,
    case: CaseSensitivity,
    folded: String,
    regex: Option<Regex>,
}

impl Filter {
    fn new(syntax: FilterSyntax, pattern: &str, case: CaseSensitivity) -> Self {
        let insensitive = case == CaseSensitivity::Insensitive;
        let regex = match syntax {
            FilterSyntax::FixedString => None,
            FilterSyntax::Wildcard => Regex::from_wildcard(pattern, false, insensitive).ok(),
            FilterSyntax::RegularExpression => Regex::new(pattern, insensitive).ok(),
        };
        Self {
            syntax,
            pattern: pattern.to_string(),
            case,
            folded: pattern.to_lowercase(),
            regex,
        }
    }

    fn is_empty(&self) -> bool {
        self.pattern.is_empty()
    }

    fn matches(&self, text: &str) -> bool {
        match self.syntax {
            FilterSyntax::FixedString => match self.case {
                CaseSensitivity::Sensitive => text.contains(&self.pattern),
                CaseSensitivity::Insensitive => text.to_lowercase().contains(&self.folded),
            },
            FilterSyntax::Wildcard | FilterSyntax::RegularExpression => {
                self.regex.as_ref().is_some_and(|rx| rx.is_match(text))
            }
        }
    }
}

/// Custom row/column acceptance: `(source_model, source_row_or_column, source_parent)`.
pub type FilterAcceptsFn = dyn Fn(&dyn AbstractItemModel, i32, &ModelIndex) -> bool;
/// Custom ordering: `(source_model, left, right)` returns `left < right`.
pub type LessThanFn = dyn Fn(&dyn AbstractItemModel, &ModelIndex, &ModelIndex) -> bool;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Dir {
    Rows,
    Columns,
}

/// Proxy/source mapping for the children of one source parent.
struct Mapping {
    source_parent: ModelIndex,
    /// proxy row -> source row
    source_rows: Vec<i32>,
    /// proxy column -> source column
    source_columns: Vec<i32>,
    /// source row -> proxy row (or -1)
    proxy_rows: Vec<i32>,
    /// source column -> proxy column (or -1)
    proxy_columns: Vec<i32>,
    /// Source indexes of children that have their own mapping.
    mapped_children: Vec<ModelIndex>,
}

impl Mapping {
    /// `(source_to_proxy, proxy_to_source)` for `dir`.
    fn lists(&mut self, dir: Dir) -> (&mut Vec<i32>, &mut Vec<i32>) {
        match dir {
            Dir::Rows => (&mut self.proxy_rows, &mut self.source_rows),
            Dir::Columns => (&mut self.proxy_columns, &mut self.source_columns),
        }
    }

    fn source_to_proxy(&self, dir: Dir) -> &Vec<i32> {
        match dir {
            Dir::Rows => &self.proxy_rows,
            Dir::Columns => &self.proxy_columns,
        }
    }

    fn proxy_to_source(&self, dir: Dir) -> &Vec<i32> {
        match dir {
            Dir::Rows => &self.source_rows,
            Dir::Columns => &self.source_columns,
        }
    }
}

struct MappingTable {
    by_parent: HashMap<ModelIndex, u64>,
    maps: HashMap<u64, Mapping>,
    next_id: u64,
}

impl MappingTable {
    fn clear(&mut self) {
        self.by_parent.clear();
        self.maps.clear();
    }
}

fn build_source_to_proxy(proxy_to_source: &[i32], source_to_proxy: &mut [i32], start: usize) {
    if start == 0 {
        source_to_proxy.fill(-1);
    }
    for (i, &source) in proxy_to_source.iter().enumerate().skip(start) {
        if let Some(slot) = source_to_proxy.get_mut(source as usize) {
            *slot = i as i32;
        }
    }
}

/// Sorted, merged proxy intervals covering `source_items`.
fn proxy_intervals_for_source_items(
    source_to_proxy: &[i32],
    source_items: &[i32],
) -> Vec<(i32, i32)> {
    let proxy_of = |s: i32| source_to_proxy.get(s as usize).copied().unwrap_or(-1);
    let mut intervals: Vec<(i32, i32)> = Vec::new();
    let mut i = 0;
    while i < source_items.len() {
        let first = proxy_of(source_items[i]);
        i += 1;
        if first < 0 {
            continue;
        }
        let mut last = first;
        while i < source_items.len() && proxy_of(source_items[i]) == last + 1 {
            last += 1;
            i += 1;
        }
        intervals.push((first, last));
    }
    intervals.sort_unstable();
    let mut merged: Vec<(i32, i32)> = Vec::with_capacity(intervals.len());
    for (start, end) in intervals {
        match merged.last_mut() {
            Some(previous) if start == previous.1 + 1 => previous.1 = end,
            _ => merged.push((start, end)),
        }
    }
    merged
}

struct SfpmInner {
    base: ModelBase,
    self_weak: Weak<SfpmInner>,
    source: SourceSlot,
    maps: RefCell<MappingTable>,
    dynamic_sort_filter: Cell<bool>,
    proxy_sort_column: Cell<i32>,
    source_sort_column: Cell<i32>,
    sort_order: Cell<SortOrder>,
    sort_role: Cell<ItemDataRole>,
    sort_case: Cell<CaseSensitivity>,
    filter_key_column: Cell<i32>,
    filter_role: Cell<ItemDataRole>,
    filter: RefCell<Rc<Filter>>,
    recursive_filtering: Cell<bool>,
    auto_accept_child_rows: Cell<bool>,
    filter_row_fn: RefCell<Option<Rc<FilterAcceptsFn>>>,
    filter_column_fn: RefCell<Option<Rc<FilterAcceptsFn>>>,
    less_than_fn: RefCell<Option<Rc<LessThanFn>>>,
    saved_persistent: RefCell<Vec<(ModelIndex, PersistentModelIndex)>>,
    items_being_removed: Cell<Option<(ModelIndex, i32, i32)>>,
    complete_insert: Cell<bool>,
    last_top_source: RefCell<PersistentModelIndex>,
}

impl SfpmInner {
    fn new() -> Rc<Self> {
        Rc::new_cyclic(|weak| SfpmInner {
            base: ModelBase::new(),
            self_weak: weak.clone(),
            source: SourceSlot::new(),
            maps: RefCell::new(MappingTable {
                by_parent: HashMap::new(),
                maps: HashMap::new(),
                next_id: 1,
            }),
            dynamic_sort_filter: Cell::new(true),
            proxy_sort_column: Cell::new(-1),
            source_sort_column: Cell::new(-1),
            sort_order: Cell::new(SortOrder::Ascending),
            sort_role: Cell::new(ItemDataRole::Display),
            sort_case: Cell::new(CaseSensitivity::Sensitive),
            filter_key_column: Cell::new(0),
            filter_role: Cell::new(ItemDataRole::Display),
            filter: RefCell::new(Rc::new(Filter::new(
                FilterSyntax::FixedString,
                "",
                CaseSensitivity::Sensitive,
            ))),
            recursive_filtering: Cell::new(false),
            auto_accept_child_rows: Cell::new(false),
            filter_row_fn: RefCell::new(None),
            filter_column_fn: RefCell::new(None),
            less_than_fn: RefCell::new(None),
            saved_persistent: RefCell::new(Vec::new()),
            items_being_removed: Cell::new(None),
            complete_insert: Cell::new(false),
            last_top_source: RefCell::new(PersistentModelIndex::default()),
        })
    }

    /// Runs `f` with the source model, if any.
    fn with_source<R>(&self, default: R, f: impl FnOnce(&dyn AbstractItemModel) -> R) -> R {
        match self.source.get() {
            Some(source) => f(&*source.try_borrow().expect(SOURCE_BORROWED)),
            None => default,
        }
    }

    fn with_mapping<R>(&self, id: u64, f: impl FnOnce(&mut Mapping) -> R) -> Option<R> {
        self.maps.borrow_mut().maps.get_mut(&id).map(f)
    }

    fn mapping_id(&self, source_parent: &ModelIndex) -> Option<u64> {
        self.maps.borrow().by_parent.get(source_parent).copied()
    }

    // ----- filtering & ordering ------------------------------------------------

    fn default_filter_accepts_row(
        &self,
        src: &dyn AbstractItemModel,
        row: i32,
        source_parent: &ModelIndex,
    ) -> bool {
        let filter = self.filter.borrow().clone();
        if filter.is_empty() {
            return true;
        }
        let role = self.filter_role.get();
        let key_column = self.filter_key_column.get();
        let columns = src.column_count(source_parent);
        let matches = |column: i32| {
            let index = src.index(row, column, source_parent);
            filter.matches(&src.data(&index, role).to_string_lossy())
        };
        if key_column == -1 {
            (0..columns).any(matches)
        } else if key_column >= columns {
            true
        } else {
            matches(key_column)
        }
    }

    fn filter_accepts_row(
        &self,
        src: &dyn AbstractItemModel,
        row: i32,
        source_parent: &ModelIndex,
    ) -> bool {
        let custom = self.filter_row_fn.borrow().clone();
        match custom {
            Some(f) => f(src, row, source_parent),
            None => self.default_filter_accepts_row(src, row, source_parent),
        }
    }

    fn filter_accepts_column(
        &self,
        src: &dyn AbstractItemModel,
        column: i32,
        source_parent: &ModelIndex,
    ) -> bool {
        let custom = self.filter_column_fn.borrow().clone();
        custom.is_none_or(|f| f(src, column, source_parent))
    }

    fn less_than(
        &self,
        src: &dyn AbstractItemModel,
        left: &ModelIndex,
        right: &ModelIndex,
    ) -> bool {
        let custom = self.less_than_fn.borrow().clone();
        match custom {
            Some(f) => f(src, left, right),
            None => {
                let role = self.sort_role.get();
                variant_less_than(
                    &src.data(left, role),
                    &src.data(right, role),
                    self.sort_case.get(),
                )
            }
        }
    }

    fn recursive_parent_accepts_row(
        &self,
        src: &dyn AbstractItemModel,
        source_parent: &ModelIndex,
    ) -> bool {
        let mut current = *source_parent;
        while current.is_valid() {
            let grand_parent = src.parent(&current);
            if self.filter_accepts_row(src, current.row, &grand_parent) {
                return true;
            }
            current = grand_parent;
        }
        false
    }

    fn recursive_child_accepts_row(
        &self,
        src: &dyn AbstractItemModel,
        row: i32,
        source_parent: &ModelIndex,
    ) -> bool {
        if src.column_count(source_parent) == 0 {
            return false;
        }
        let index = src.index(row, 0, source_parent);
        (0..src.row_count(&index)).any(|child| {
            self.filter_accepts_row(src, child, &index)
                || self.recursive_child_accepts_row(src, child, &index)
        })
    }

    fn filter_accepts_row_internal(
        &self,
        src: &dyn AbstractItemModel,
        row: i32,
        source_parent: &ModelIndex,
    ) -> bool {
        self.filter_accepts_row(src, row, source_parent)
            || (self.auto_accept_child_rows.get()
                && self.recursive_parent_accepts_row(src, source_parent))
            || (self.recursive_filtering.get()
                && self.recursive_child_accepts_row(src, row, source_parent))
    }

    fn accepts(
        &self,
        src: &dyn AbstractItemModel,
        dir: Dir,
        item: i32,
        source_parent: &ModelIndex,
    ) -> bool {
        match dir {
            Dir::Rows => self.filter_accepts_row_internal(src, item, source_parent),
            Dir::Columns => self.filter_accepts_column(src, item, source_parent),
        }
    }

    fn sort_source_rows(
        &self,
        src: &dyn AbstractItemModel,
        rows: &mut [i32],
        source_parent: &ModelIndex,
    ) {
        let column = self.source_sort_column.get();
        let ascending = self.sort_order.get() == SortOrder::Ascending;
        if column >= 0 {
            let index = |row: i32| src.index(row, column, source_parent);
            stable_sort_by_less(rows, &mut |a, b| {
                let (left, right) = (index(*a), index(*b));
                if ascending {
                    self.less_than(src, &left, &right)
                } else {
                    self.less_than(src, &right, &left)
                }
            });
        } else if ascending {
            rows.sort_unstable();
        } else {
            rows.sort_unstable_by(|a, b| b.cmp(a));
        }
    }

    // ----- mapping management ---------------------------------------------------

    fn create_mapping(&self, src: &dyn AbstractItemModel, source_parent: &ModelIndex) -> u64 {
        if let Some(id) = self.mapping_id(source_parent) {
            return id;
        }
        let row_count = src.row_count(source_parent).max(0);
        let column_count = src.column_count(source_parent).max(0);
        let mut source_rows: Vec<i32> = (0..row_count)
            .filter(|&row| self.filter_accepts_row_internal(src, row, source_parent))
            .collect();
        let source_columns: Vec<i32> = (0..column_count)
            .filter(|&column| self.filter_accepts_column(src, column, source_parent))
            .collect();
        self.sort_source_rows(src, &mut source_rows, source_parent);
        let mut proxy_rows = vec![-1; row_count as usize];
        build_source_to_proxy(&source_rows, &mut proxy_rows, 0);
        let mut proxy_columns = vec![-1; column_count as usize];
        build_source_to_proxy(&source_columns, &mut proxy_columns, 0);

        if source_parent.is_valid() {
            let grand_parent = src.parent(source_parent);
            let grand_id = self.create_mapping(src, &grand_parent);
            self.with_mapping(grand_id, |m| m.mapped_children.push(*source_parent));
        }

        let mut table = self.maps.borrow_mut();
        if let Some(&id) = table.by_parent.get(source_parent) {
            return id;
        }
        let id = table.next_id;
        table.next_id += 1;
        table.by_parent.insert(*source_parent, id);
        table.maps.insert(
            id,
            Mapping {
                source_parent: *source_parent,
                source_rows,
                source_columns,
                proxy_rows,
                proxy_columns,
                mapped_children: Vec::new(),
            },
        );
        id
    }

    /// `true` if `source_parent` is visible in a mapped grand parent.
    fn is_visible_in(&self, grand_id: u64, source_parent: &ModelIndex) -> bool {
        let table = self.maps.borrow();
        table.maps.get(&grand_id).is_some_and(|m| {
            m.proxy_rows
                .get(source_parent.row as usize)
                .is_some_and(|&r| r != -1)
                && m.proxy_columns
                    .get(source_parent.column as usize)
                    .is_some_and(|&c| c != -1)
        })
    }

    fn create_mapping_recursive(
        &self,
        src: &dyn AbstractItemModel,
        source_parent: &ModelIndex,
    ) -> Option<u64> {
        if source_parent.is_valid() {
            let grand_parent = src.parent(source_parent);
            let grand_id = match self.mapping_id(&grand_parent) {
                Some(id) => id,
                None => self.create_mapping_recursive(src, &grand_parent)?,
            };
            if !self.is_visible_in(grand_id, source_parent) {
                return None;
            }
        }
        Some(self.create_mapping(src, source_parent))
    }

    fn can_create_mapping(&self, src: &dyn AbstractItemModel, source_parent: &ModelIndex) -> bool {
        if !source_parent.is_valid() {
            return true;
        }
        let grand_parent = src.parent(source_parent);
        self.mapping_id(&grand_parent)
            .is_some_and(|grand_id| self.is_visible_in(grand_id, source_parent))
    }

    fn remove_from_mapping(&self, source_parent: &ModelIndex) {
        let removed = {
            let mut table = self.maps.borrow_mut();
            let id = table.by_parent.remove(source_parent);
            id.and_then(|id| table.maps.remove(&id))
        };
        if let Some(mapping) = removed {
            for child in &mapping.mapped_children {
                self.remove_from_mapping(child);
            }
        }
    }

    fn proxy_to_source(&self, src: &dyn AbstractItemModel, proxy: &ModelIndex) -> ModelIndex {
        if !proxy.is_valid() || proxy.model_id != self.base.id() {
            return ModelIndex::INVALID;
        }
        let located = {
            let table = self.maps.borrow();
            table.maps.get(&proxy.internal_id).and_then(|m| {
                let row = *m.source_rows.get(proxy.row as usize)?;
                let column = *m.source_columns.get(proxy.column as usize)?;
                Some((m.source_parent, row, column))
            })
        };
        match located {
            Some((parent, row, column)) => src.index(row, column, &parent),
            None => ModelIndex::INVALID,
        }
    }

    fn source_to_proxy(&self, src: &dyn AbstractItemModel, source: &ModelIndex) -> ModelIndex {
        if !source.is_valid() || source.model_id != src.model_id() {
            return ModelIndex::INVALID;
        }
        let source_parent = src.parent(source);
        let Some(id) = self.create_mapping_recursive(src, &source_parent) else {
            return ModelIndex::INVALID;
        };
        let table = self.maps.borrow();
        let Some(m) = table.maps.get(&id) else {
            return ModelIndex::INVALID;
        };
        let row = m.proxy_rows.get(source.row as usize).copied().unwrap_or(-1);
        let column = m
            .proxy_columns
            .get(source.column as usize)
            .copied()
            .unwrap_or(-1);
        if row < 0 || column < 0 {
            return ModelIndex::INVALID;
        }
        ModelIndex::new(row, column, id, self.base.id())
    }

    fn store_persistent_indexes(
        &self,
        src: &dyn AbstractItemModel,
    ) -> Vec<(ModelIndex, PersistentModelIndex)> {
        self.base
            .persistent_index_list()
            .into_iter()
            .map(|proxy| {
                (
                    proxy,
                    PersistentModelIndex::new(src, &self.proxy_to_source(src, &proxy)),
                )
            })
            .collect()
    }

    fn update_persistent_indexes(
        &self,
        src: &dyn AbstractItemModel,
        saved: Vec<(ModelIndex, PersistentModelIndex)>,
    ) {
        let (from, to): (Vec<ModelIndex>, Vec<ModelIndex>) = saved
            .iter()
            .map(|(proxy, source)| (*proxy, self.source_to_proxy(src, &source.index())))
            .unzip();
        self.base.change_persistent_index_list(&from, &to);
    }

    fn clear_mapping(&self, src: &dyn AbstractItemModel) {
        let saved = self.store_persistent_indexes(src);
        self.maps.borrow_mut().clear();
        if self.dynamic_sort_filter.get() {
            self.source_sort_column
                .set(self.find_source_sort_column(src));
        }
        self.update_persistent_indexes(src, saved);
    }

    fn update_source_sort_column(&self, src: &dyn AbstractItemModel) -> bool {
        let old = self.source_sort_column.get();
        let proxy_column = self.proxy_sort_column.get();
        let new = if proxy_column < 0 {
            -1
        } else {
            let id = self.create_mapping(src, &ModelIndex::INVALID);
            let table = self.maps.borrow();
            table
                .maps
                .get(&id)
                .and_then(|m| m.source_columns.get(proxy_column as usize).copied())
                .unwrap_or(-1)
        };
        self.source_sort_column.set(new);
        old != new
    }

    fn find_source_sort_column(&self, src: &dyn AbstractItemModel) -> i32 {
        let proxy_column = self.proxy_sort_column.get();
        if proxy_column < 0 {
            return -1;
        }
        let root = ModelIndex::INVALID;
        let mut accepted = -1;
        for column in 0..src.column_count(&root) {
            if self.filter_accepts_column(src, column, &root) {
                accepted += 1;
                if accepted == proxy_column {
                    return column;
                }
            }
        }
        -1
    }

    /// Re-sorts every existing mapping, preserving persistent indexes.
    fn do_sort(&self, src: &dyn AbstractItemModel) {
        self.begin_layout_change();
        let saved = self.store_persistent_indexes(src);
        let entries: Vec<(u64, ModelIndex, Vec<i32>)> = self
            .maps
            .borrow()
            .maps
            .iter()
            .map(|(id, m)| (*id, m.source_parent, m.source_rows.clone()))
            .collect();
        for (id, source_parent, mut rows) in entries {
            self.sort_source_rows(src, &mut rows, &source_parent);
            self.with_mapping(id, move |m| {
                m.source_rows = rows;
                build_source_to_proxy(&m.source_rows, &mut m.proxy_rows, 0);
            });
        }
        self.update_persistent_indexes(src, saved);
        self.end_layout_change();
    }

    // ----- incremental updates ---------------------------------------------------

    fn remove_proxy_interval(
        &self,
        id: u64,
        start: i32,
        end: i32,
        proxy_parent: &ModelIndex,
        dir: Dir,
        emit: bool,
    ) {
        if emit {
            match dir {
                Dir::Rows => self.begin_remove_rows(proxy_parent, start, end),
                Dir::Columns => self.begin_remove_columns(proxy_parent, start, end),
            }
        }
        self.with_mapping(id, |m| {
            let (source_to_proxy, proxy_to_source) = m.lists(dir);
            let (start, end) = (start as usize, end as usize);
            for &source in &proxy_to_source[start..=end] {
                if let Some(slot) = source_to_proxy.get_mut(source as usize) {
                    *slot = -1;
                }
            }
            proxy_to_source.drain(start..=end);
            build_source_to_proxy(proxy_to_source, source_to_proxy, start);
        });
        if emit {
            match dir {
                Dir::Rows => self.end_remove_rows(),
                Dir::Columns => self.end_remove_columns(),
            }
        }
    }

    fn remove_source_items(
        &self,
        src: &dyn AbstractItemModel,
        id: u64,
        items: &[i32],
        source_parent: &ModelIndex,
        dir: Dir,
        emit: bool,
    ) {
        let proxy_parent = self.source_to_proxy(src, source_parent);
        if !proxy_parent.is_valid() && source_parent.is_valid() {
            self.with_mapping(id, |m| {
                let (source_to_proxy, proxy_to_source) = m.lists(dir);
                proxy_to_source.clear();
                source_to_proxy.fill(-1);
            });
            return;
        }
        let intervals = {
            let table = self.maps.borrow();
            match table.maps.get(&id) {
                Some(m) => proxy_intervals_for_source_items(m.source_to_proxy(dir), items),
                None => return,
            }
        };
        for &(start, end) in intervals.iter().rev() {
            self.remove_proxy_interval(id, start, end, &proxy_parent, dir, emit);
        }
    }

    fn proxy_intervals_for_source_items_to_add(
        &self,
        src: &dyn AbstractItemModel,
        proxy_to_source: &[i32],
        items: &[i32],
        source_parent: &ModelIndex,
        dir: Dir,
    ) -> Vec<(usize, Vec<i32>)> {
        let sort_column = self.source_sort_column.get();
        let compare = dir == Dir::Rows && sort_column >= 0 && self.dynamic_sort_filter.get();
        let ascending = self.sort_order.get() == SortOrder::Ascending;
        // `true` if source item `a` must come before existing item `b`.
        let before = |a: i32, b: i32| -> bool {
            if compare {
                let (ia, ib) = (
                    src.index(a, sort_column, source_parent),
                    src.index(b, sort_column, source_parent),
                );
                if ascending {
                    self.less_than(src, &ia, &ib)
                } else {
                    self.less_than(src, &ib, &ia)
                }
            } else {
                a < b
            }
        };
        let mut intervals = Vec::new();
        let mut proxy_low = 0usize;
        let mut i = 0;
        while i < items.len() {
            let first = items[i];
            i += 1;
            let mut interval = vec![first];
            let (mut low, mut high) = (proxy_low as isize, proxy_to_source.len() as isize - 1);
            while low <= high {
                let middle = (low + high) / 2;
                if before(first, proxy_to_source[middle as usize]) {
                    high = middle - 1;
                } else {
                    low = middle + 1;
                }
            }
            proxy_low = low as usize;
            if proxy_low >= proxy_to_source.len() {
                interval.extend_from_slice(&items[i..]);
                i = items.len();
            } else {
                let existing = proxy_to_source[proxy_low];
                while i < items.len() && !before(existing, items[i]) {
                    interval.push(items[i]);
                    i += 1;
                }
            }
            intervals.push((proxy_low, interval));
        }
        intervals
    }

    fn insert_source_items(
        &self,
        src: &dyn AbstractItemModel,
        id: u64,
        items: &[i32],
        source_parent: &ModelIndex,
        dir: Dir,
        emit: bool,
    ) {
        let proxy_parent = self.source_to_proxy(src, source_parent);
        if !proxy_parent.is_valid() && source_parent.is_valid() {
            return;
        }
        let Some(proxy_to_source) = self.with_mapping(id, |m| m.proxy_to_source(dir).clone())
        else {
            return;
        };
        let intervals = self.proxy_intervals_for_source_items_to_add(
            src,
            &proxy_to_source,
            items,
            source_parent,
            dir,
        );
        for (start, interval_items) in intervals.iter().rev() {
            let (first, last) = (*start as i32, (*start + interval_items.len()) as i32 - 1);
            if emit {
                match dir {
                    Dir::Rows => self.begin_insert_rows(&proxy_parent, first, last),
                    Dir::Columns => self.begin_insert_columns(&proxy_parent, first, last),
                }
            }
            self.with_mapping(id, |m| {
                let (source_to_proxy, proxy_to_source) = m.lists(dir);
                proxy_to_source.splice(*start..*start, interval_items.iter().copied());
                build_source_to_proxy(proxy_to_source, source_to_proxy, *start);
            });
            if emit {
                match dir {
                    Dir::Rows => self.end_insert_rows(),
                    Dir::Columns => self.end_insert_columns(),
                }
            }
        }
    }

    /// Re-keys or drops child mappings after items shifted in `source_parent`.
    #[allow(clippy::too_many_arguments)]
    fn update_children_mapping(
        &self,
        src: &dyn AbstractItemModel,
        source_parent: &ModelIndex,
        id: u64,
        dir: Dir,
        start: i32,
        end: i32,
        delta: i32,
        remove: bool,
    ) {
        let Some(children) = self.with_mapping(id, |m| std::mem::take(&mut m.mapped_children))
        else {
            return;
        };
        let mut kept = Vec::with_capacity(children.len());
        let mut removed = Vec::new();
        let mut moved = Vec::new();
        for child in children {
            let position = if dir == Dir::Rows {
                child.row
            } else {
                child.column
            };
            if position < start {
                kept.push(child);
            } else if remove && position <= end {
                removed.push(child);
            } else {
                let new_position = if remove {
                    position - delta
                } else {
                    position + delta
                };
                let new_index = match dir {
                    Dir::Rows => src.index(new_position, child.column, source_parent),
                    Dir::Columns => src.index(child.row, new_position, source_parent),
                };
                kept.push(new_index);
                moved.push((child, new_index));
            }
        }
        self.with_mapping(id, |m| m.mapped_children = kept);
        for child in &removed {
            self.remove_from_mapping(child);
        }
        let mut table = self.maps.borrow_mut();
        let rekeyed: Vec<(ModelIndex, u64)> = moved
            .iter()
            .filter_map(|(old, new)| table.by_parent.remove(old).map(|child_id| (*new, child_id)))
            .collect();
        for (new_index, child_id) in rekeyed {
            table.by_parent.insert(new_index, child_id);
            if let Some(m) = table.maps.get_mut(&child_id) {
                m.source_parent = new_index;
            }
        }
    }

    fn source_items_inserted(
        &self,
        src: &dyn AbstractItemModel,
        source_parent: &ModelIndex,
        start: i32,
        end: i32,
        dir: Dir,
    ) {
        if start < 0 || end < start {
            return;
        }
        let Some(id) = self.mapping_id(source_parent) else {
            if !self.can_create_mapping(src, source_parent) {
                return;
            }
            let id = self.create_mapping(src, source_parent);
            let proxy_parent = self.source_to_proxy(src, source_parent);
            let (rows, columns) = self
                .with_mapping(id, |m| {
                    (m.source_rows.len() as i32, m.source_columns.len() as i32)
                })
                .unwrap_or((0, 0));
            if rows > 0 {
                self.begin_insert_rows(&proxy_parent, 0, rows - 1);
                self.end_insert_rows();
            }
            if columns > 0 {
                self.begin_insert_columns(&proxy_parent, 0, columns - 1);
                self.end_insert_columns();
            }
            return;
        };
        let delta = end - start + 1;
        self.update_children_mapping(src, source_parent, id, dir, start, end, delta, false);
        let expanded = self.with_mapping(id, |m| {
            let (source_to_proxy, proxy_to_source) = m.lists(dir);
            let old_count = source_to_proxy.len();
            let at = start as usize;
            if at > old_count {
                return false;
            }
            source_to_proxy.splice(at..at, std::iter::repeat_n(-1, delta as usize));
            if at < old_count {
                for source in proxy_to_source.iter_mut() {
                    if *source >= start {
                        *source += delta;
                    }
                }
                build_source_to_proxy(proxy_to_source, source_to_proxy, 0);
            }
            true
        });
        if expanded != Some(true) {
            self.remove_from_mapping(source_parent);
            return;
        }
        let mut items: Vec<i32> = (start..=end)
            .filter(|&item| self.accepts(src, dir, item, source_parent))
            .collect();

        let total = match dir {
            Dir::Rows => src.row_count(source_parent),
            Dir::Columns => src.column_count(source_parent),
        };
        if total == delta {
            // Items were inserted where there were none before: make sure the
            // orthogonal direction is mapped too.
            let ortho = if dir == Dir::Rows {
                Dir::Columns
            } else {
                Dir::Rows
            };
            let ortho_empty = self
                .with_mapping(id, |m| m.source_to_proxy(ortho).is_empty())
                .unwrap_or(false);
            if ortho_empty {
                let ortho_total = match ortho {
                    Dir::Rows => src.row_count(source_parent),
                    Dir::Columns => src.column_count(source_parent),
                }
                .max(0);
                let mut accepted: Vec<i32> = (0..ortho_total)
                    .filter(|&item| self.accepts(src, ortho, item, source_parent))
                    .collect();
                if ortho == Dir::Rows {
                    self.sort_source_rows(src, &mut accepted, source_parent);
                }
                self.with_mapping(id, |m| {
                    let (source_to_proxy, proxy_to_source) = m.lists(ortho);
                    *source_to_proxy = vec![-1; ortho_total as usize];
                    *proxy_to_source = accepted;
                    build_source_to_proxy(proxy_to_source, source_to_proxy, 0);
                });
            }
        }
        if dir == Dir::Rows {
            self.sort_source_rows(src, &mut items, source_parent);
        }
        self.insert_source_items(src, id, &items, source_parent, dir, true);
    }

    fn source_items_about_to_be_removed(
        &self,
        src: &dyn AbstractItemModel,
        source_parent: &ModelIndex,
        start: i32,
        end: i32,
        dir: Dir,
    ) {
        if start < 0 || end < start {
            return;
        }
        let Some(id) = self.mapping_id(source_parent) else {
            return;
        };
        let items: Vec<i32> = self
            .with_mapping(id, |m| {
                m.proxy_to_source(dir)
                    .iter()
                    .copied()
                    .filter(|&s| s >= start && s <= end)
                    .collect()
            })
            .unwrap_or_default();
        self.remove_source_items(src, id, &items, source_parent, dir, true);
    }

    fn source_items_removed(
        &self,
        src: &dyn AbstractItemModel,
        source_parent: &ModelIndex,
        start: i32,
        end: i32,
        dir: Dir,
    ) {
        if start < 0 || end < start {
            return;
        }
        let Some(id) = self.mapping_id(source_parent) else {
            return;
        };
        let outcome = self.with_mapping(id, |m| {
            let (source_to_proxy, proxy_to_source) = m.lists(dir);
            let end = end.min(source_to_proxy.len() as i32 - 1);
            if end < start {
                return Some((end, 0));
            }
            let delta = end - start + 1;
            source_to_proxy.drain(start as usize..=end as usize);
            if proxy_to_source.len() > source_to_proxy.len() {
                return None;
            }
            for source in proxy_to_source.iter_mut() {
                if *source >= start {
                    *source -= delta;
                }
            }
            build_source_to_proxy(proxy_to_source, source_to_proxy, 0);
            Some((end, delta))
        });
        match outcome {
            Some(Some((end, delta))) => {
                if delta > 0 {
                    self.update_children_mapping(
                        src,
                        source_parent,
                        id,
                        dir,
                        start,
                        end,
                        delta,
                        true,
                    );
                }
            }
            Some(None) => {
                // Inconsistent change notifications: rebuild this level.
                self.begin_reset_model();
                self.remove_from_mapping(source_parent);
                self.end_reset_model();
            }
            None => {}
        }
    }

    fn needs_reorder(
        &self,
        src: &dyn AbstractItemModel,
        id: u64,
        rows: &[i32],
        source_parent: &ModelIndex,
    ) -> bool {
        let column = self.source_sort_column.get();
        let ascending = self.sort_order.get() == SortOrder::Ascending;
        let Some((proxy_rows, source_rows)) =
            self.with_mapping(id, |m| (m.proxy_rows.clone(), m.source_rows.clone()))
        else {
            return false;
        };
        let index = |row: i32| src.index(row, column, source_parent);
        let lt = |a: &ModelIndex, b: &ModelIndex| {
            if ascending {
                self.less_than(src, a, b)
            } else {
                self.less_than(src, b, a)
            }
        };
        rows.iter().any(|&row| {
            let Some(&proxy_row) = proxy_rows.get(row as usize) else {
                return false;
            };
            if proxy_row < 0 {
                return false;
            }
            let current = index(row);
            let p = proxy_row as usize;
            (p > 0 && lt(&current, &index(source_rows[p - 1])))
                || (p + 1 < source_rows.len() && lt(&index(source_rows[p + 1]), &current))
        })
    }

    fn source_data_changed(
        &self,
        src: &dyn AbstractItemModel,
        top_left: &ModelIndex,
        bottom_right: &ModelIndex,
        roles: &[i32],
    ) {
        if !top_left.is_valid() || !bottom_right.is_valid() {
            return;
        }
        let mut changes = vec![(*top_left, *bottom_right)];
        if self.recursive_filtering.get()
            && (roles.is_empty() || roles.contains(&self.filter_role.get().to_i32()))
        {
            let mut ancestor = src.parent(top_left);
            while ancestor.is_valid() {
                changes.push((ancestor, ancestor));
                ancestor = src.parent(&ancestor);
            }
        }
        for (top_left, bottom_right) in changes {
            let source_parent = src.parent(&top_left);
            let mut change_in_unmapped_parent = false;
            let id = match self.mapping_id(&source_parent) {
                Some(id) => id,
                None => match self.create_mapping_recursive(src, &source_parent) {
                    Some(id) => {
                        change_in_unmapped_parent = true;
                        id
                    }
                    None => continue,
                },
            };
            let Some(proxy_rows) = self.with_mapping(id, |m| m.proxy_rows.clone()) else {
                continue;
            };
            let sort_column = self.source_sort_column.get();
            let dynamic = self.dynamic_sort_filter.get();
            let being_removed = self.items_being_removed.get();
            let mut to_remove = Vec::new();
            let mut to_insert = Vec::new();
            let mut changed = Vec::new();
            let mut to_resort = Vec::new();
            let last = bottom_right.row.min(proxy_rows.len() as i32 - 1);
            for row in top_left.row..=last {
                let visible = proxy_rows[row as usize] != -1;
                if dynamic && !change_in_unmapped_parent {
                    if visible {
                        if !self.filter_accepts_row_internal(src, row, &source_parent) {
                            to_remove.push(row);
                        } else if sort_column >= top_left.column
                            && sort_column <= bottom_right.column
                        {
                            to_resort.push(row);
                        } else {
                            changed.push(row);
                        }
                    } else {
                        let removing = being_removed.is_some_and(|(parent, first, last)| {
                            parent == source_parent && row >= first && row <= last
                        });
                        if !removing && self.filter_accepts_row_internal(src, row, &source_parent) {
                            to_insert.push(row);
                        }
                    }
                } else if visible {
                    changed.push(row);
                }
            }

            if !to_remove.is_empty() {
                self.remove_source_items(src, id, &to_remove, &source_parent, Dir::Rows, true);
                let removed: HashSet<i32> = to_remove.iter().copied().collect();
                let dropped = self
                    .with_mapping(id, |m| {
                        let (gone, kept): (Vec<ModelIndex>, Vec<ModelIndex>) = m
                            .mapped_children
                            .iter()
                            .partition(|child| removed.contains(&child.row));
                        m.mapped_children = kept;
                        gone
                    })
                    .unwrap_or_default();
                for child in dropped.iter().rev() {
                    self.remove_from_mapping(child);
                }
            }

            if !to_resort.is_empty() {
                if self.needs_reorder(src, id, &to_resort, &source_parent) {
                    self.begin_layout_change();
                    let saved = self.store_persistent_indexes(src);
                    self.remove_source_items(src, id, &to_resort, &source_parent, Dir::Rows, false);
                    let mut sorted = to_resort.clone();
                    self.sort_source_rows(src, &mut sorted, &source_parent);
                    self.insert_source_items(src, id, &sorted, &source_parent, Dir::Rows, false);
                    self.update_persistent_indexes(src, saved);
                    self.end_layout_change();
                }
                changed.extend(to_resort);
            }

            if !changed.is_empty() {
                if let Some((proxy_rows, proxy_columns)) =
                    self.with_mapping(id, |m| (m.proxy_rows.clone(), m.proxy_columns.clone()))
                {
                    let rows = changed
                        .iter()
                        .filter_map(|&row| proxy_rows.get(row as usize).copied())
                        .filter(|&row| row >= 0);
                    let (low, high) =
                        rows.fold((i32::MAX, i32::MIN), |(lo, hi), r| (lo.min(r), hi.max(r)));
                    let column_of = |c: i32| proxy_columns.get(c as usize).copied().unwrap_or(-1);
                    if high >= 0 {
                        let mut left = top_left.column;
                        while left < bottom_right.column && column_of(left) == -1 {
                            left += 1;
                        }
                        let mut right = bottom_right.column;
                        while right > top_left.column && column_of(right) == -1 {
                            right -= 1;
                        }
                        if column_of(left) != -1 && column_of(right) != -1 {
                            let proxy_top_left =
                                ModelIndex::new(low, column_of(left), id, self.base.id());
                            let proxy_bottom_right =
                                ModelIndex::new(high, column_of(right), id, self.base.id());
                            self.signals().data_changed.emit(&(
                                proxy_top_left,
                                proxy_bottom_right,
                                roles.to_vec(),
                            ));
                        }
                    }
                }
            }

            if !to_insert.is_empty() {
                self.sort_source_rows(src, &mut to_insert, &source_parent);
                self.insert_source_items(src, id, &to_insert, &source_parent, Dir::Rows, true);
            }
        }
    }

    fn source_header_data_changed(
        &self,
        src: &dyn AbstractItemModel,
        orientation: Orientation,
        start: i32,
        end: i32,
    ) {
        if end < start {
            return;
        }
        let id = self.create_mapping(src, &ModelIndex::INVALID);
        let dir = match orientation {
            Orientation::Vertical => Dir::Rows,
            Orientation::Horizontal => Dir::Columns,
        };
        let mut positions: Vec<i32> = self
            .with_mapping(id, |m| {
                let source_to_proxy = m.source_to_proxy(dir);
                (start..=end)
                    .filter_map(|s| source_to_proxy.get(s as usize).copied())
                    .filter(|&p| p != -1)
                    .collect()
            })
            .unwrap_or_default();
        positions.sort_unstable();
        let mut i = 0;
        while i < positions.len() {
            let first = positions[i];
            let mut last = first;
            i += 1;
            while i < positions.len() && positions[i] == last + 1 {
                last += 1;
                i += 1;
            }
            self.signals()
                .header_data_changed
                .emit(&(orientation, first, last));
        }
    }

    fn source_rows_about_to_be_inserted(
        &self,
        src: &dyn AbstractItemModel,
        source_parent: &ModelIndex,
    ) {
        let top_level = !source_parent.is_valid();
        let recursive = self.recursive_filtering.get();
        let parent_accepted = recursive
            && !top_level
            && self.filter_accepts_row_internal(src, source_parent.row, &src.parent(source_parent));
        if !recursive || top_level || parent_accepted {
            if self.can_create_mapping(src, source_parent) {
                self.create_mapping(src, source_parent);
            }
            if recursive {
                self.complete_insert.set(true);
            }
        } else {
            // The parent may be hidden: find the topmost hidden ancestor that
            // could become visible through the new rows.
            let mut top = *source_parent;
            let mut parent = src.parent(source_parent);
            let mut grand_parent = src.parent(&parent);
            while parent.is_valid()
                && !self.filter_accepts_row_internal(src, parent.row, &grand_parent)
            {
                top = parent;
                parent = grand_parent;
                grand_parent = src.parent(&parent);
            }
            *self.last_top_source.borrow_mut() = PersistentModelIndex::new(src, &top);
        }
    }

    fn source_rows_inserted(
        &self,
        src: &dyn AbstractItemModel,
        source_parent: &ModelIndex,
        start: i32,
        end: i32,
    ) {
        let recursive = self.recursive_filtering.get();
        if !recursive || self.complete_insert.get() {
            self.complete_insert.set(false);
            self.source_items_inserted(src, source_parent, start, end, Dir::Rows);
            if self.update_source_sort_column(src) && self.dynamic_sort_filter.get() {
                self.do_sort(src);
            }
            return;
        }
        let accepted =
            (start..=end).any(|row| self.filter_accepts_row_internal(src, row, source_parent));
        if !accepted {
            return;
        }
        let top = std::mem::take(&mut *self.last_top_source.borrow_mut()).index();
        if top.is_valid() {
            self.source_data_changed(src, &top, &top, &[]);
        }
    }

    fn source_rows_removed(
        &self,
        src: &dyn AbstractItemModel,
        source_parent: &ModelIndex,
        start: i32,
        end: i32,
    ) {
        self.items_being_removed.set(None);
        self.source_items_removed(src, source_parent, start, end, Dir::Rows);
        if self.recursive_filtering.get() {
            // Removing visible rows may leave ancestors without accepted descendants.
            let mut to_hide = ModelIndex::INVALID;
            let mut ancestor = *source_parent;
            while ancestor.is_valid() {
                let grand_parent = src.parent(&ancestor);
                if self.filter_accepts_row_internal(src, ancestor.row, &grand_parent) {
                    break;
                }
                to_hide = ancestor;
                ancestor = grand_parent;
            }
            if to_hide.is_valid() {
                self.source_data_changed(src, &to_hide, &to_hide, &[]);
            }
        }
    }

    fn source_columns_inserted(
        &self,
        src: &dyn AbstractItemModel,
        source_parent: &ModelIndex,
        start: i32,
        end: i32,
    ) {
        self.source_items_inserted(src, source_parent, start, end, Dir::Columns);
        if source_parent.is_valid() {
            return;
        }
        let sort_column = self.source_sort_column.get();
        if sort_column == -1 {
            if self.update_source_sort_column(src) && self.dynamic_sort_filter.get() {
                self.do_sort(src);
            }
        } else {
            let sort_column = if start <= sort_column {
                sort_column + end - start + 1
            } else {
                sort_column
            };
            self.source_sort_column.set(sort_column);
            let proxy = self.source_to_proxy(src, &src.index(0, sort_column, source_parent));
            self.proxy_sort_column.set(proxy.column);
        }
    }

    fn source_columns_removed(
        &self,
        src: &dyn AbstractItemModel,
        source_parent: &ModelIndex,
        start: i32,
        end: i32,
    ) {
        self.source_items_removed(src, source_parent, start, end, Dir::Columns);
        if source_parent.is_valid() {
            return;
        }
        let mut sort_column = self.source_sort_column.get();
        if start <= sort_column {
            if end < sort_column {
                sort_column -= end - start + 1;
            } else {
                sort_column = -1;
            }
        }
        self.source_sort_column.set(sort_column);
        if sort_column >= 0 {
            let proxy = self.source_to_proxy(src, &src.index(0, sort_column, source_parent));
            self.proxy_sort_column.set(proxy.column);
        } else {
            self.proxy_sort_column.set(-1);
        }
    }

    // ----- filter changes ----------------------------------------------------------

    fn filter_about_to_be_changed(&self, src: &dyn AbstractItemModel) {
        let root = ModelIndex::INVALID;
        if !self.filter.borrow().is_empty() && self.mapping_id(&root).is_none() {
            self.create_mapping(src, &root);
        }
    }

    fn handle_filter_changed(
        &self,
        src: &dyn AbstractItemModel,
        id: u64,
        source_parent: &ModelIndex,
        dir: Dir,
    ) -> HashSet<i32> {
        let Some((proxy_to_source, source_count)) = self.with_mapping(id, |m| {
            (m.proxy_to_source(dir).clone(), m.source_to_proxy(dir).len())
        }) else {
            return HashSet::new();
        };
        let to_remove: Vec<i32> = proxy_to_source
            .iter()
            .copied()
            .filter(|&item| !self.accepts(src, dir, item, source_parent))
            .collect();
        let hidden: Vec<i32> = self
            .with_mapping(id, |m| {
                let source_to_proxy = m.source_to_proxy(dir);
                (0..source_count as i32)
                    .filter(|&s| source_to_proxy[s as usize] == -1)
                    .collect()
            })
            .unwrap_or_default();
        let mut to_insert: Vec<i32> = hidden
            .into_iter()
            .filter(|&item| self.accepts(src, dir, item, source_parent))
            .collect();
        if !to_remove.is_empty() || !to_insert.is_empty() {
            self.remove_source_items(src, id, &to_remove, source_parent, dir, true);
            if dir == Dir::Rows {
                self.sort_source_rows(src, &mut to_insert, source_parent);
            }
            self.insert_source_items(src, id, &to_insert, source_parent, dir, true);
        }
        to_remove.into_iter().collect()
    }

    fn filter_changed(
        &self,
        src: &dyn AbstractItemModel,
        rows: bool,
        columns: bool,
        source_parent: &ModelIndex,
    ) {
        let Some(id) = self.mapping_id(source_parent) else {
            return;
        };
        let rows_removed = if rows {
            self.handle_filter_changed(src, id, source_parent, Dir::Rows)
        } else {
            HashSet::new()
        };
        let columns_removed = if columns {
            self.handle_filter_changed(src, id, source_parent, Dir::Columns)
        } else {
            HashSet::new()
        };
        let children = self
            .with_mapping(id, |m| m.mapped_children.clone())
            .unwrap_or_default();
        let mut stale = Vec::new();
        for (position, child) in children.iter().enumerate() {
            if rows_removed.contains(&child.row) || columns_removed.contains(&child.column) {
                stale.push(position);
                self.remove_from_mapping(child);
            } else {
                self.filter_changed(src, rows, columns, child);
            }
        }
        self.with_mapping(id, |m| {
            for &position in stale.iter().rev() {
                if position < m.mapped_children.len() {
                    m.mapped_children.remove(position);
                }
            }
        });
    }

    /// Applies a filter setting change and re-filters existing mappings.
    fn change_filter(&self, rows: bool, columns: bool, apply: impl FnOnce()) {
        match self.source.get() {
            Some(source) => {
                let source = source.try_borrow().expect(SOURCE_BORROWED);
                let src: &dyn AbstractItemModel = &*source;
                self.filter_about_to_be_changed(src);
                apply();
                self.filter_changed(src, rows, columns, &ModelIndex::INVALID);
            }
            None => apply(),
        }
    }

    fn set_source(&self, source: Option<SharedModel>) {
        self.begin_reset_model();
        let listener: Weak<dyn ModelListener> = self.self_weak.clone();
        self.source.replace(source, listener);
        self.maps.borrow_mut().clear();
        self.saved_persistent.borrow_mut().clear();
        self.items_being_removed.set(None);
        self.complete_insert.set(false);
        *self.last_top_source.borrow_mut() = PersistentModelIndex::default();
        self.end_reset_model();
        self.with_source((), |src| {
            if self.update_source_sort_column(src) && self.dynamic_sort_filter.get() {
                self.do_sort(src);
            }
        });
    }

    fn resort_if_dynamic(&self) {
        if self.dynamic_sort_filter.get() {
            self.with_source((), |src| self.do_sort(src));
        }
    }
}

impl AbstractItemModel for SfpmInner {
    fn base(&self) -> &ModelBase {
        &self.base
    }

    fn index(&self, row: i32, column: i32, parent: &ModelIndex) -> ModelIndex {
        if row < 0 || column < 0 {
            return ModelIndex::INVALID;
        }
        self.with_source(ModelIndex::INVALID, |src| {
            let source_parent = self.proxy_to_source(src, parent);
            if parent.is_valid() && !source_parent.is_valid() {
                return ModelIndex::INVALID;
            }
            let id = self.create_mapping(src, &source_parent);
            let in_range = self
                .with_mapping(id, |m| {
                    (row as usize) < m.source_rows.len()
                        && (column as usize) < m.source_columns.len()
                })
                .unwrap_or(false);
            if in_range {
                ModelIndex::new(row, column, id, self.base.id())
            } else {
                ModelIndex::INVALID
            }
        })
    }

    fn parent(&self, child: &ModelIndex) -> ModelIndex {
        if !child.is_valid() || child.model_id != self.base.id() {
            return ModelIndex::INVALID;
        }
        let Some(source_parent) = self
            .maps
            .borrow()
            .maps
            .get(&child.internal_id)
            .map(|m| m.source_parent)
        else {
            return ModelIndex::INVALID;
        };
        if !source_parent.is_valid() {
            return ModelIndex::INVALID;
        }
        self.with_source(ModelIndex::INVALID, |src| {
            self.source_to_proxy(src, &source_parent)
        })
    }

    fn row_count(&self, parent: &ModelIndex) -> i32 {
        self.with_source(0, |src| {
            let source_parent = self.proxy_to_source(src, parent);
            if parent.is_valid() && !source_parent.is_valid() {
                return 0;
            }
            let id = self.create_mapping(src, &source_parent);
            self.with_mapping(id, |m| m.source_rows.len() as i32)
                .unwrap_or(0)
        })
    }

    fn column_count(&self, parent: &ModelIndex) -> i32 {
        self.with_source(0, |src| {
            let source_parent = self.proxy_to_source(src, parent);
            if parent.is_valid() && !source_parent.is_valid() {
                return 0;
            }
            let id = self.create_mapping(src, &source_parent);
            self.with_mapping(id, |m| m.source_columns.len() as i32)
                .unwrap_or(0)
        })
    }

    fn has_children(&self, parent: &ModelIndex) -> bool {
        self.with_source(false, |src| {
            let source_parent = self.proxy_to_source(src, parent);
            if parent.is_valid() && !source_parent.is_valid() {
                return false;
            }
            if !src.has_children(&source_parent) {
                return false;
            }
            let id = self.create_mapping(src, &source_parent);
            self.with_mapping(id, |m| {
                !m.source_rows.is_empty() && !m.source_columns.is_empty()
            })
            .unwrap_or(false)
        })
    }

    fn sibling(&self, row: i32, column: i32, index: &ModelIndex) -> ModelIndex {
        if !index.is_valid() || index.model_id != self.base.id() || row < 0 || column < 0 {
            return ModelIndex::INVALID;
        }
        let in_range = self
            .with_mapping(index.internal_id, |m| {
                (row as usize) < m.source_rows.len() && (column as usize) < m.source_columns.len()
            })
            .unwrap_or(false);
        if in_range {
            ModelIndex::new(row, column, index.internal_id, self.base.id())
        } else {
            ModelIndex::INVALID
        }
    }

    fn data(&self, index: &ModelIndex, role: ItemDataRole) -> Variant {
        self.with_source(Variant::Invalid, |src| {
            let source = self.proxy_to_source(src, index);
            if source.is_valid() {
                src.data(&source, role)
            } else {
                Variant::Invalid
            }
        })
    }

    fn set_data(&self, index: &ModelIndex, value: Variant, role: ItemDataRole) -> bool {
        self.with_source(false, |src| {
            let source = self.proxy_to_source(src, index);
            source.is_valid() && src.set_data(&source, value, role)
        })
    }

    fn header_data(&self, section: i32, orientation: Orientation, role: ItemDataRole) -> Variant {
        self.with_source(Variant::Invalid, |src| {
            let id = self.create_mapping(src, &ModelIndex::INVALID);
            let source_section = self.with_mapping(id, |m| {
                let list = match orientation {
                    Orientation::Vertical => &m.source_rows,
                    Orientation::Horizontal => &m.source_columns,
                };
                usize::try_from(section)
                    .ok()
                    .and_then(|s| list.get(s).copied())
            });
            match source_section.flatten() {
                Some(source_section) => src.header_data(source_section, orientation, role),
                None => Variant::Invalid,
            }
        })
    }

    fn set_header_data(
        &self,
        section: i32,
        orientation: Orientation,
        value: Variant,
        role: ItemDataRole,
    ) -> bool {
        self.with_source(false, |src| {
            let id = self.create_mapping(src, &ModelIndex::INVALID);
            let source_section = self.with_mapping(id, |m| {
                let list = match orientation {
                    Orientation::Vertical => &m.source_rows,
                    Orientation::Horizontal => &m.source_columns,
                };
                usize::try_from(section)
                    .ok()
                    .and_then(|s| list.get(s).copied())
            });
            match source_section.flatten() {
                Some(source_section) => {
                    src.set_header_data(source_section, orientation, value, role)
                }
                None => false,
            }
        })
    }

    fn flags(&self, index: &ModelIndex) -> ItemFlags {
        self.with_source(ItemFlags::NONE, |src| {
            src.flags(&self.proxy_to_source(src, index))
        })
    }

    fn buddy(&self, index: &ModelIndex) -> ModelIndex {
        self.with_source(ModelIndex::INVALID, |src| {
            let source = self.proxy_to_source(src, index);
            let buddy = src.buddy(&source);
            if buddy == source {
                *index
            } else {
                self.source_to_proxy(src, &buddy)
            }
        })
    }

    fn insert_rows(&self, row: i32, count: i32, parent: &ModelIndex) -> bool {
        if row < 0 || count <= 0 {
            return false;
        }
        let Some(source) = self.source.get() else {
            return false;
        };
        let target = {
            let src = source.try_borrow().expect(SOURCE_BORROWED);
            let source_parent = self.proxy_to_source(&*src, parent);
            if parent.is_valid() && !source_parent.is_valid() {
                return false;
            }
            let id = self.create_mapping(&*src, &source_parent);
            self.with_mapping(id, |m| {
                let row = row as usize;
                if row > m.source_rows.len() {
                    None
                } else if row == m.source_rows.len() {
                    Some(m.proxy_rows.len() as i32)
                } else {
                    Some(m.source_rows[row])
                }
            })
            .flatten()
            .map(|source_row| (source_row, source_parent))
        };
        match target {
            Some((source_row, source_parent)) => {
                source
                    .borrow()
                    .insert_rows(source_row, count, &source_parent)
            }
            None => false,
        }
    }

    fn remove_rows(&self, row: i32, count: i32, parent: &ModelIndex) -> bool {
        self.remove_items(row, count, parent, Dir::Rows)
    }

    fn insert_columns(&self, column: i32, count: i32, parent: &ModelIndex) -> bool {
        if column < 0 || count <= 0 {
            return false;
        }
        let Some(source) = self.source.get() else {
            return false;
        };
        let target = {
            let src = source.try_borrow().expect(SOURCE_BORROWED);
            let source_parent = self.proxy_to_source(&*src, parent);
            if parent.is_valid() && !source_parent.is_valid() {
                return false;
            }
            let id = self.create_mapping(&*src, &source_parent);
            self.with_mapping(id, |m| {
                let column = column as usize;
                if column > m.source_columns.len() {
                    None
                } else if column == m.source_columns.len() {
                    Some(m.proxy_columns.len() as i32)
                } else {
                    Some(m.source_columns[column])
                }
            })
            .flatten()
            .map(|source_column| (source_column, source_parent))
        };
        match target {
            Some((source_column, source_parent)) => {
                source
                    .borrow()
                    .insert_columns(source_column, count, &source_parent)
            }
            None => false,
        }
    }

    fn remove_columns(&self, column: i32, count: i32, parent: &ModelIndex) -> bool {
        self.remove_items(column, count, parent, Dir::Columns)
    }

    fn sort(&self, column: i32, order: SortOrder) {
        if self.dynamic_sort_filter.get()
            && self.proxy_sort_column.get() == column
            && self.sort_order.get() == order
        {
            return;
        }
        self.sort_order.set(order);
        self.proxy_sort_column.set(column);
        self.with_source((), |src| {
            self.update_source_sort_column(src);
            self.do_sort(src);
        });
    }
}

impl SfpmInner {
    /// Removes proxy rows/columns by removing the corresponding source ranges.
    fn remove_items(&self, first: i32, count: i32, parent: &ModelIndex, dir: Dir) -> bool {
        if first < 0 || count <= 0 {
            return false;
        }
        let Some(source) = self.source.get() else {
            return false;
        };
        let plan = {
            let src = source.try_borrow().expect(SOURCE_BORROWED);
            let source_parent = self.proxy_to_source(&*src, parent);
            if parent.is_valid() && !source_parent.is_valid() {
                return false;
            }
            let id = self.create_mapping(&*src, &source_parent);
            let items = self
                .with_mapping(id, |m| {
                    let list = m.proxy_to_source(dir);
                    let end = (first + count) as usize;
                    (end <= list.len()).then(|| list[first as usize..end].to_vec())
                })
                .flatten();
            items.map(|items| (items, source_parent))
        };
        let Some((mut items, source_parent)) = plan else {
            return false;
        };
        items.sort_unstable();
        // Remove contiguous source intervals from the back so earlier positions stay valid.
        let mut ok = true;
        let mut position = items.len();
        while position > 0 {
            position -= 1;
            let last = items[position];
            let mut first_item = last;
            while position > 0 && items[position - 1] == first_item - 1 {
                position -= 1;
                first_item -= 1;
            }
            let removed = {
                let src = source.borrow();
                match dir {
                    Dir::Rows => src.remove_rows(first_item, last - first_item + 1, &source_parent),
                    Dir::Columns => {
                        src.remove_columns(first_item, last - first_item + 1, &source_parent)
                    }
                }
            };
            ok &= removed;
        }
        ok
    }
}

impl ModelListener for SfpmInner {
    fn model_event(&self, event: &ModelEvent) {
        let Some(source) = self.source.get() else {
            return;
        };
        let source = source.try_borrow().expect(SOURCE_BORROWED);
        let src: &dyn AbstractItemModel = &*source;
        match event {
            ModelEvent::DataChanged {
                top_left,
                bottom_right,
                roles,
            } => self.source_data_changed(src, top_left, bottom_right, roles),
            ModelEvent::HeaderDataChanged {
                orientation,
                first,
                last,
            } => self.source_header_data_changed(src, *orientation, *first, *last),
            ModelEvent::RowsAboutToBeInserted { parent, .. } => {
                self.source_rows_about_to_be_inserted(src, parent)
            }
            ModelEvent::RowsInserted {
                parent,
                first,
                last,
            } => self.source_rows_inserted(src, parent, *first, *last),
            ModelEvent::RowsAboutToBeRemoved {
                parent,
                first,
                last,
            } => {
                self.items_being_removed.set(Some((*parent, *first, *last)));
                self.source_items_about_to_be_removed(src, parent, *first, *last, Dir::Rows);
            }
            ModelEvent::RowsRemoved {
                parent,
                first,
                last,
            } => self.source_rows_removed(src, parent, *first, *last),
            ModelEvent::ColumnsAboutToBeInserted { parent, .. } => {
                if self.can_create_mapping(src, parent) {
                    self.create_mapping(src, parent);
                }
            }
            ModelEvent::ColumnsInserted {
                parent,
                first,
                last,
            } => self.source_columns_inserted(src, parent, *first, *last),
            ModelEvent::ColumnsAboutToBeRemoved {
                parent,
                first,
                last,
            } => self.source_items_about_to_be_removed(src, parent, *first, *last, Dir::Columns),
            ModelEvent::ColumnsRemoved {
                parent,
                first,
                last,
            } => self.source_columns_removed(src, parent, *first, *last),
            ModelEvent::LayoutAboutToBeChanged => {
                self.saved_persistent.borrow_mut().clear();
                self.begin_layout_change();
                let saved = self.store_persistent_indexes(src);
                *self.saved_persistent.borrow_mut() = saved;
            }
            ModelEvent::LayoutChanged => {
                self.maps.borrow_mut().clear();
                let saved = std::mem::take(&mut *self.saved_persistent.borrow_mut());
                self.update_persistent_indexes(src, saved);
                if self.dynamic_sort_filter.get() {
                    self.source_sort_column
                        .set(self.find_source_sort_column(src));
                }
                self.end_layout_change();
            }
            ModelEvent::ModelAboutToBeReset => self.begin_reset_model(),
            ModelEvent::ModelReset => {
                self.base.invalidate_persistent_indexes();
                self.maps.borrow_mut().clear();
                if self.dynamic_sort_filter.get() {
                    self.source_sort_column
                        .set(self.find_source_sort_column(src));
                }
                self.end_reset_model();
                if self.update_source_sort_column(src) && self.dynamic_sort_filter.get() {
                    self.do_sort(src);
                }
            }
        }
    }
}

/// Sorting and filtering proxy (`QSortFilterProxyModel`).
///
/// Rows are filtered by matching the filter pattern against the
/// `filter_role` data of `filter_key_column` (or any column when the key
/// column is -1), optionally keeping ancestors of matches (recursive
/// filtering) and descendants of matches (`auto_accept_child_rows`). Sorting
/// compares `sort_role` data. With dynamic sort/filter enabled (the default),
/// source changes are applied incrementally with the proper proxy signals.
pub struct SortFilterProxyModel {
    inner: Rc<SfpmInner>,
}

delegate_item_model!(SortFilterProxyModel, inner);

impl Default for SortFilterProxyModel {
    fn default() -> Self {
        Self::new()
    }
}

impl SortFilterProxyModel {
    pub fn new() -> Self {
        Self {
            inner: SfpmInner::new(),
        }
    }

    pub fn with_source(source: SharedModel) -> Self {
        let proxy = Self::new();
        proxy.inner.set_source(Some(source));
        proxy
    }

    // ----- filtering ------------------------------------------------------------

    pub fn filter_key_column(&self) -> i32 {
        self.inner.filter_key_column.get()
    }

    /// Column matched by the filter; -1 matches any column.
    pub fn set_filter_key_column(&self, column: i32) {
        if self.inner.filter_key_column.get() == column {
            return;
        }
        self.inner
            .change_filter(true, false, || self.inner.filter_key_column.set(column));
    }

    pub fn filter_role(&self) -> ItemDataRole {
        self.inner.filter_role.get()
    }

    pub fn set_filter_role(&self, role: ItemDataRole) {
        if self.inner.filter_role.get() == role {
            return;
        }
        self.inner
            .change_filter(true, false, || self.inner.filter_role.set(role));
    }

    pub fn filter_case_sensitivity(&self) -> CaseSensitivity {
        self.inner.filter.borrow().case
    }

    pub fn set_filter_case_sensitivity(&self, case: CaseSensitivity) {
        let (syntax, pattern) = {
            let filter = self.inner.filter.borrow();
            if filter.case == case {
                return;
            }
            (filter.syntax, filter.pattern.clone())
        };
        self.set_filter(syntax, &pattern, case);
    }

    pub fn filter_pattern(&self) -> String {
        self.inner.filter.borrow().pattern.clone()
    }

    pub fn filter_syntax(&self) -> FilterSyntax {
        self.inner.filter.borrow().syntax
    }

    /// `true` if the regular expression / wildcard pattern compiled.
    pub fn is_filter_valid(&self) -> bool {
        let filter = self.inner.filter.borrow();
        filter.syntax == FilterSyntax::FixedString || filter.regex.is_some()
    }

    fn set_filter(&self, syntax: FilterSyntax, pattern: &str, case: CaseSensitivity) {
        let filter = Rc::new(Filter::new(syntax, pattern, case));
        self.inner
            .change_filter(true, false, || *self.inner.filter.borrow_mut() = filter);
    }

    /// Accepts rows whose key text contains `pattern`.
    pub fn set_filter_fixed_string(&self, pattern: &str) {
        self.set_filter(
            FilterSyntax::FixedString,
            pattern,
            self.filter_case_sensitivity(),
        );
    }

    /// Accepts rows whose key text matches the shell wildcard `pattern` anywhere.
    pub fn set_filter_wildcard(&self, pattern: &str) {
        self.set_filter(
            FilterSyntax::Wildcard,
            pattern,
            self.filter_case_sensitivity(),
        );
    }

    /// Accepts rows whose key text matches the regular expression `pattern`
    /// (case sensitivity from [`Self::filter_case_sensitivity`]).
    pub fn set_filter_regular_expression(&self, pattern: &str) {
        self.set_filter(
            FilterSyntax::RegularExpression,
            pattern,
            self.filter_case_sensitivity(),
        );
    }

    /// Uses `expression` (including its case option) as the filter.
    pub fn set_filter_regular_expression_object(&self, expression: &RegularExpression) {
        let case = if expression.is_case_insensitive() {
            CaseSensitivity::Insensitive
        } else {
            CaseSensitivity::Sensitive
        };
        self.set_filter(FilterSyntax::RegularExpression, expression.pattern(), case);
    }

    pub fn is_recursive_filtering_enabled(&self) -> bool {
        self.inner.recursive_filtering.get()
    }

    /// Keeps parents whose descendants match the filter.
    pub fn set_recursive_filtering_enabled(&self, enabled: bool) {
        if self.inner.recursive_filtering.get() == enabled {
            return;
        }
        self.inner
            .change_filter(true, false, || self.inner.recursive_filtering.set(enabled));
    }

    pub fn auto_accept_child_rows(&self) -> bool {
        self.inner.auto_accept_child_rows.get()
    }

    /// Keeps all descendants of accepted rows.
    pub fn set_auto_accept_child_rows(&self, accept: bool) {
        if self.inner.auto_accept_child_rows.get() == accept {
            return;
        }
        self.inner.change_filter(true, false, || {
            self.inner.auto_accept_child_rows.set(accept)
        });
    }

    /// Replaces row acceptance with `filter(source_model, source_row, source_parent)`.
    pub fn set_filter_accepts_row<F>(&self, filter: F)
    where
        F: Fn(&dyn AbstractItemModel, i32, &ModelIndex) -> bool + 'static,
    {
        let filter: Rc<FilterAcceptsFn> = Rc::new(filter);
        self.inner.change_filter(true, false, || {
            *self.inner.filter_row_fn.borrow_mut() = Some(filter)
        });
    }

    /// Restores pattern-based row filtering.
    pub fn clear_filter_accepts_row(&self) {
        self.inner.change_filter(true, false, || {
            *self.inner.filter_row_fn.borrow_mut() = None
        });
    }

    /// Replaces column acceptance with `filter(source_model, source_column, source_parent)`.
    pub fn set_filter_accepts_column<F>(&self, filter: F)
    where
        F: Fn(&dyn AbstractItemModel, i32, &ModelIndex) -> bool + 'static,
    {
        let filter: Rc<FilterAcceptsFn> = Rc::new(filter);
        self.inner.change_filter(false, true, || {
            *self.inner.filter_column_fn.borrow_mut() = Some(filter)
        });
    }

    /// Accepts every column again.
    pub fn clear_filter_accepts_column(&self) {
        self.inner.change_filter(false, true, || {
            *self.inner.filter_column_fn.borrow_mut() = None
        });
    }

    /// Re-applies the filter to all rows and columns (`invalidateFilter`).
    pub fn invalidate_filter(&self) {
        self.inner.change_filter(true, true, || {});
    }

    pub fn invalidate_rows_filter(&self) {
        self.inner.change_filter(true, false, || {});
    }

    pub fn invalidate_columns_filter(&self) {
        self.inner.change_filter(false, true, || {});
    }

    /// Rebuilds all mappings (`invalidate`).
    pub fn invalidate(&self) {
        self.inner.begin_layout_change();
        self.inner
            .with_source((), |src| self.inner.clear_mapping(src));
        self.inner.end_layout_change();
    }

    // ----- sorting ----------------------------------------------------------------

    pub fn sort_column(&self) -> i32 {
        self.inner.proxy_sort_column.get()
    }

    pub fn sort_order(&self) -> SortOrder {
        self.inner.sort_order.get()
    }

    pub fn sort_role(&self) -> ItemDataRole {
        self.inner.sort_role.get()
    }

    pub fn set_sort_role(&self, role: ItemDataRole) {
        if self.inner.sort_role.get() == role {
            return;
        }
        self.inner.sort_role.set(role);
        self.inner.resort_if_dynamic();
    }

    pub fn sort_case_sensitivity(&self) -> CaseSensitivity {
        self.inner.sort_case.get()
    }

    pub fn set_sort_case_sensitivity(&self, case: CaseSensitivity) {
        if self.inner.sort_case.get() == case {
            return;
        }
        self.inner.sort_case.set(case);
        self.inner.resort_if_dynamic();
    }

    /// Replaces the ordering with `less_than(source_model, left, right)`.
    pub fn set_less_than<F>(&self, less_than: F)
    where
        F: Fn(&dyn AbstractItemModel, &ModelIndex, &ModelIndex) -> bool + 'static,
    {
        let less_than: Rc<LessThanFn> = Rc::new(less_than);
        *self.inner.less_than_fn.borrow_mut() = Some(less_than);
        self.inner.resort_if_dynamic();
    }

    /// Restores role-based ordering.
    pub fn clear_less_than(&self) {
        *self.inner.less_than_fn.borrow_mut() = None;
        self.inner.resort_if_dynamic();
    }

    pub fn dynamic_sort_filter(&self) -> bool {
        self.inner.dynamic_sort_filter.get()
    }

    /// When enabled, source changes re-filter and re-sort automatically.
    pub fn set_dynamic_sort_filter(&self, enabled: bool) {
        self.inner.dynamic_sort_filter.set(enabled);
        if enabled {
            self.inner.with_source((), |src| self.inner.do_sort(src));
        }
    }
}

impl AbstractProxyModel for SortFilterProxyModel {
    fn source_model(&self) -> Option<SharedModel> {
        self.inner.source.get()
    }

    fn set_source_model(&self, source: Option<SharedModel>) {
        self.inner.set_source(source);
    }

    fn map_to_source(&self, proxy_index: &ModelIndex) -> ModelIndex {
        self.inner.with_source(ModelIndex::INVALID, |src| {
            self.inner.proxy_to_source(src, proxy_index)
        })
    }

    fn map_from_source(&self, source_index: &ModelIndex) -> ModelIndex {
        self.inner.with_source(ModelIndex::INVALID, |src| {
            self.inner.source_to_proxy(src, source_index)
        })
    }
}

/// Canonical Qt aliases.
pub type QAbstractProxyModel = dyn AbstractProxyModel;
pub type QIdentityProxyModel = IdentityProxyModel;
pub type QSortFilterProxyModel = SortFilterProxyModel;
