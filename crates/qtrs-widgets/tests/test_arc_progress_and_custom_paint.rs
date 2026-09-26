use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use qtrs_core::event::{Event, EventKind};
use qtrs_core::object::{ObjectId, ObjectData, QObject};
use qtrs_gui::geometry::primitives::{Rect, RectF};
use qtrs_gui::paint::pixmap::Pixmap;
use qtrs_gui::paint::{Brush, Painter, Pen};
use qtrs_gui::tiny_skia::Color;
use qtrs_widgets::hit_test::EventTreeDispatcher;
use qtrs_widgets::{
    ArcProgressWidget, CustomWidget, DialWidget, Widget, WidgetBase, WidgetRef,
};

// -----------------------------------------------------------------------------
// 1. Test custom struct overriding Widget::paint_event virtual method
// -----------------------------------------------------------------------------

struct CustomPaintedGauge {
    base: WidgetBase,
    painted_flag: Arc<AtomicBool>,
}

impl CustomPaintedGauge {
    fn new(flag: Arc<AtomicBool>) -> Self {
        Self {
            base: WidgetBase::with_geometry(Rect::new(0, 0, 100, 100)),
            painted_flag: flag,
        }
    }
}

impl QObject for CustomPaintedGauge {
    fn object_data(&self) -> &ObjectData {
        &self.base.object_data
    }
    fn object_data_mut(&mut self) -> &mut ObjectData {
        &mut self.base.object_data
    }
}

impl Widget for CustomPaintedGauge {
    fn id(&self) -> ObjectId {
        self.base.object_data.id
    }
    fn geometry(&self) -> Rect {
        self.base.geometry
    }
    fn set_geometry(&mut self, rect: Rect) {
        self.base.geometry = rect;
    }
    fn is_visible(&self) -> bool {
        self.base.visible
    }
    fn set_visible(&mut self, visible: bool) {
        self.base.visible = visible;
    }
    fn is_enabled(&self) -> bool {
        self.base.enabled
    }
    fn set_enabled(&mut self, enabled: bool) {
        self.base.enabled = enabled;
    }
    fn update(&mut self) {}
    fn dirty_rect(&self) -> Option<Rect> {
        self.base.dirty
    }
    fn clear_dirty(&mut self) {
        self.base.dirty = None;
    }
    fn layout(&self) -> Option<&dyn qtrs_widgets::layout::Layout> {
        None
    }
    fn layout_mut(&mut self) -> Option<&mut Box<dyn qtrs_widgets::layout::Layout>> {
        None
    }
    fn set_layout(&mut self, _layout: Box<dyn qtrs_widgets::layout::Layout>) {}
    fn parent_widget(&self) -> Option<qtrs_widgets::WidgetWeak> {
        self.base.parent.clone()
    }
    fn set_parent_widget(&mut self, parent: Option<qtrs_widgets::WidgetWeak>) {
        self.base.parent = parent;
    }
    fn window_id(&self) -> Option<ObjectId> {
        self.base.window_id
    }
    fn set_window_id(&mut self, window_id: Option<ObjectId>) {
        self.base.window_id = window_id;
    }
    fn children(&self) -> Vec<WidgetRef> {
        Vec::new()
    }
    fn add_child(&mut self, _child: WidgetRef) {}
    fn remove_child(&mut self, _child_id: ObjectId) {}

