use std::fmt;

use qtrs_gui::{PointF, RectF, Transform2D};
use tiny_skia::Color;

/// Stable index into a scene's node arena.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(usize);

impl NodeId {
    pub fn index(self) -> usize {
        self.0
    }
}

/// Backend-independent drawing data attached to a scene node.
#[derive(Debug, Clone)]
pub enum RenderNode {
    FillRect { rect: RectF, color: Color },
}

impl RenderNode {
    fn bounds(&self) -> RectF {
        match self {
            Self::FillRect { rect, .. } => *rect,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Node {
    pub(crate) parent: Option<NodeId>,
    pub(crate) children: Vec<NodeId>,
    pub(crate) transform: Transform2D,
    pub(crate) render: Option<RenderNode>,
    pub(crate) visible: bool,
}

/// Scene graph that unifies transforms, hit testing, and render-node ordering.
#[derive(Debug, Clone)]
pub struct Scene {
    nodes: Vec<Node>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneError {
    InvalidParent(NodeId),
    RootMutation,
}

impl fmt::Display for SceneError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidParent(id) => write!(f, "invalid parent node {}", id.0),
            Self::RootMutation => f.write_str("the scene root cannot be removed or reparented"),
        }
    }
}

impl std::error::Error for SceneError {}

impl Default for Scene {
    fn default() -> Self {
        Self::new()
    }
}

impl Scene {
    pub fn new() -> Self {
        Self {
            nodes: vec![Node {
                parent: None,
                children: Vec::new(),
                transform: Transform2D::identity(),
                render: None,
                visible: true,
            }],
        }
    }

    pub const fn root(&self) -> NodeId {
        NodeId(0)
    }

    pub fn add_node(
        &mut self,
        parent: NodeId,
        transform: Transform2D,
        render: Option<RenderNode>,
    ) -> Result<NodeId, SceneError> {
        if self.nodes.get(parent.0).is_none() {
            return Err(SceneError::InvalidParent(parent));
        }
        let id = NodeId(self.nodes.len());
        self.nodes.push(Node {
            parent: Some(parent),
            children: Vec::new(),
            transform,
            render,
            visible: true,
        });
        self.nodes[parent.0].children.push(id);
        Ok(id)
    }

    pub fn set_transform(&mut self, id: NodeId, transform: Transform2D) -> Result<(), SceneError> {
        let node = self
            .nodes
            .get_mut(id.0)
            .ok_or(SceneError::InvalidParent(id))?;
        node.transform = transform;
        Ok(())
    }

    pub fn set_visible(&mut self, id: NodeId, visible: bool) -> Result<(), SceneError> {
        let node = self
            .nodes
            .get_mut(id.0)
            .ok_or(SceneError::InvalidParent(id))?;
        node.visible = visible;
        Ok(())
    }

    pub fn set_render_node(
        &mut self,
        id: NodeId,
        render: Option<RenderNode>,
    ) -> Result<(), SceneError> {
        let node = self
            .nodes
            .get_mut(id.0)
            .ok_or(SceneError::InvalidParent(id))?;
        node.render = render;
        Ok(())
    }

    pub fn parent(&self, id: NodeId) -> Option<NodeId> {
        self.nodes.get(id.0).and_then(|node| node.parent)
    }

    /// Returns the topmost visible render node containing the scene-space point.
    pub fn hit_test(&self, point: PointF) -> Option<NodeId> {
        self.hit_test_node(self.root(), Transform2D::identity(), point)
    }

    fn hit_test_node(
        &self,
        id: NodeId,
        parent_transform: Transform2D,
        point: PointF,
    ) -> Option<NodeId> {
        let node = &self.nodes[id.0];
        if !node.visible {
            return None;
        }
        let world_transform = parent_transform.post_concat(&node.transform);
        for child in node.children.iter().rev() {
            if let Some(hit) = self.hit_test_node(*child, world_transform, point) {
                return Some(hit);
            }
        }
        let render = node.render.as_ref()?;
        let local_point = world_transform.inverted()?.map_point(point);
        rect_contains(render.bounds(), local_point).then_some(id)
    }

    pub(crate) fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id.0]
    }
}

fn rect_contains(rect: RectF, point: PointF) -> bool {
    rect.width > 0.0
        && rect.height > 0.0
        && point.x >= rect.x
        && point.y >= rect.y
        && point.x < rect.x + rect.width
        && point.y < rect.y + rect.height
}
