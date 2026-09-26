//! # qtrs-model
//!
//! Qt Item Model / View / Selection foundations for `qtrs`, matching
//! `Qt Core`'s `itemmodels` module:
//!
//! - [`ModelIndex`] / [`PersistentModelIndex`] (`QModelIndex`)
//! - [`AbstractItemModel`] / [`AbstractListModel`] / [`AbstractTableModel`]
//! - [`StringListModel`] / [`StandardItemModel`]
//! - [`IdentityProxyModel`] / [`SortFilterProxyModel`]
//! - [`ItemSelectionModel`] and selection helpers
//!
//! Models are shared with views as [`SharedModel`] (`Rc<RefCell<dyn AbstractItemModel>>`).

pub mod index;
pub mod list_model;
pub mod model;
pub mod pattern;
pub mod proxy;
pub mod role;
pub mod selection;
pub mod standard;

pub use index::{ModelIndex, PersistentModelIndex};
pub use list_model::{AbstractListModel, AbstractTableModel, StringListModel};
pub use model::{
    shared_model, AbstractItemModel, ModelBase, ModelConnection, ModelEvent, ModelListener,
    ModelSignals, SharedModel,
};
pub use pattern::{PatternError, Regex};
pub use proxy::{AbstractProxyModel, FilterSyntax, IdentityProxyModel, SortFilterProxyModel};
pub use role::{
    CaseSensitivity, CheckState, ItemDataRole, ItemFlags, MatchFlags, Orientation, SortOrder,
};
pub use selection::{ItemSelection, ItemSelectionModel, ItemSelectionRange, SelectionFlags};
pub use standard::{StandardItem, StandardItemModel};

/// Canonical Qt aliases.
pub type QModelIndex = ModelIndex;
pub type QPersistentModelIndex = PersistentModelIndex;
pub type QAbstractItemModel = dyn AbstractItemModel;
pub type QAbstractListModel = dyn AbstractListModel;
pub type QAbstractTableModel = dyn AbstractTableModel;
pub type QStringListModel = StringListModel;
pub type QStandardItem = StandardItem;
pub type QStandardItemModel = StandardItemModel;
pub type QAbstractProxyModel = dyn AbstractProxyModel;
pub type QIdentityProxyModel = IdentityProxyModel;
pub type QSortFilterProxyModel = SortFilterProxyModel;
pub type QItemSelection = ItemSelection;
pub type QItemSelectionRange = ItemSelectionRange;
pub type QItemSelectionModel = ItemSelectionModel;
