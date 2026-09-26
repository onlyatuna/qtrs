use qtrs_gui::{Pixmap, PointF, RectF, Transform2D};
use qtrs_render::{RenderNode, Renderer, Scene, SoftwareRenderer};
use tiny_skia::Color;

fn rectangle(color: Color) -> RenderNode {
    RenderNode::FillRect {
        rect: RectF::new(0.0, 0.0, 10.0, 10.0),
        color,
    }
}

#[test]
fn nested_transforms_and_visibility_drive_topmost_hit_test() {
    let mut scene = Scene::new();
    let parent = scene
        .add_node(scene.root(), Transform2D::from_translate(10.0, 0.0), None)
        .unwrap();
    let first = scene
        .add_node(
            parent,
            Transform2D::identity(),
            Some(rectangle(Color::from_rgba8(255, 0, 0, 255))),
        )
        .unwrap();
    let top = scene
        .add_node(
            parent,
            Transform2D::from_translate(5.0, 0.0),
            Some(rectangle(Color::from_rgba8(0, 0, 255, 255))),
        )
        .unwrap();

    assert_eq!(scene.hit_test(PointF::new(16.0, 1.0)), Some(top));
    scene.set_visible(top, false).unwrap();
    assert_eq!(scene.hit_test(PointF::new(16.0, 1.0)), Some(first));
    assert_eq!(scene.hit_test(PointF::new(21.0, 1.0)), None);
}

#[test]
fn software_renderer_rasterizes_scene_and_clears_target() {
    let mut scene = Scene::new();
    scene
        .add_node(
            scene.root(),
            Transform2D::from_translate(2.0, 1.0),
            Some(RenderNode::FillRect {
                rect: RectF::new(0.0, 0.0, 3.0, 3.0),
                color: Color::from_rgba8(220, 20, 30, 255),
            }),
        )
        .unwrap();
    let mut pixmap = Pixmap::new(8, 8).unwrap();
    let mut renderer = SoftwareRenderer;
    renderer.render(&scene, &mut pixmap, Color::WHITE);

    assert_eq!(pixmap.as_tiny_skia().pixel(3, 2).unwrap().red(), 220);
    assert_eq!(pixmap.as_tiny_skia().pixel(0, 0).unwrap().red(), 255);
}

#[test]
fn software_renderer_preserves_device_pixel_ratio() {
    let mut scene = Scene::new();
    scene
        .add_node(
            scene.root(),
            Transform2D::identity(),
            Some(RenderNode::FillRect {
                rect: RectF::new(1.0, 1.0, 2.0, 2.0),
                color: Color::from_rgba8(220, 20, 30, 255),
            }),
        )
        .unwrap();
    let mut pixmap = Pixmap::with_dpr(8, 8, 2.0).unwrap();
    SoftwareRenderer.render(&scene, &mut pixmap, Color::WHITE);

    assert_eq!(pixmap.as_tiny_skia().pixel(2, 2).unwrap().red(), 220);
    assert_eq!(pixmap.as_tiny_skia().pixel(1, 1).unwrap().red(), 255);
}
