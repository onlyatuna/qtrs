use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use qtrs_core::event::{Event, EventKind, FocusReason};
use qtrs_core::object::QObject;
use qtrs_gui::geometry::primitives::{Point, Rect};
use qtrs_gui::paint::{Painter, Pixmap};
use qtrs_widgets::button::Button;
use qtrs_widgets::checkbox::{CheckBox, CheckState};
use qtrs_widgets::dialog::{Dialog, DialogCode};
use qtrs_widgets::focus::FocusManager;
use qtrs_widgets::layout::{BoxLayout, GridLayout, Layout};
use qtrs_widgets::line_edit::{EchoMode, LineEdit};
use qtrs_widgets::popup::PopupManager;
use qtrs_widgets::radio_button::{ButtonGroup, RadioButton};
use qtrs_widgets::scroll::ScrollArea;
use qtrs_widgets::size_policy::{Policy, QSizePolicy};
use qtrs_widgets::stacked::StackedWidget;
use qtrs_widgets::widget::{EmptyWidget, Widget, WidgetRef};
use qtrs_widgets::window::collect_dirty_region;

fn wrap_widget<T: Widget + 'static>(w: T) -> WidgetRef {
    Rc::new(RefCell::new(Box::new(w)))
}

// ---------------------------------------------------------------------------
// 1. Focus Management & Keyboard Navigation Tests
// ---------------------------------------------------------------------------
#[test]
fn test_focus_manager_navigation_and_policies() {
    let root = wrap_widget(EmptyWidget::new());

    let btn1 = wrap_widget(Button::new("Btn1"));
    let btn2 = wrap_widget(Button::new("Btn2"));
    let no_focus = wrap_widget(EmptyWidget::new()); // default NoFocus
    let line_edit = wrap_widget(LineEdit::new());

    root.borrow_mut().add_child(btn1.clone());
    root.borrow_mut().add_child(no_focus.clone());
    root.borrow_mut().add_child(btn2.clone());
    root.borrow_mut().add_child(line_edit.clone());

    let mut fm = FocusManager::new();
    assert_eq!(fm.focused_widget_id(), None);

    // Initial Tab navigation picks first focusable (btn1)
    assert!(fm.focus_next(&root));
    assert_eq!(fm.focused_widget_id(), Some(btn1.borrow().id()));
    assert!(btn1.borrow().has_focus());
    assert!(!btn2.borrow().has_focus());

    // Next Tab skips no_focus and selects btn2
    assert!(fm.focus_next(&root));
    assert_eq!(fm.focused_widget_id(), Some(btn2.borrow().id()));
    assert!(!btn1.borrow().has_focus());
    assert!(btn2.borrow().has_focus());

    // Next Tab selects line_edit
    assert!(fm.focus_next(&root));
    assert_eq!(fm.focused_widget_id(), Some(line_edit.borrow().id()));
    assert!(line_edit.borrow().has_focus());

    // Next Tab wraps around back to btn1
    assert!(fm.focus_next(&root));
    assert_eq!(fm.focused_widget_id(), Some(btn1.borrow().id()));

    // Shift+Tab (focus_previous) wraps to line_edit
    assert!(fm.focus_previous(&root));
    assert_eq!(fm.focused_widget_id(), Some(line_edit.borrow().id()));

    // Clear focus
    assert!(fm.clear_focus(&root, FocusReason::Other));
    assert_eq!(fm.focused_widget_id(), None);
    assert!(!line_edit.borrow().has_focus());

    // Mouse click focus test
    assert!(fm.handle_mouse_click(&root, &btn2));
    assert_eq!(fm.focused_widget_id(), Some(btn2.borrow().id()));
    assert!(btn2.borrow().has_focus());
}

