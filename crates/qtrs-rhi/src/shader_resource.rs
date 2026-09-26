//! Shader resource bindings (descriptors) and resource update batching matching Qt 6 QRhi.

use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_SRB_ID: AtomicU64 = AtomicU64::new(1);

/// Shader stage visibility for a resource binding matching `QRhiShaderResourceBinding::StageFlag`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StageVisibility {
    pub vertex: bool,
    pub fragment: bool,
    pub compute: bool,
}

impl StageVisibility {
    pub const fn vertex() -> Self {
        Self {
            vertex: true,
            fragment: false,
            compute: false,
        }
    }

    pub const fn fragment() -> Self {
        Self {
            vertex: false,
            fragment: true,
            compute: false,
        }
    }

    pub const fn all_graphics() -> Self {
        Self {
            vertex: true,
            fragment: true,
            compute: false,
        }
    }

    pub const fn compute() -> Self {
        Self {
            vertex: false,
            fragment: false,
            compute: true,
        }
    }
}

/// Binding resource payload matching `QRhiShaderResourceBinding`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindingResource {
    UniformBuffer {
        buffer_id: u64,
        offset: usize,
        size: usize,
    },
    SampledTexture {
        texture_id: u64,
        sampler_id: u64,
    },
    Texture {
        texture_id: u64,
    },
    Sampler {
        sampler_id: u64,
    },
    StorageBuffer {
        buffer_id: u64,
        offset: usize,
        size: usize,
    },
}

/// A single shader resource binding matching `QRhiShaderResourceBinding`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShaderResourceBinding {
    pub binding: u32,
    pub stage: StageVisibility,
    pub resource: BindingResource,
}

impl ShaderResourceBinding {
    pub fn uniform_buffer(binding: u32, stage: StageVisibility, buffer_id: u64, offset: usize, size: usize) -> Self {
        Self {
            binding,
            stage,
            resource: BindingResource::UniformBuffer {
                buffer_id,
                offset,
                size,
            },
        }
    }

    pub fn sampled_texture(binding: u32, stage: StageVisibility, texture_id: u64, sampler_id: u64) -> Self {
        Self {
            binding,
            stage,
            resource: BindingResource::SampledTexture {
                texture_id,
                sampler_id,
            },
        }
    }
}

/// Description of bound shader resources matching `QRhiShaderResourceBindings`.
#[derive(Debug)]
pub struct ShaderResourceBindings {
    id: u64,
    bindings: Vec<ShaderResourceBinding>,
}

impl ShaderResourceBindings {
    pub fn new(bindings: Vec<ShaderResourceBinding>) -> Self {
        Self {
            id: NEXT_SRB_ID.fetch_add(1, Ordering::Relaxed),
            bindings,
        }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn bindings(&self) -> &[ShaderResourceBinding] {
        &self.bindings
    }
}

/// Buffer write operation in a batch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BufferUpload {
    pub buffer_id: u64,
    pub offset: usize,
    pub data: Vec<u8>,
}

/// Texture write operation in a batch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextureUpload {
    pub texture_id: u64,
    pub level: u32,
    pub layer: u32,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

/// Resource upload and copy batch matching Qt 6 `QRhiResourceUpdateBatch`.
#[derive(Debug, Default, Clone)]
pub struct ResourceUpdateBatch {
    pub buffer_uploads: Vec<BufferUpload>,
    pub texture_uploads: Vec<TextureUpload>,
}

impl ResourceUpdateBatch {
    pub fn new() -> Self {
        Self::default()
    }

    /// Queues a buffer upload operation.
    pub fn upload_buffer(&mut self, buffer_id: u64, offset: usize, data: &[u8]) {
        self.buffer_uploads.push(BufferUpload {
            buffer_id,
            offset,
            data: data.to_vec(),
        });
    }

    /// Queues a 2D texture data upload operation.
    pub fn upload_texture_2d(
        &mut self,
        texture_id: u64,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        data: &[u8],
    ) {
        self.texture_uploads.push(TextureUpload {
            texture_id,
            level: 0,
            layer: 0,
            x,
            y,
            width,
            height,
            data: data.to_vec(),
        });
    }

    /// Returns true if the batch contains no pending operations.
    pub fn is_empty(&self) -> bool {
        self.buffer_uploads.is_empty() && self.texture_uploads.is_empty()
    }

    /// Clears all pending operations.
    pub fn clear(&mut self) {
        self.buffer_uploads.clear();
        self.texture_uploads.clear();
    }
}
