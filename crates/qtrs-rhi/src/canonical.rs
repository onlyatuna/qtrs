//! Canonical Qt 6 `QRhi` type aliases for seamless compatibility.

pub type QRhi = crate::rhi::Rhi;
pub type QRhiBuffer = crate::buffer::Buffer;
pub type QRhiTexture = crate::texture::Texture;
pub type QRhiSampler = crate::texture::Sampler;
pub type QRhiSwapChain = crate::swapchain::SwapChain;
pub type QRhiCommandBuffer = crate::command_buffer::CommandBuffer;
pub type QRhiGraphicsPipeline = crate::pipeline::GraphicsPipeline;
pub type QRhiShaderResourceBindings = crate::shader_resource::ShaderResourceBindings;
pub type QRhiResourceUpdateBatch = crate::shader_resource::ResourceUpdateBatch;
pub type QRhiTextureRenderTarget = crate::render_target::TextureRenderTarget;
pub type QRhiSwapChainRenderTarget = crate::render_target::SwapchainRenderTarget;
pub type QRhiViewport = crate::types::Viewport;
pub type QRhiScissor = crate::types::Scissor;
pub type QRhiDepthStencilClearValue = crate::types::DepthStencilClearValue;
