//! Backend-independent scene graph and rendering abstraction.
//!
//! Scene transforms, hit testing, and render nodes share one hierarchy. The
//! initial backend rasterizes on the CPU; additional backends can implement
//! [`Renderer`] without changing scene contents.

mod renderer;
mod scene;

pub use renderer::{Renderer, SoftwareRenderer};
pub use scene::{NodeId, RenderNode, Scene, SceneError};
