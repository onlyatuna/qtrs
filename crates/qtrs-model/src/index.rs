//! Model indexes (`QModelIndex`, `QPersistentModelIndex`).

use std::cell::Cell;
use std::fmt;
use std::rc::Rc;

use qtrs_core::Variant;

use crate::model::AbstractItemModel;
use crate::role::{ItemDataRole, ItemFlags};

/// Lightweight locator of an item inside an [`AbstractItemModel`] (`QModelIndex`).
///
/// An index is only meaningful for the model that created it (identified by
/// `model_id`) and only until the model structure changes; use
/// [`PersistentModelIndex`] to track an item across structural changes.
///
/// Models must keep `internal_id` stable across insertions and removals of
/// rows in ancestor levels (e.g. by storing the id of the parent node).
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ModelIndex {
    pub row: i32,
    pub column: i32,
    pub internal_id: u64,
    pub model_id: u64,
}

impl ModelIndex {
    /// The canonical invalid index (`QModelIndex()`).
    pub const INVALID: ModelIndex = ModelIndex {
        row: -1,
        column: -1,
        internal_id: 0,
        model_id: 0,
    };

    /// Creates an index; negative coordinates or a zero model id yield [`ModelIndex::INVALID`].
    #[inline]
    pub const fn new(row: i32, column: i32, internal_id: u64, model_id: u64) -> Self {
        if row < 0 || column < 0 || model_id == 0 {
            Self::INVALID
        } else {
            Self {
                row,
                column,
                internal_id,
                model_id,
            }
        }
    }

    /// Returns the invalid index.
    #[inline]
    pub const fn invalid() -> Self {
        Self::INVALID
    }

    /// Returns `true` if the index refers to an item of some model.
    #[inline]
    pub const fn is_valid(&self) -> bool {
        self.row >= 0 && self.column >= 0 && self.model_id != 0
    }

    #[inline]
    pub const fn row(&self) -> i32 {
        self.row
    }

    #[inline]
    pub const fn column(&self) -> i32 {
        self.column
    }

    #[inline]
    pub const fn internal_id(&self) -> u64 {
        self.internal_id
    }

    #[inline]
    pub const fn model_id(&self) -> u64 {
        self.model_id
    }

    /// Parent of this index as reported by `model`.
    pub fn parent<M: AbstractItemModel + ?Sized>(&self, model: &M) -> ModelIndex {
        if self.is_valid() {
            model.parent(self)
        } else {
            ModelIndex::INVALID
        }
    }

    /// Sibling at `(row, column)` under the same parent.
    pub fn sibling<M: AbstractItemModel + ?Sized>(
        &self,
        row: i32,
        column: i32,
        model: &M,
    ) -> ModelIndex {
        if self.is_valid() {
            model.sibling(row, column, self)
        } else {
            ModelIndex::INVALID
        }
    }

    /// Sibling in the same row at `column`.
    pub fn sibling_at_column<M: AbstractItemModel + ?Sized>(
        &self,
        column: i32,
        model: &M,
    ) -> ModelIndex {
        self.sibling(self.row, column, model)
    }

    /// Sibling in the same column at `row`.
    pub fn sibling_at_row<M: AbstractItemModel + ?Sized>(&self, row: i32, model: &M) -> ModelIndex {
        self.sibling(row, self.column, model)
    }

    /// Data stored under `role` for this index.
    pub fn data<M: AbstractItemModel + ?Sized>(&self, model: &M, role: ItemDataRole) -> Variant {
        if self.is_valid() {
            model.data(self, role)
        } else {
            Variant::Invalid
        }
    }

    /// Flags of this index.
    pub fn flags<M: AbstractItemModel + ?Sized>(&self, model: &M) -> ItemFlags {
        if self.is_valid() {
            model.flags(self)
        } else {
            ItemFlags::NONE
        }
    }
}

impl Default for ModelIndex {
    fn default() -> Self {
        Self::INVALID
    }
}

impl fmt::Debug for ModelIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_valid() {
            write!(
                f,
                "ModelIndex({}, {}, id={}, model={})",
                self.row, self.column, self.internal_id, self.model_id
            )
        } else {
            f.write_str("ModelIndex(invalid)")
        }
    }
}

/// Shared cell that a model keeps up to date for a [`PersistentModelIndex`].
pub(crate) struct PersistentData {
    pub(crate) index: Cell<ModelIndex>,
}

/// Index that follows its item across insertions, removals and layout changes
/// (`QPersistentModelIndex`).
///
/// Clones share the same tracked position. The index becomes invalid when the
/// item is removed, the model is reset or the model is dropped.
#[derive(Clone, Default)]
pub struct PersistentModelIndex {
    data: Option<Rc<PersistentData>>,
}

impl PersistentModelIndex {
    /// Starts tracking `index` of `model`. Invalid or foreign indexes produce an
    /// invalid persistent index.
    pub fn new<M: AbstractItemModel + ?Sized>(model: &M, index: &ModelIndex) -> Self {
        if !index.is_valid() || index.model_id != model.model_id() {
            return Self::default();
        }
        let data = Rc::new(PersistentData {
            index: Cell::new(*index),
        });
        model.base().register_persistent(&data);
        Self { data: Some(data) }
    }

    /// Current position of the tracked item.
    #[inline]
    pub fn index(&self) -> ModelIndex {
        self.data
            .as_ref()
            .map_or(ModelIndex::INVALID, |d| d.index.get())
    }

    #[inline]
    pub fn is_valid(&self) -> bool {
        self.index().is_valid()
    }

    #[inline]
    pub fn row(&self) -> i32 {
        self.index().row
    }

    #[inline]
    pub fn column(&self) -> i32 {
        self.index().column
    }

    #[inline]
    pub fn internal_id(&self) -> u64 {
        self.index().internal_id
    }

    #[inline]
    pub fn model_id(&self) -> u64 {
        self.index().model_id
    }

    /// Parent of the tracked item.
    pub fn parent<M: AbstractItemModel + ?Sized>(&self, model: &M) -> ModelIndex {
        self.index().parent(model)
    }

    /// Data of the tracked item.
    pub fn data<M: AbstractItemModel + ?Sized>(&self, model: &M, role: ItemDataRole) -> Variant {
        self.index().data(model, role)
    }
}

impl PartialEq for PersistentModelIndex {
    fn eq(&self, other: &Self) -> bool {
        match (&self.data, &other.data) {
            (Some(a), Some(b)) => Rc::ptr_eq(a, b) || a.index.get() == b.index.get(),
            (None, None) => true,
            _ => self.index() == other.index(),
        }
    }
}

impl PartialEq<ModelIndex> for PersistentModelIndex {
    fn eq(&self, other: &ModelIndex) -> bool {
        self.index() == *other
    }
}

impl PartialEq<PersistentModelIndex> for ModelIndex {
    fn eq(&self, other: &PersistentModelIndex) -> bool {
        *self == other.index()
    }
}

impl fmt::Debug for PersistentModelIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Persistent{:?}", self.index())
    }
}
