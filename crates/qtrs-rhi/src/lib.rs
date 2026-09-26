//! # qtrs-rhi
//!
//! Rendering Hardware Interface (QRhi) subsystem for `qtrs`, modernizing Qt 6 RHI.
//!
//! This crate provides a unified, hardware-independent graphics and compute abstraction
//! matching Qt 6's `QRhi` architecture, enabling accelerated rendering across Vulkan,
//! Metal, Direct3D 11/12, and OpenGL ES.
//!
//! ## Core Architecture
//!
//! - [`Rhi`]: Device instance managing adapters, pipelines, and frame lifecycles (`QRhi`).
//! - [`Buffer`]: Vertex, index, uniform, and storage buffer memory (`QRhiBuffer`).
//! - [`Texture`]: 2D, 3D, Cube, and Array textures (`QRhiTexture`).
//! - [`Sampler`]: Texture filtering and address modes (`QRhiSampler`).
//! - [`SwapChain`]: Native surface presentation and vertical synchronization (`QRhiSwapChain`).
//! - [`CommandBuffer`]: Render pass and draw command recording (`QRhiCommandBuffer`).
//! - [`GraphicsPipeline`]: Fixed-function and programmable shader pipeline state (`QRhiGraphicsPipeline`).
//! - [`ShaderResourceBindings`]: Uniform buffer and texture descriptor sets (`QRhiShaderResourceBindings`).
//! - [`ResourceUpdateBatch`]: Batched asynchronous data transfers (`QRhiResourceUpdateBatch`).

pub mod backend;
pub mod buffer;
pub mod canonical;
pub mod command_buffer;
pub mod pipeline;
pub mod render_target;
pub mod rhi;
pub mod shader_resource;
pub mod swapchain;
pub mod texture;
pub mod types;

// Re-exports
pub use backend::{NullBackend, RhiBackend};
pub use buffer::{Buffer, BufferDescription, BufferType, BufferUsage};
pub use canonical::*;
pub use command_buffer::{CommandBuffer, DrawCommand, RenderPass};
pub use pipeline::{
    BlendFactor, BlendState, CullMode, FrontFace, GraphicsPipeline, GraphicsPipelineDescription,
    PrimitiveTopology, ShaderSource, ShaderStage, ShaderStageType, VertexFormat,
    VertexInputAttribute, VertexInputBinding, VertexInputLayout,
};
pub use render_target::{
    ColorAttachment, RenderTarget, SwapchainRenderTarget, TextureRenderTarget,
    TextureRenderTargetDescription,
};
pub use rhi::Rhi;
pub use shader_resource::{
    BufferUpload, ResourceUpdateBatch, ShaderResourceBinding, ShaderResourceBindings,
    StageVisibility, TextureUpload,
};
pub use swapchain::{PresentMode, SwapChain, SwapChainConfig};
pub use texture::{
    AddressMode, Sampler, SamplerDescription, SamplerFilter, Texture, TextureDescription,
    TextureFlags, TextureFormat, TextureType,
};
pub use types::{
    ColorClearValue, DepthStencilClearValue, FrameOpResult, RhiBackend as BackendType,
    RhiFeature, RhiFlags, Scissor, Viewport,
};
