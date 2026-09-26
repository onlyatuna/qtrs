use qtrs_widgets::{AbstractSlider, ComboBox, Dial, DoubleSpinBox, ProgressBar, Slider, SpinBox};

#[test]
fn spin_boxes_clamp_step_wrap_and_round() {
    let mut int = SpinBox::new();
    int.set_range(1, 3);
    int.set_value(99);
    assert_eq!(int.value(), 3);
    int.set_wrapping(true);
    int.step_up();
    assert_eq!(int.value(), 1);

    let mut float = DoubleSpinBox::new();
    float.set_decimals(2);
    float.set_range(0.0, 1.0);
    float.set_value(0.126);
    assert_eq!(float.value(), 0.13);
}

#[test]
fn slider_clamps_range_and_dial_angle_maps_bounds() {
    let mut slider = Slider::new();
    slider.set_range(-10, 10);
    slider.set_value(50);
    assert_eq!(slider.value(), 10);
    let mut dial = Dial::new();
    dial.set_range(0, 100);
    assert_eq!(dial.value_for_angle(dial.angle_for_value(0)), 0);
    assert_eq!(dial.value_for_angle(dial.angle_for_value(100)), 100);
}

#[test]
fn progress_bar_format_percent_and_busy_state() {
    let mut progress = ProgressBar::new();
    progress.set_range(0, 200);
    progress.set_value(50);
    progress.set_format("P=%p, value=%v max=%m");
    assert_eq!(progress.text(), "P=25, value=50 max=200");

    progress.set_range(0, 0);
    progress.set_value(1);
    assert!(progress.is_busy());
}

#[test]
fn combo_box_data_selection_and_out_of_range_clear() {
    let mut combo = ComboBox::new();
    combo.add_items(["red", "green", "blue"]);
    combo.set_current_index(1);
    assert_eq!(combo.current_text(), "green");
    combo.set_current_index(99);
    assert_eq!(combo.current_index(), -1);
}
