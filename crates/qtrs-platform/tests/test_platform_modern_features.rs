use qtrs_gui::geometry::primitives::{Point, Rect};
use qtrs_platform::backdrop::{set_window_backdrop, BackdropType};
use qtrs_platform::drag_drop::{DropAction, DropEvent};
use qtrs_platform::ime::CompositionContext;
use qtrs_platform::window_system_interface::{WindowSystemEvent, WindowSystemEventHandler};
use qtrs_platform::{PlatformWindow, WindowFlags};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[test]
fn test_backdrop_type_variants() {
    let none = BackdropType::None;
    let mica = BackdropType::Mica;
    let mica_alt = BackdropType::MicaAlt;
    let acrylic = BackdropType::Acrylic;
    let blur_behind = BackdropType::BlurBehind;

    assert_ne!(none, mica);
    assert_ne!(mica, mica_alt);
    assert_ne!(acrylic, blur_behind);

    // Calling with a null/dummy handle should safely return false without crashing
    #[cfg(windows)]
    {
        let null_hwnd = std::ptr::null_mut();
        assert!(!set_window_backdrop(null_hwnd, BackdropType::Mica, true));
        assert!(!set_window_backdrop(null_hwnd, BackdropType::Acrylic, false));
    }
}

#[test]
fn test_ime_composition_context() {
    let mut ctx = CompositionContext::default();
    assert!(!ctx.is_composing);
    assert_eq!(ctx.composition_string, "");
    assert_eq!(ctx.cursor_position, 0);

    ctx.is_composing = true;
    ctx.composition_string = "nihao".to_string();
    ctx.cursor_position = 5;

    assert!(ctx.is_composing);
    assert_eq!(ctx.composition_string, "nihao");
    assert_eq!(ctx.cursor_position, 5);
}

#[test]
fn test_drag_drop_actions_and_events() {
    assert_eq!(DropAction::Ignore as u32, 0);
    assert_eq!(DropAction::Copy as u32, 1);
    assert_eq!(DropAction::Move as u32, 2);
    assert_eq!(DropAction::Link as u32, 4);

    let enter_ev = DropEvent::Enter {
        pos: Point::new(100, 200),
        formats: vec!["text/uri-list".to_string()],
        effect: 1,
    };

    match enter_ev {
        DropEvent::Enter { pos, formats, effect } => {
            assert_eq!(pos, Point::new(100, 200));
            assert_eq!(formats, vec!["text/uri-list".to_string()]);
            assert_eq!(effect, 1);
        }
        _ => panic!("Expected DropEvent::Enter"),
    }
}

#[test]
fn test_dpi_change_and_ime_events_in_window_system() {
    struct MockEventHandler {
        received_dpi: Arc<AtomicBool>,
        received_ime: Arc<AtomicBool>,
        received_drop: Arc<AtomicBool>,
    }

    impl WindowSystemEventHandler for MockEventHandler {
        fn handle_window_event(&mut self, event: WindowSystemEvent) {
            match event {
                WindowSystemEvent::DpiChanged { dpi_x, dpi_y } => {
                    if dpi_x == 192 && dpi_y == 192 {
                        self.received_dpi.store(true, Ordering::SeqCst);
                    }
                }
                WindowSystemEvent::InputMethod { commit_string, preedit_string, .. } => {
                    if commit_string == "你好" || preedit_string == "nihao" {
                        self.received_ime.store(true, Ordering::SeqCst);
                    }
                }
                WindowSystemEvent::Drop { pos, formats, .. } => {
                    if pos == Point::new(50, 50) && formats.contains(&"text/plain".to_string()) {
                        self.received_drop.store(true, Ordering::SeqCst);
                    }
                }
                _ => {}
            }
        }
    }

    let dpi_flag = Arc::new(AtomicBool::new(false));
    let ime_flag = Arc::new(AtomicBool::new(false));
    let drop_flag = Arc::new(AtomicBool::new(false));

    let mut handler = MockEventHandler {
        received_dpi: dpi_flag.clone(),
        received_ime: ime_flag.clone(),
        received_drop: drop_flag.clone(),
    };

    // Dispatch DPI Changed
    handler.handle_window_event(WindowSystemEvent::DpiChanged { dpi_x: 192, dpi_y: 192 });
    assert!(dpi_flag.load(Ordering::SeqCst));

    // Dispatch IME event
    handler.handle_window_event(WindowSystemEvent::InputMethod {
        commit_string: "你好".to_string(),
        preedit_string: "".to_string(),
        cursor_position: 2,
    });
    assert!(ime_flag.load(Ordering::SeqCst));

    // Dispatch Drop event
    handler.handle_window_event(WindowSystemEvent::Drop {
        pos: Point::new(50, 50),
        formats: vec!["text/plain".to_string()],
        data: vec![("text/plain".to_string(), b"hello".to_vec())],
        drop_action: 1,
    });
    assert!(drop_flag.load(Ordering::SeqCst));
}

#[test]
fn test_platform_window_advanced_features_support() {
    let mut win = qtrs_platform::window::NativeWindow::new(
        "TestAdvancedFeatures",
        Rect::new(100, 100, 400, 300),
        WindowFlags::FRAMELESS,
    ).expect("Window creation should succeed");

    // Test backdrop call
    let _ = win.set_backdrop(BackdropType::Mica, false);

    // Test IME microfocus positioning
    win.set_ime_focus(Point::new(120, 80));

    // Test drop target toggle
    let _ = win.enable_drop_target(true);
    let _ = win.enable_drop_target(false);
}