    // Overriding the virtual paint_event without dirty_rect
    fn paint_event(&mut self, painter: &mut Painter) {
        self.painted_flag.store(true, Ordering::SeqCst);
        painter.set_brush(Brush::Color(Color::from_rgba8(255, 0, 0, 255)));
        painter.draw_rect(RectF::new(0.0, 0.0, 50.0, 50.0));
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[test]
fn test_widget_virtual_paint_event_override() {
    let flag = Arc::new(AtomicBool::new(false));
    let mut gauge = CustomPaintedGauge::new(Arc::clone(&flag));

    let mut pixmap = Pixmap::new(100, 100).expect("Pixmap 建立失敗");
    let mut painter = Painter::begin(&mut pixmap);

    gauge.paint_event(&mut painter);
    assert!(flag.load(Ordering::SeqCst), "自訂 paint_event 應成功執行");

    // 檢查繪圖輸出 (50x50 紅色區塊)
    let data = pixmap.data();
    // 座標 (25, 25) 應為紅色 RGBA: [255, 0, 0, 255]
    let idx = (25 * 100 + 25) * 4;
    assert_eq!(data[idx], 255);
    assert_eq!(data[idx + 1], 0);
    assert_eq!(data[idx + 2], 0);
    assert_eq!(data[idx + 3], 255);
}

// -----------------------------------------------------------------------------
// 2. Test EmptyWidget / CustomWidget closure-based paint handler
// -----------------------------------------------------------------------------

#[test]
fn test_custom_widget_paint_handler_closure() {
    let mut widget = CustomWidget::with_geometry(Rect::new(0, 0, 80, 80));
    let executed = Arc::new(AtomicBool::new(false));
    let executed_clone = Arc::clone(&executed);

    widget.set_paint_handler(move |painter| {
        executed_clone.store(true, Ordering::SeqCst);
        painter.set_pen(Pen::from_rgba8(0, 255, 0, 255, 2.0));
        painter.draw_line(
            qtrs_gui::geometry::primitives::PointF::new(0.0, 0.0),
            qtrs_gui::geometry::primitives::PointF::new(80.0, 80.0),
        );
    });

    let mut pixmap = Pixmap::new(80, 80).expect("Pixmap 建立失敗");
    let mut painter = Painter::begin(&mut pixmap);

    widget.paint_event(&mut painter);
    assert!(executed.load(Ordering::SeqCst), "自訂繪製閉包應被觸發");
}

// -----------------------------------------------------------------------------
// 3. Test ArcProgressWidget value range, percentage, and signal
// -----------------------------------------------------------------------------

#[test]
fn test_arc_progress_values_and_signal() {
    let mut arc = ArcProgressWidget::new();
    assert_eq!(arc.value(), 0.0);
    assert_eq!(arc.min_value(), 0.0);
    assert_eq!(arc.max_value(), 100.0);
    assert_eq!(arc.progress_ratio(), 0.0);
    assert_eq!(arc.progress_percentage(), 0.0);

    let received = Arc::new(std::sync::Mutex::new(Vec::new()));
    let rec_clone = Arc::clone(&received);
    let _conn = arc.value_changed.connect(move |val: &f32| {
        rec_clone.lock().unwrap().push(*val);
    });

    arc.set_value(75.5);
    assert_eq!(arc.value(), 75.5);
    assert_eq!(arc.progress_ratio(), 0.755);
    assert_eq!(arc.progress_percentage(), 75.5);

    // 驗證超過上限自動裁切 (clamp)
    arc.set_value(150.0);
    assert_eq!(arc.value(), 100.0);

    // 驗證低於下限自動裁切
    arc.set_value(-20.0);
    assert_eq!(arc.value(), 0.0);

    let values = received.lock().unwrap().clone();
    assert_eq!(values, vec![75.5, 100.0, 0.0]);
}

// -----------------------------------------------------------------------------
// 4. Test ArcProgressWidget presets and angles
// -----------------------------------------------------------------------------

#[test]
fn test_arc_progress_presets() {
    let standard = ArcProgressWidget::new();
    assert_eq!(standard.start_angle(), 225.0);
    assert_eq!(standard.span_angle(), -270.0);

    let ring = ArcProgressWidget::full_ring();
    assert_eq!(ring.start_angle(), 90.0);
    assert_eq!(ring.span_angle(), -360.0);

    let semi = ArcProgressWidget::semicircle();
    assert_eq!(semi.start_angle(), 180.0);
    assert_eq!(semi.span_angle(), -180.0);
}

// -----------------------------------------------------------------------------
// 5. Test multi-tier threshold colors (Normal -> Warning -> Critical)
// -----------------------------------------------------------------------------

#[test]
fn test_threshold_colors() {
    let mut arc = ArcProgressWidget::new();
    let normal_c = Color::from_rgba8(0, 255, 0, 255);
    let warn_c = Color::from_rgba8(255, 200, 0, 255);
    let crit_c = Color::from_rgba8(255, 0, 0, 255);

    arc.set_threshold_colors(normal_c, warn_c, crit_c, 70.0, 90.0);

    arc.set_value(50.0);
    assert_eq!(arc.current_progress_color(), normal_c);

    arc.set_value(75.0);
    assert_eq!(arc.current_progress_color(), warn_c);

    arc.set_value(95.0);
    assert_eq!(arc.current_progress_color(), crit_c);
}

// -----------------------------------------------------------------------------
// 6. Test ArcProgressWidget rendering to Pixmap
// -----------------------------------------------------------------------------

#[test]
fn test_arc_progress_rendering() {
    let mut arc = ArcProgressWidget::new();
    arc.set_geometry(Rect::new(0, 0, 100, 100));
    arc.set_track_width(6.0);
    arc.set_progress_width(6.0);
    arc.set_value(50.0);
    arc.set_title("CPU");

    let mut pixmap = Pixmap::new(100, 100).expect("Pixmap 建立失敗");
    let mut painter = Painter::begin(&mut pixmap);

    arc.paint_event(&mut painter);

    // 驗證有畫出非透明像素 (背景軌道與進度弧線)
    let mut non_zero_pixels = 0;
    for pixel in pixmap.data().chunks(4) {
        if pixel[3] > 0 {
            non_zero_pixels += 1;
        }
    }
    assert!(non_zero_pixels > 100, "進度環形計量表應繪製出可見像素");
}

// -----------------------------------------------------------------------------
// 7. Test interactive DialWidget mouse drag and wheel input
// -----------------------------------------------------------------------------

#[test]
fn test_interactive_dial_widget() {
    let dial_ref: WidgetRef = Rc::new(RefCell::new(Box::new(DialWidget::new())));
    {
        let mut dial = dial_ref.borrow_mut();
        dial.set_geometry(Rect::new(0, 0, 200, 200));
        if let Some(d) = dial.as_any_mut().downcast_mut::<ArcProgressWidget>() {
            d.set_interactive(true);
            d.set_range(0.0, 100.0);
            d.set_step(5.0);
            d.set_value(0.0);
        }
    }

    let mut dispatcher = EventTreeDispatcher::new();

    // 點擊頂部中央 (x: 100, y: 10) -> 對應角度約 90 度 (12 點鐘)，進度約 50%
    let mut press_ev = Event::new_spontaneous(EventKind::MouseButtonPress {
        x: 100,
        y: 10,
        button: 1,
    });
    dispatcher.dispatch_event(&dial_ref, &mut press_ev);

    let val = {
        let dial = dial_ref.borrow();
        let d = dial.as_any().downcast_ref::<ArcProgressWidget>().unwrap();
        d.value()
    };
    // 90 度位處 225 度與 -45 度的中間，比值約 0.5 (即 50%)
    assert!((val - 50.0).abs() < 5.0, "點擊頂部中央應將數值調整至約 50%，實際為: {}", val);

    // 測試滑鼠滾輪向上增加數值
    let mut wheel_up = Event::new_spontaneous(EventKind::Wheel {
        x: 100,
        y: 100,
        pixel_delta_x: 0,
        pixel_delta_y: 120,
        angle_delta_x: 0,
        angle_delta_y: 120,
        modifiers: 0,
    });
    dispatcher.dispatch_event(&dial_ref, &mut wheel_up);

    let val_after_wheel = {
        let dial = dial_ref.borrow();
        let d = dial.as_any().downcast_ref::<ArcProgressWidget>().unwrap();
        d.value()
    };
    assert!((val_after_wheel - (val + 5.0)).abs() < 1e-3, "滾輪應增加 step (5.0)");
}