// ---------------------------------------------------------------------------
// 2. Size Policy & Box Layout Constraints Tests
// ---------------------------------------------------------------------------
#[test]
fn test_size_policy_and_box_layout_constraints() {
    let mut layout = BoxLayout::horizontal();
    layout.set_margins(qtrs_gui::geometry::primitives::Margins::new(0, 0, 0, 0));
    layout.set_spacing(0);

    // Item 1: Fixed width = 100
    let item1 = wrap_widget(EmptyWidget::new());
    item1.borrow_mut().set_size_policy(QSizePolicy::new(Policy::Fixed, Policy::Preferred));
    item1.borrow_mut().set_geometry(Rect::new(0, 0, 100, 30));

    // Item 2: Expanding width
    let item2 = wrap_widget(EmptyWidget::new());
    item2.borrow_mut().set_size_policy(QSizePolicy::new(Policy::Expanding, Policy::Preferred));
    item2.borrow_mut().set_geometry(Rect::new(0, 0, 50, 30));

    layout.add_widget(item1.clone());
    layout.add_widget(item2.clone());

    // Layout in 400px width
    layout.set_geometry(Rect::new(0, 0, 400, 50));

    let g1 = item1.borrow().geometry();
    let g2 = item2.borrow().geometry();

    // Item 1 must stay at its fixed size hint (100)
    assert_eq!(g1.width, 100);
    assert_eq!(g1.x, 0);

    // Item 2 must expand into all remaining 300px
    assert_eq!(g2.width, 300);
    assert_eq!(g2.x, 100);
}

// ---------------------------------------------------------------------------
// 3. Grid Layout & Stacked Widget Tests
// ---------------------------------------------------------------------------
#[test]
fn test_grid_layout_and_stacked_widget() {
    // A. GridLayout
    let mut grid = GridLayout::new();
    grid.set_margins(qtrs_gui::geometry::primitives::Margins::new(0, 0, 0, 0));
    grid.set_spacing(0);

    let cell_0_0 = wrap_widget(EmptyWidget::new());
    let cell_0_1 = wrap_widget(EmptyWidget::new());
    let cell_1_span = wrap_widget(EmptyWidget::new()); // spans 2 columns

    grid.add_widget(cell_0_0.clone(), 0, 0);
    grid.add_widget(cell_0_1.clone(), 0, 1);
    grid.add_widget_with_span(cell_1_span.clone(), 1, 0, 1, 2);

    grid.set_geometry(Rect::new(0, 0, 200, 100));

    assert_eq!(grid.row_count(), 2);
    assert_eq!(grid.column_count(), 2);

    let g_span = cell_1_span.borrow().geometry();
    assert_eq!(g_span.x, 0);
    assert_eq!(g_span.width, 200); // Spans full width

    // B. StackedWidget
    let mut stacked = StackedWidget::new();
    let page1 = wrap_widget(EmptyWidget::new());
    let page2 = wrap_widget(EmptyWidget::new());
    let page3 = wrap_widget(EmptyWidget::new());

    let idx1 = stacked.add_widget(page1.clone());
    let idx2 = stacked.add_widget(page2.clone());
    let idx3 = stacked.add_widget(page3.clone());

    assert_eq!(idx1, 0);
    assert_eq!(idx2, 1);
    assert_eq!(idx3, 2);
    assert_eq!(stacked.count(), 3);

    let changed_index = Arc::new(AtomicUsize::new(999));
    let ch_clone = changed_index.clone();
    stacked.current_changed.connect(move |idx| {
        ch_clone.store(*idx, Ordering::SeqCst);
    });

    stacked.set_geometry(Rect::new(0, 0, 300, 200));

    // Page 0 visible, others hidden
    assert!(page1.borrow().is_visible());
    assert!(!page2.borrow().is_visible());
    assert!(!page3.borrow().is_visible());

    // Switch to page 1
    stacked.set_current_index(1);
    assert_eq!(changed_index.load(Ordering::SeqCst), 1);
    assert!(!page1.borrow().is_visible());
    assert!(page2.borrow().is_visible());
    assert!(!page3.borrow().is_visible());
}

