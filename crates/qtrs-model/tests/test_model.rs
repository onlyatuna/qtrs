//! Integration tests for qtrs-model: StringListModel, StandardItemModel,
//! SortFilterProxyModel, and ItemSelectionModel.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use qtrs_core::Variant;
use qtrs_model::{
    shared_model, AbstractItemModel, AbstractProxyModel, CaseSensitivity, ItemDataRole,
    ItemSelectionModel, MatchFlags, ModelIndex, SelectionFlags, SortFilterProxyModel, SortOrder,
    StandardItem, StandardItemModel, StringListModel,
};

fn collect_display(model: &dyn AbstractItemModel) -> Vec<String> {
    let rows = model.row_count(&ModelIndex::INVALID);
    (0..rows)
        .map(|r| {
            let idx = model.index(r, 0, &ModelIndex::INVALID);
            model.data(&idx, ItemDataRole::Display).to_string_lossy()
        })
        .collect()
}

#[test]
fn string_list_model_insert_remove_set_data_and_signals() {
    let model = StringListModel::with_strings(["alpha", "beta", "gamma"]);
    assert_eq!(model.count(), 3);
    assert_eq!(model.string_list(), vec!["alpha", "beta", "gamma"]);

    let inserted = Arc::new(Mutex::new(None));
    let removed = Arc::new(Mutex::new(None));
    let changed = Arc::new(AtomicUsize::new(0));
    let reset = Arc::new(AtomicUsize::new(0));

    let inserted_c = inserted.clone();
    let _c1 = model
        .signals()
        .rows_inserted
        .connect(move |(parent, first, last)| {
            *inserted_c.lock().unwrap() = Some((parent.is_valid(), *first, *last));
        });
    let removed_c = removed.clone();
    let _c2 = model
        .signals()
        .rows_removed
        .connect(move |(parent, first, last)| {
            *removed_c.lock().unwrap() = Some((parent.is_valid(), *first, *last));
        });
    let changed_c = changed.clone();
    let _c3 = model.signals().data_changed.connect(move |_| {
        changed_c.fetch_add(1, Ordering::SeqCst);
    });
    let reset_c = reset.clone();
    let _c4 = model.signals().model_reset.connect(move |_| {
        reset_c.fetch_add(1, Ordering::SeqCst);
    });

    assert!(model.insert_rows(1, 1, &ModelIndex::INVALID));
    assert_eq!(*inserted.lock().unwrap(), Some((false, 1, 1)));
    assert_eq!(model.count(), 4);
    assert_eq!(model.string_at(1).as_deref(), Some(""));

    let idx = model.index(1, 0, &ModelIndex::INVALID);
    assert!(model.set_data(&idx, Variant::String("delta".into()), ItemDataRole::Edit));
    assert_eq!(changed.load(Ordering::SeqCst), 1);
    assert_eq!(model.string_at(1).as_deref(), Some("delta"));

    assert!(model.remove_rows(0, 1, &ModelIndex::INVALID));
    assert_eq!(*removed.lock().unwrap(), Some((false, 0, 0)));
    assert_eq!(model.string_list(), vec!["delta", "beta", "gamma"]);

    model.set_string_list(["z", "a"]);
    assert_eq!(reset.load(Ordering::SeqCst), 1);
    assert_eq!(model.string_list(), vec!["z", "a"]);

    model.sort(0, SortOrder::Ascending);
    assert_eq!(model.string_list(), vec!["a", "z"]);
}

#[test]
fn standard_item_model_tree_parent_child_find_and_sort() {
    let model = StandardItemModel::new();
    let root_a = StandardItem::with_text("Animals");
    let root_b = StandardItem::with_text("Plants");
    let dog = StandardItem::with_text("Dog");
    let cat = StandardItem::with_text("Cat");
    root_a.append_row(vec![dog.clone()]);
    root_a.append_row(vec![cat.clone()]);
    model.append_row(vec![root_a.clone()]);
    model.append_row(vec![root_b.clone()]);

    assert_eq!(model.row_count(&ModelIndex::INVALID), 2);
    let animals = model.index(0, 0, &ModelIndex::INVALID);
    assert!(animals.is_valid());
    assert_eq!(
        model
            .data(&animals, ItemDataRole::Display)
            .to_string_lossy(),
        "Animals"
    );
    assert_eq!(model.row_count(&animals), 2);

    let dog_idx = model.index(0, 0, &animals);
    let parent = model.parent(&dog_idx);
    assert_eq!(parent, animals);
    assert_eq!(
        model
            .data(&dog_idx, ItemDataRole::Display)
            .to_string_lossy(),
        "Dog"
    );

    let found = model.find_items("Cat", MatchFlags::EXACTLY | MatchFlags::RECURSIVE, 0);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].text(), "Cat");

    // Sort children of Animals ascending by display text.
    root_a.sort_children(0, SortOrder::Ascending);
    let first = model.index(0, 0, &animals);
    let second = model.index(1, 0, &animals);
    assert_eq!(
        model.data(&first, ItemDataRole::Display).to_string_lossy(),
        "Cat"
    );
    assert_eq!(
        model.data(&second, ItemDataRole::Display).to_string_lossy(),
        "Dog"
    );
}

