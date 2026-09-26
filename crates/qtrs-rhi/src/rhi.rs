//! Main RHI device interface matching Qt 6 `QRhi`.

use crate::backend::{NullBackend, RhiBackend};
use crate::buffer::{Buffer, BufferDescription};
use crate::command_buffer::CommandBuffer;
use crate::pipeline::{GraphicsPipeline, GraphicsPipelineDescription};
use crate::render_target::{TextureRenderTarget, TextureRenderTargetDescription};
use crate::shader_resource::ResourceUpdateBatch;
use crate::swapchain::{SwapChain, SwapChainConfig};
use crate::texture::{Sampler, SamplerDescription, Texture, TextureDescription};
use crate::types::{FrameOpResult, RhiBackend as BackendType, RhiFlags};

/// Rendering Hardware Interface (RHI) device matching Qt 6 `QRhi`.
pub struct Rhi {
    backend: Box<dyn RhiBackend>,
    flags: RhiFlags,
    is_recording: bool,
}

impl Rhi {
    /// Creates a new RHI device with the specified backend and flags.
    ///
    /// Corresponds to `QRhi::create(impl, params, flags)`.
    pub fn create(backend_type: BackendType, flags: RhiFlags) -> Result<Self, String> {
        let backend: Box<dyn RhiBackend> = match backend_type {
            BackendType::Null => Box::new(NullBackend::new()),
            unsupported => {
                return Err(format!(
                    "{} backend is unavailable: only the Null backend is implemented",
                    unsupported.name()
                ));
            }
        };

        Ok(Self {
            backend,
            flags,
            is_recording: false,
        })
    }

    /// Creates a null/headless RHI device (ideal for testing and CI).
    pub fn new_null() -> Self {
        Self::create(BackendType::Null, RhiFlags::default()).expect("Null backend creation")
    }

    /// Returns the active backend type.
    pub fn backend_type(&self) -> BackendType {
        self.backend.backend_type()
    }

    /// Returns the backend's readable name.
    pub fn backend_name(&self) -> &'static str {
        self.backend.backend_type().name()
    }

    /// Returns whether the active backend implements the requested feature.
    pub fn is_feature_supported(&self, feature: crate::types::RhiFeature) -> bool {
        self.backend.is_feature_supported(feature)
    }

    /// Returns the active RHI configuration flags.
    pub fn flags(&self) -> &RhiFlags {
        &self.flags
    }

    /// Creates a new buffer matching `QRhi::newBuffer`.
    pub fn new_buffer(&mut self, desc: BufferDescription) -> Result<Buffer, String> {
        let id = self.backend.create_buffer(&desc)?;
        Ok(Buffer::with_id(id, desc))
    }

    /// Creates a new texture matching `QRhi::newTexture`.
    pub fn new_texture(&mut self, desc: TextureDescription) -> Result<Texture, String> {
        let id = self.backend.create_texture(&desc)?;
        Ok(Texture::with_id(id, desc))
    }

    /// Creates a new sampler matching `QRhi::newSampler`.
    pub fn new_sampler(&mut self, desc: SamplerDescription) -> Result<Sampler, String> {
        let id = self.backend.create_sampler(&desc)?;
        Ok(Sampler::with_id(id, desc))
    }

    /// Creates a new graphics pipeline matching `QRhi::newGraphicsPipeline`.
    pub fn new_graphics_pipeline(&mut self, desc: GraphicsPipelineDescription) -> Result<GraphicsPipeline, String> {
        let id = self.backend.create_pipeline(&desc)?;
        Ok(GraphicsPipeline::with_id(id, desc))
    }

    /// Creates a new swapchain matching `QRhi::newSwapChain`.
    pub fn new_swapchain(
        &mut self,
        surface_handle: Option<usize>,
        width: u32,
        height: u32,
        config: SwapChainConfig,
    ) -> SwapChain {
        SwapChain::new(surface_handle, width, height, config)
    }

    /// Creates an offscreen texture render target matching `QRhi::newTextureRenderTarget`.
    pub fn new_texture_render_target(&self, desc: TextureRenderTargetDescription) -> TextureRenderTarget {
        TextureRenderTarget::new(desc)
    }

    /// Allocates an empty resource update batch matching `QRhi::nextResourceUpdateBatch`.
    pub fn next_resource_update_batch(&self) -> ResourceUpdateBatch {
        ResourceUpdateBatch::new()
    }

    /// Begins a frame presentation cycle on the given swapchain matching `QRhi::beginFrame`.
    pub fn begin_frame(&mut self, swapchain: &mut SwapChain) -> FrameOpResult {
        if self.is_recording {
            return FrameOpResult::Error;
        }
        let res = self.backend.begin_frame(swapchain);
        if res == FrameOpResult::Success {
            self.is_recording = true;
        }
        res
    }

    /// Ends and presents a frame cycle matching `QRhi::endFrame`.
    pub fn end_frame(&mut self, swapchain: &mut SwapChain) -> FrameOpResult {
        if !self.is_recording {
            return FrameOpResult::Error;
        }
        let res = self.backend.end_frame(swapchain);
        self.is_recording = false;
        res
    }

    /// Submits a command buffer to the backend for execution.
    pub fn submit(&mut self, cmd: &CommandBuffer) -> Result<(), String> {
        self.backend.submit_command_buffer(cmd)
    }

    /// Reads back texture pixel bytes into host memory.
    pub fn readback_texture(&mut self, texture: &Texture) -> Result<Vec<u8>, String> {
        self.backend.readback_texture(texture.id())
    }

    /// Returns whether the RHI is currently recording a frame.
    pub fn is_recording_frame(&self) -> bool {
        self.is_recording
    }
}