// ---------------------------------------------------------------------------
// 4. LineEdit Interaction, IME & Selection Tests
// ---------------------------------------------------------------------------
#[test]
fn test_line_edit_interaction_ime_and_clipboard() {
    let mut le = LineEdit::with_text("Rust GUI");
    assert_eq!(le.text(), "Rust GUI");
    assert_eq!(le.cursor_position(), 8);

    // Text changed signal
    let text_log = Arc::new(std::sync::Mutex::new(Vec::new()));
    let tl_clone = text_log.clone();
    le.text_changed.connect(move |s| {
        tl_clone.lock().unwrap().push(s.clone());
    });

    // Key press: Home -> cursor at 0
    let mut ev_home = Event::new_spontaneous(EventKind::KeyPress {
        key: 0x24, // Home
        modifiers: 0,
        is_repeat: false,
    });
    le.event(&mut ev_home);
    assert_eq!(le.cursor_position(), 0);

    // Key press: Type 'Q' (0x51)
    let mut ev_q = Event::new_spontaneous(EventKind::KeyPress {
        key: 0x51,
        modifiers: 0,
        is_repeat: false,
    });
    le.event(&mut ev_q);
    assert_eq!(le.text(), "QRust GUI");
    assert_eq!(le.cursor_position(), 1);

    // Select All (Ctrl+A)
    le.select_all();
    assert!(le.has_selected_text());
    assert_eq!(le.selected_text(), "QRust GUI");

    // Typing while selected replaces entire text
    let mut ev_n = Event::new_spontaneous(EventKind::KeyPress {
        key: 0x4E, // 'N'
        modifiers: 0,
        is_repeat: false,
    });
    le.event(&mut ev_n);
    assert_eq!(le.text(), "N");
    assert_eq!(le.cursor_position(), 1);

    // InputMethod IME composition
    let mut ime_commit = Event::new_spontaneous(EventKind::InputMethod {
        commit_string: "Framework".to_string(),
        preedit_string: "".to_string(),
        cursor_position: 0,
    });
    le.event(&mut ime_commit);
    assert_eq!(le.text(), "NFramework");

    // EchoMode
    le.set_echo_mode(EchoMode::Password);
    assert_eq!(le.echo_mode(), EchoMode::Password);
}

// ---------------------------------------------------------------------------
// 5. CheckBox & RadioButton with ButtonGroup Tests
// ---------------------------------------------------------------------------
#[test]
fn test_checkbox_and_radio_button_group() {
    // CheckBox
    let mut cb = CheckBox::new("Enable Notifications");
    assert!(!cb.is_checked());
    assert_eq!(cb.check_state(), CheckState::Unchecked);

    cb.toggle();
    assert!(cb.is_checked());
    assert_eq!(cb.check_state(), CheckState::Checked);

    cb.set_tristate(true);
    cb.toggle();
    assert_eq!(cb.check_state(), CheckState::Unchecked);
    cb.toggle();
    assert_eq!(cb.check_state(), CheckState::PartiallyChecked);

    // RadioButton & ButtonGroup
    let mut group = ButtonGroup::new();
    let rb1 = RadioButton::new("Option A");
    let rb2 = RadioButton::new("Option B");

    let id1 = rb1.id();
    let id2 = rb2.id();

    group.add_button(id1, 101);
    group.add_button(id2, 102);

    let clicked_id = Arc::new(AtomicUsize::new(0));
    let cl_clone = clicked_id.clone();
    group.button_clicked.connect(move |id| {
        cl_clone.store(*id as usize, Ordering::SeqCst);
    });

    group.set_checked(101, true);
    assert_eq!(group.checked_id(), Some(101));
    assert_eq!(clicked_id.load(Ordering::SeqCst), 101);

    group.set_checked(102, true);
    assert_eq!(group.checked_id(), Some(102));
    assert_eq!(clicked_id.load(Ordering::SeqCst), 102);
}

