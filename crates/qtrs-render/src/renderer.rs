use qtrs_gui::{Painter, Pixmap, Transform2D};
use tiny_skia::Color;

use crate::scene::{NodeId, RenderNode, Scene};

/// Executes a scene on a render target.
pub trait Renderer {
    fn render(&mut self, scene: &Scene, target: &mut Pixmap, clear: Color);
}

/// CPU raster renderer backed by qtrs-gui's painter and tiny-skia.
#[derive(Debug, Default)]
pub struct SoftwareRenderer;

impl Renderer for SoftwareRenderer {
    fn render(&mut self, scene: &Scene, target: &mut Pixmap, clear: Color) {
        let dpr = target.physical_width() as f32 / target.logical_width();
        let width = target.logical_width();
        let height = target.logical_height();
        let mut painter = Painter::begin(target);
        painter.fill_rect(qtrs_gui::RectF::new(0.0, 0.0, width, height), clear);
        draw_node(
            scene,
            scene.root(),
            Transform2D::from_scale(dpr, dpr),
            &mut painter,
        );
    }
}

fn draw_node(scene: &Scene, id: NodeId, parent_transform: Transform2D, painter: &mut Painter<'_>) {
    let node = scene.node(id);
    if !node.visible {
        return;
    }
    let world_transform = parent_transform.post_concat(&node.transform);
    if let Some(render) = &node.render {
        painter.set_transform(&world_transform);
        match render {
            RenderNode::FillRect { rect, color } => painter.fill_rect(*rect, *color),
        }
    }
    for child in &node.children {
        draw_node(scene, *child, world_transform, painter);
    }
}
