use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use qtrs_core::event::{Event, EventKind};
use qtrs_core::object::QObject;
use qtrs_gui::geometry::primitives::{Point, Rect};
use qtrs_widgets::*;

fn empty_widget() -> WidgetRef {
    Rc::new(RefCell::new(Box::new(EmptyWidget::new())))
}

#[test]
fn tab_transitions_moves_and_closes() {
    let mut tabs = TabBar::new();
    tabs.add_tab("First");
    tabs.add_tab("Second");
    tabs.add_tab("Third");
    tabs.set_current_index(1);
    assert_eq!(tabs.current_index(), Some(1));
    tabs.move_tab(1, 2);
    assert_eq!(tabs.tab_text(2), Some("Second"));
    assert_eq!(tabs.current_index(), Some(2));

    let closes = Arc::new(AtomicUsize::new(0));
    let close_count = closes.clone();
    let _connection = tabs.tab_close_requested.connect(move |_| {
        close_count.fetch_add(1, Ordering::SeqCst);
    });
    tabs.set_tabs_closable(true);
    tabs.set_geometry(Rect::new(0, 0, 300, 28));
    tabs.mouse_press_event(Point::new(10, 0), 1, 0);
    tabs.mouse_release_event(Point::new(10, 0), 1, 0);
    assert_eq!(closes.load(Ordering::SeqCst), 1);
    tabs.set_current_index(2);
    tabs.remove_tab(2);
    assert_eq!(tabs.count(), 2);
    assert_eq!(tabs.current_index(), Some(1));
    tabs.set_tab_enabled(1, false);
    assert_ne!(tabs.current_index(), Some(1));
}

#[test]
fn tab_widget_tracks_active_page_visibility() {
    let mut tabs = TabWidget::new();
    let first = empty_widget();
    let second = empty_widget();
    tabs.add_tab(first.clone(), "One");
    tabs.add_tab(second.clone(), "Two");
    assert!(first.borrow().is_visible());
    assert!(!second.borrow().is_visible());
    tabs.set_current_index(1);
    assert!(!first.borrow().is_visible());
    assert!(second.borrow().is_visible());
    assert_eq!(
        tabs.current_widget().unwrap().borrow().id(),
        second.borrow().id()
    );
}

#[test]
fn splitter_drag_respects_stored_minimum_sizes() {
    let mut splitter = Splitter::horizontal();
    splitter.set_geometry(Rect::new(0, 0, 305, 80));
    let left = empty_widget();
    let right = empty_widget();
    splitter.add_widget(left.clone());
    splitter.add_widget(right.clone());
    splitter.set_sizes(vec![150, 150]);
    splitter.set_minimum_size(0, 100);
    splitter.set_minimum_size(1, 120);
    splitter.move_splitter(0, 1000);
    assert!(left.borrow().geometry().width >= 100);
    assert!(right.borrow().geometry().width >= 120);
    assert_eq!(splitter.sizes()[1], 120);
    splitter.move_splitter(0, -1000);
    assert_eq!(splitter.sizes()[0], 100);
}

#[test]
fn menu_keyboard_activation_and_action_group_exclusivity() {
    let mut bar = MenuBar::new();
    let menu = bar.add_menu("&File");
    let calls = Arc::new(AtomicUsize::new(0));
    let called = calls.clone();
    let action = Action::new_ref("Open");
    let _connection = action.borrow().triggered.connect(move |_| {
        called.fetch_add(1, Ordering::SeqCst);
    });
    menu.borrow_mut().add_action(action.clone());
    let mut key = Event::new(EventKind::KeyPress {
        key: b'F' as u32,
        modifiers: 0x0800_0000,
        is_repeat: false,
    });
    assert!(bar.event(&mut key));
    assert_eq!(bar.active_menu().unwrap().borrow().title(), "&File");
    Action::trigger(&action);
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    let a = Action::new_ref("A");
    let b = Action::new_ref("B");
    a.borrow_mut().set_checkable(true);
    b.borrow_mut().set_checkable(true);
    let mut group = ActionGroup::new();
    group.set_exclusive(true);
    group.add_action(&a);
    group.add_action(&b);
    a.borrow_mut().set_checked(true);
    b.borrow_mut().set_checked(true);
    assert!(!a.borrow().is_checked());
    assert!(b.borrow().is_checked());
}

#[test]
fn status_messages_and_main_window_regions_are_calculated() {
    let mut status = StatusBar::new();
    status.show_message("Ready");
    assert_eq!(status.current_message(), "Ready");
    status.clear_message();
    assert_eq!(status.current_message(), "");

    let mut window = MainWindow::new();
    let central = empty_widget();
    window.set_central_widget(central.clone());
    let toolbar: WidgetRef = Rc::new(RefCell::new(Box::new(ToolBar::new("Tools"))));
    window.add_tool_bar(toolbar.clone());
    let dock: WidgetRef = Rc::new(RefCell::new(Box::new(DockWidget::new("Inspector"))));
    window.add_dock_widget(DockArea::Left, dock.clone());
    let bottom_dock: WidgetRef = Rc::new(RefCell::new(Box::new(DockWidget::new("Bottom"))));
    window.add_dock_widget(DockArea::Bottom, bottom_dock.clone());
    window.set_geometry(Rect::new(0, 0, 800, 600));
    let central_rect = central.borrow().geometry();
    assert!(central_rect.y > toolbar.borrow().geometry().y);
    assert!(central_rect.x > dock.borrow().geometry().x);
    assert!(central_rect.y + central_rect.height <= bottom_dock.borrow().geometry().y);
}

#[test]
fn toolbox_selects_and_shows_page() {
    let mut toolbox = ToolBox::new();
    let first = empty_widget();
    let second = empty_widget();
    toolbox.add_item(first.clone(), "First");
    toolbox.add_item(second.clone(), "Second");
    toolbox.set_current_index(1);
    assert!(!first.borrow().is_visible());
    assert!(second.borrow().is_visible());
}