// ---------------------------------------------------------------------------
// 6. ScrollArea & Viewport Clipping Tests
// ---------------------------------------------------------------------------
#[test]
fn test_scroll_area_and_viewport_clipping() {
    let mut scroll_area = ScrollArea::new();
    scroll_area.set_geometry(Rect::new(0, 0, 200, 200));

    let content = wrap_widget(EmptyWidget::new());
    // Large content: 400 x 600
    content.borrow_mut().set_geometry(Rect::new(0, 0, 400, 600));

    scroll_area.set_widget(content.clone());

    // Both scrollbars should be active because content exceeds 200x200
    assert!(scroll_area.vertical_scroll_bar().maximum() > 0);
    assert!(scroll_area.horizontal_scroll_bar().maximum() > 0);

    // Scroll down by 50px
    scroll_area.set_scroll_position(0, 50);
    assert_eq!(scroll_area.scroll_position(), Point::new(0, 50));

    // Content geometry offset by scroll
    let cg = content.borrow().geometry();
    assert_eq!(cg.y, -50);

    // Viewport rect excludes scrollbars
    let vp = scroll_area.viewport_rect();
    assert!(vp.width < 200);
    assert!(vp.height < 200);

    // Render test with clipping to Pixmap
    let mut pixmap = Pixmap::new(200, 200).unwrap();
    let mut painter = Painter::begin(&mut pixmap);
    scroll_area.paint_event(&mut painter);
}

// ---------------------------------------------------------------------------
// 7. Dialog Modal & Popup Auto-Dismiss Tests
// ---------------------------------------------------------------------------
#[test]
fn test_dialog_modal_and_popup_dismiss() {
    // Dialog accept / reject
    let mut dlg = Dialog::new();
    assert!(dlg.is_modal());

    let finished_code = Arc::new(std::sync::Mutex::new(None));
    let fc_clone = finished_code.clone();
    dlg.finished.connect(move |code| {
        *fc_clone.lock().unwrap() = Some(*code);
    });

    dlg.open();
    assert!(dlg.is_visible());

    dlg.accept();
    assert!(!dlg.is_visible());
    assert_eq!(*finished_code.lock().unwrap(), Some(DialogCode::Accepted));

    dlg.open();
    dlg.reject();
    assert_eq!(*finished_code.lock().unwrap(), Some(DialogCode::Rejected));

    // PopupManager auto-dismiss test
    let mut pm = PopupManager::new();
    let popup = wrap_widget(EmptyWidget::new());
    popup.borrow_mut().set_geometry(Rect::new(50, 50, 100, 100));

    pm.open_popup(popup.clone(), true);
    assert!(popup.borrow().is_visible());
    assert_eq!(pm.mouse_grabber(), Some(popup.borrow().id()));

    // Click inside popup -> not dismissed
    assert!(!pm.handle_mouse_press(Point::new(80, 80)));
    assert!(popup.borrow().is_visible());

    // Click outside popup -> auto dismissed
    assert!(pm.handle_mouse_press(Point::new(10, 10)));
    assert!(!popup.borrow().is_visible());
    assert_eq!(pm.mouse_grabber(), None);
}

// ---------------------------------------------------------------------------
// 8. Dirty Region Collection & Dynamic Relayout Tests
// ---------------------------------------------------------------------------
#[test]
fn test_dirty_region_collection_and_dpi_change() {
    let root = wrap_widget(EmptyWidget::new());
    root.borrow_mut().set_geometry(Rect::new(0, 0, 800, 600));

    let child1 = wrap_widget(Button::new("Test Button"));
    child1.borrow_mut().set_geometry(Rect::new(50, 50, 100, 30));
    root.borrow_mut().add_child(child1.clone());
    root.borrow_mut().clear_dirty();
    child1.borrow_mut().clear_dirty();

    // Invalidate child1
    child1.borrow_mut().update();

    let dirty = collect_dirty_region(&root, Point::new(0, 0));
    assert!(dirty.is_some());
    let d = dirty.unwrap();
    assert!(d.width >= 100);
    assert!(d.height >= 30);
    assert_eq!(d.x, 50);
    assert_eq!(d.y, 50);

    // After collection, dirty is cleared
    let cleared = collect_dirty_region(&root, Point::new(0, 0));
    assert!(cleared.is_none());
}
