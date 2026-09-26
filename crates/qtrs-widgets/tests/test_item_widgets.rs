use qtrs_model::StandardItem;
use qtrs_widgets::{ListWidget, TableWidget, TreeWidget};

#[test]
fn list_widget_inserts_removes_and_keeps_model_rows_in_order() {
    let mut list = ListWidget::new();
    list.add_item("first");
    list.add_item("third");
    assert!(list.insert_item(1, "second"));
    assert_eq!(list.count(), 3);
    assert_eq!(list.item(0).as_deref(), Some("first"));
    assert_eq!(list.item(1).as_deref(), Some("second"));
    assert_eq!(list.item(2).as_deref(), Some("third"));
    assert!(list.remove_item(1));
    assert!(!list.remove_item(9));
    assert_eq!(list.item(1).as_deref(), Some("third"));
}

#[test]
fn tree_widget_owns_standard_item_model_and_table_sizes_are_fixed_at_creation() {
    let mut tree = TreeWidget::new();
    let item = StandardItem::new();
    item.set_text("root");
    tree.add_top_level_item(item.clone());
    assert_eq!(tree.top_level_count(), 1);
    assert_eq!(tree.top_level_item(0).unwrap().text(), "root");

    let mut table = TableWidget::new(2, 3);
    assert_eq!((table.row_count(), table.column_count()), (2, 3));
    let cell = StandardItem::new();
    cell.set_text("value");
    assert!(table.set_item(1, 2, cell));
    assert_eq!(table.item(1, 2).unwrap().text(), "value");
    assert!(!table.set_item(2, 0, StandardItem::new()));
}
