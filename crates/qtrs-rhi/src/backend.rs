//! RHI backend trait and Null backend implementation matching `QRhiImplementation` / `QRhiNull`.

use std::collections::HashMap;

use crate::buffer::BufferDescription;
use crate::command_buffer::CommandBuffer;
use crate::pipeline::GraphicsPipelineDescription;
use crate::swapchain::SwapChain;
use crate::texture::{SamplerDescription, TextureDescription};
use crate::types::{FrameOpResult, RhiBackend as BackendType, RhiFeature};

/// Backend implementation interface matching Qt 6 `QRhiImplementation`.
pub trait RhiBackend {
    /// Returns the backend type (Vulkan, Metal, D3D12, Null, etc.).
    fn backend_type(&self) -> BackendType;

    /// Checks if a hardware feature is supported by this backend.
    fn is_feature_supported(&self, feature: RhiFeature) -> bool;

    /// Allocates and creates backend buffer resources.
    fn create_buffer(&mut self, desc: &BufferDescription) -> Result<u64, String>;

    /// Destroys backend buffer resources.
    fn destroy_buffer(&mut self, buffer_id: u64);

    /// Allocates and creates backend texture resources.
    fn create_texture(&mut self, desc: &TextureDescription) -> Result<u64, String>;

    /// Destroys backend texture resources.
    fn destroy_texture(&mut self, texture_id: u64);

    /// Allocates and creates backend sampler resources.
    fn create_sampler(&mut self, desc: &SamplerDescription) -> Result<u64, String>;

    /// Destroys backend sampler resources.
    fn destroy_sampler(&mut self, sampler_id: u64);

    /// Compiles and builds a graphics pipeline.
    fn create_pipeline(&mut self, desc: &GraphicsPipelineDescription) -> Result<u64, String>;

    /// Destroys a graphics pipeline.
    fn destroy_pipeline(&mut self, pipeline_id: u64);

    /// Begins a frame presentation cycle for a swapchain.
    fn begin_frame(&mut self, swapchain: &mut SwapChain) -> FrameOpResult;

    /// Ends and presents a frame cycle for a swapchain.
    fn end_frame(&mut self, swapchain: &mut SwapChain) -> FrameOpResult;

    /// Submits a recorded command buffer for execution.
    fn submit_command_buffer(&mut self, cmd: &CommandBuffer) -> Result<(), String>;

    /// Reads back texture pixel bytes to host memory.
    fn readback_texture(&mut self, texture_id: u64) -> Result<Vec<u8>, String>;
}

/// Headless Null backend matching `QRhiNull`.
///
/// Implements full RHI state tracking in memory without requiring a physical GPU driver,
/// making it ideal for CI, unit testing, and headless verification.
#[derive(Debug, Default)]
pub struct NullBackend {
    buffers: HashMap<u64, (BufferDescription, Vec<u8>)>,
    textures: HashMap<u64, (TextureDescription, Vec<u8>)>,
    samplers: HashMap<u64, SamplerDescription>,
    pipelines: HashMap<u64, GraphicsPipelineDescription>,
    next_id: u64,
    frame_count: u64,
}

impl NullBackend {
    pub fn new() -> Self {
        Self {
            buffers: HashMap::new(),
            textures: HashMap::new(),
            samplers: HashMap::new(),
            pipelines: HashMap::new(),
            next_id: 1,
            frame_count: 0,
        }
    }

    fn alloc_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }
}

impl RhiBackend for NullBackend {
    fn backend_type(&self) -> BackendType {
        BackendType::Null
    }

    fn is_feature_supported(&self, _feature: RhiFeature) -> bool {
        // The Null backend tracks resources and uploads, but does not execute
        // graphics or compute work or provide hardware feature guarantees.
        false
    }

    fn create_buffer(&mut self, desc: &BufferDescription) -> Result<u64, String> {
        let id = self.alloc_id();
        self.buffers.insert(id, (*desc, vec![0u8; desc.size]));
        Ok(id)
    }

    fn destroy_buffer(&mut self, buffer_id: u64) {
        self.buffers.remove(&buffer_id);
    }

    fn create_texture(&mut self, desc: &TextureDescription) -> Result<u64, String> {
        let id = self.alloc_id();
        let size = (desc.width * desc.height) as usize * desc.format.bytes_per_pixel();
        self.textures.insert(id, (*desc, vec![0u8; size]));
        Ok(id)
    }

    fn destroy_texture(&mut self, texture_id: u64) {
        self.textures.remove(&texture_id);
    }

    fn create_sampler(&mut self, desc: &SamplerDescription) -> Result<u64, String> {
        let id = self.alloc_id();
        self.samplers.insert(id, *desc);
        Ok(id)
    }

    fn destroy_sampler(&mut self, sampler_id: u64) {
        self.samplers.remove(&sampler_id);
    }

    fn create_pipeline(&mut self, desc: &GraphicsPipelineDescription) -> Result<u64, String> {
        let id = self.alloc_id();
        self.pipelines.insert(id, desc.clone());
        Ok(id)
    }

    fn destroy_pipeline(&mut self, pipeline_id: u64) {
        self.pipelines.remove(&pipeline_id);
    }

    fn begin_frame(&mut self, _swapchain: &mut SwapChain) -> FrameOpResult {
        FrameOpResult::Success
    }

    fn end_frame(&mut self, _swapchain: &mut SwapChain) -> FrameOpResult {
        self.frame_count += 1;
        FrameOpResult::Success
    }

    fn submit_command_buffer(&mut self, cmd: &CommandBuffer) -> Result<(), String> {
        for pass in cmd.passes() {
            // Apply resource uploads if present
            if let Some(batch) = &pass.resource_updates {
                for b_up in &batch.buffer_uploads {
                    if let Some((desc, storage)) = self.buffers.get_mut(&b_up.buffer_id) {
                        if b_up.offset + b_up.data.len() <= desc.size {
                            storage[b_up.offset..b_up.offset + b_up.data.len()]
                                .copy_from_slice(&b_up.data);
                        }
                    }
                }
                for t_up in &batch.texture_uploads {
                    if let Some((_desc, storage)) = self.textures.get_mut(&t_up.texture_id) {
                        if t_up.data.len() <= storage.len() {
                            storage[..t_up.data.len()].copy_from_slice(&t_up.data);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn readback_texture(&mut self, texture_id: u64) -> Result<Vec<u8>, String> {
        self.textures
            .get(&texture_id)
            .map(|(_, data)| data.clone())
            .ok_or_else(|| format!("Texture id {texture_id} not found"))
    }
}
