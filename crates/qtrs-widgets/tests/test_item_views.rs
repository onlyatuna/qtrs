use qtrs_core::event::{Event, EventKind};
use qtrs_core::object::QObject;
use qtrs_core::Variant;
use qtrs_model::{shared_model, ItemDataRole, ModelIndex, StringListModel};
use qtrs_widgets::{AbstractItemView, DataWidgetMapper, ViewMode, Widget};

#[test]
fn concrete_views_choose_their_matching_layout_modes() {
    let model = shared_model(StringListModel::new());
    assert_eq!(
        qtrs_widgets::ListView::new(model.clone())
            .view()
            .view_mode(),
        ViewMode::List
    );
    assert_eq!(
        qtrs_widgets::TreeView::new(model.clone())
            .view()
            .view_mode(),
        ViewMode::Tree
    );
    assert_eq!(
        qtrs_widgets::TableView::new(model.clone())
            .view()
            .view_mode(),
        ViewMode::Table
    );
    assert_eq!(
        qtrs_widgets::ColumnView::new(model).view().view_mode(),
        ViewMode::Column
    );
}

#[test]
fn tree_view_flattens_nested_rows_and_navigation_keeps_model_indexes() {
    let model = qtrs_model::StandardItemModel::new();
    let root_item = qtrs_model::StandardItem::new();
    root_item.set_text("root");
    let child_item = qtrs_model::StandardItem::new();
    child_item.set_text("child");
    root_item.append_row(vec![child_item]);
    model.append_row(vec![root_item]);
    let shared = shared_model(model);
    let mut tree = qtrs_widgets::TreeView::new(shared.clone());
    tree.set_geometry(qtrs_gui::geometry::primitives::Rect::new(0, 0, 240, 80));

    let child = tree
        .view()
        .index_at(qtrs_gui::geometry::primitives::Point::new(24, 30));
    assert!(child.is_valid());
    assert_eq!(
        shared
            .borrow()
            .data(&child, ItemDataRole::Display)
            .to_string_lossy(),
        "child"
    );
    let mut down = Event::new_spontaneous(EventKind::KeyPress {
        key: 0x28,
        modifiers: 0,
        is_repeat: false,
    });
    tree.event(&mut down);
    assert_eq!(tree.current_index(), child);
}
#[test]
fn list_view_hit_testing_selection_keyboard_and_scroll() {
    let model = shared_model(StringListModel::with_strings([
        "first", "second", "third", "fourth",
    ]));
    let mut view = AbstractItemView::with_mode(model.clone(), ViewMode::List);
    view.set_geometry(qtrs_gui::geometry::primitives::Rect::new(0, 0, 180, 48));

    let first = view.index_at(qtrs_gui::geometry::primitives::Point::new(20, 10));
    assert_eq!(first.row, 0);
    let mut click = Event::new_spontaneous(EventKind::MouseButtonPress {
        x: 20,
        y: 30,
        button: 1,
    });
    view.event(&mut click);
    assert_eq!(view.current_index().row, 1);
    assert_eq!(view.selection_model().selected_indexes().len(), 1);

    let mut down = Event::new_spontaneous(EventKind::KeyPress {
        key: 0x28,
        modifiers: 0,
        is_repeat: false,
    });
    view.event(&mut down);
    assert_eq!(view.current_index().row, 2);
    assert_eq!(view.vertical_offset(), 1);
}

#[test]
fn table_view_renders_multiple_model_columns_and_mapper_edits_role_data() {
    let model = shared_model(StringListModel::with_strings(["alpha", "beta"]));
    let mut view = AbstractItemView::with_mode(model.clone(), ViewMode::Table);
    let root = ModelIndex::INVALID;
    let first = model.borrow().index(0, 0, &root);
    assert!(first.is_valid());
    assert_eq!(
        model
            .borrow()
            .data(&first, ItemDataRole::Display)
            .to_string_lossy(),
        "alpha"
    );

    let mut mapper = DataWidgetMapper::new();
    mapper.set_model(model.clone());
    mapper.set_current_index(1);
    assert_eq!(mapper.current_data().unwrap().to_string_lossy(), "beta");
    assert!(mapper.set_current_data(Variant::String("changed".into())));
    assert_eq!(
        model
            .borrow()
            .data(&model.borrow().index(1, 0, &root), ItemDataRole::Display)
            .to_string_lossy(),
        "changed"
    );

    view.set_geometry(qtrs_gui::geometry::primitives::Rect::new(0, 0, 320, 100));
    assert_eq!(
        view.index_at(qtrs_gui::geometry::primitives::Point::new(8, 32))
            .row,
        0
    );
}