#[test]
fn sort_filter_proxy_filter_sort_and_mapping() {
    let source = shared_model(StringListModel::with_strings([
        "Cherry", "apple", "Banana", "apricot",
    ]));
    let proxy = SortFilterProxyModel::with_source(source.clone());
    proxy.set_filter_case_sensitivity(CaseSensitivity::Insensitive);
    proxy.set_filter_fixed_string("ap");

    assert_eq!(proxy.row_count(&ModelIndex::INVALID), 2);
    let displays = collect_display(&proxy);
    assert!(displays.iter().all(|s| s.to_lowercase().contains("ap")));

    // Mapping: proxy row 0 maps to a source row containing "ap".
    let p0 = proxy.index(0, 0, &ModelIndex::INVALID);
    let s0 = proxy.map_to_source(&p0);
    assert!(s0.is_valid());
    let back = proxy.map_from_source(&s0);
    assert_eq!(back, p0);

    proxy.sort(0, SortOrder::Ascending);
    let sorted = collect_display(&proxy);
    let mut expected = sorted.clone();
    expected.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
    assert_eq!(sorted, expected);

    // Source change triggers re-filter.
    {
        let src = source.borrow();
        let n = src.row_count(&ModelIndex::INVALID);
        assert!(src.insert_rows(n, 1, &ModelIndex::INVALID));
        let idx = src.index(n, 0, &ModelIndex::INVALID);
        assert!(src.set_data(
            &idx,
            Variant::String("Application".into()),
            ItemDataRole::Edit
        ));
    }
    assert_eq!(proxy.row_count(&ModelIndex::INVALID), 3);
}

#[test]
fn sort_filter_proxy_recursive_tree_filter() {
    let model = StandardItemModel::new();
    let parent = StandardItem::with_text("Parent");
    let child_keep = StandardItem::with_text("keep-me");
    let child_drop = StandardItem::with_text("other");
    parent.append_row(vec![child_keep]);
    parent.append_row(vec![child_drop]);
    model.append_row(vec![parent]);
    model.append_row(vec![StandardItem::with_text("Sibling")]);

    let shared = shared_model(model);
    let proxy = SortFilterProxyModel::with_source(shared);
    proxy.set_recursive_filtering_enabled(true);
    proxy.set_filter_fixed_string("keep");

    // Parent survives because a descendant matches.
    assert_eq!(proxy.row_count(&ModelIndex::INVALID), 1);
    let p = proxy.index(0, 0, &ModelIndex::INVALID);
    assert_eq!(
        proxy.data(&p, ItemDataRole::Display).to_string_lossy(),
        "Parent"
    );
    assert_eq!(proxy.row_count(&p), 1);
    let c = proxy.index(0, 0, &p);
    assert_eq!(
        proxy.data(&c, ItemDataRole::Display).to_string_lossy(),
        "keep-me"
    );
}

#[test]
fn item_selection_model_select_toggle_rows_and_current() {
    let model = shared_model(StringListModel::with_strings(["a", "b", "c", "d"]));
    let sel = ItemSelectionModel::new(Some(model.clone()));

    let current_changes = Arc::new(AtomicUsize::new(0));
    let selection_changes = Arc::new(AtomicUsize::new(0));
    let cc = current_changes.clone();
    let _c1 = sel.current_changed.connect(move |_| {
        cc.fetch_add(1, Ordering::SeqCst);
    });
    let sc = selection_changes.clone();
    let _c2 = sel.selection_changed.connect(move |_| {
        sc.fetch_add(1, Ordering::SeqCst);
    });

    let idx1 = {
        let m = model.borrow();
        m.index(1, 0, &ModelIndex::INVALID)
    };
    sel.set_current_index(&idx1, SelectionFlags::CLEAR_AND_SELECT);
    assert_eq!(sel.current_index(), idx1);
    assert!(sel.is_selected(&idx1));
    assert_eq!(sel.selected_indexes().len(), 1);
    assert!(current_changes.load(Ordering::SeqCst) >= 1);
    assert!(selection_changes.load(Ordering::SeqCst) >= 1);

    let idx2 = {
        let m = model.borrow();
        m.index(2, 0, &ModelIndex::INVALID)
    };
    sel.select_index(&idx2, SelectionFlags::TOGGLE);
    assert!(sel.is_selected(&idx2));
    assert_eq!(sel.selected_indexes().len(), 2);

    sel.select_index(&idx2, SelectionFlags::TOGGLE);
    assert!(!sel.is_selected(&idx2));

    // Select rows: selecting one cell with ROWS expands to the row.
    sel.clear();
    sel.select_index(
        &idx1,
        SelectionFlags::CLEAR_AND_SELECT | SelectionFlags::ROWS,
    );
    let rows = sel.selected_rows(0);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].row(), 1);
}
