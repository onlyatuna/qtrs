//! Texture and Sampler resource abstractions matching `QRhiTexture` and `QRhiSampler`.

use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEXTURE_ID: AtomicU64 = AtomicU64::new(1);
static NEXT_SAMPLER_ID: AtomicU64 = AtomicU64::new(1);

/// Texture pixel formats matching `QRhiTexture::Format`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TextureFormat {
    #[default]
    Rgba8Unorm,
    Bgra8Unorm,
    R8Unorm,
    Rg8Unorm,
    R16Float,
    Rgba16Float,
    Rgba32Float,
    Depth16,
    Depth24Stencil8,
    Depth32Float,
}

impl TextureFormat {
    /// Returns the number of bytes per pixel for uncompressed formats.
    pub const fn bytes_per_pixel(&self) -> usize {
        match self {
            Self::R8Unorm => 1,
            Self::Rg8Unorm | Self::R16Float | Self::Depth16 => 2,
            Self::Rgba8Unorm | Self::Bgra8Unorm | Self::Depth24Stencil8 | Self::Depth32Float => 4,
            Self::Rgba16Float => 8,
            Self::Rgba32Float => 16,
        }
    }

    /// Returns whether this format represents a depth or stencil buffer.
    pub const fn is_depth_stencil(&self) -> bool {
        matches!(
            self,
            Self::Depth16 | Self::Depth24Stencil8 | Self::Depth32Float
        )
    }
}

/// Texture dimension type matching `QRhiTexture::Type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TextureType {
    #[default]
    Texture2D,
    TextureCube,
    Texture3D,
    TextureArray,
}

/// Texture usage flags matching `QRhiTexture::Flags`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TextureFlags {
    pub render_target: bool,
    pub mip_mapped: bool,
    pub used_with_load_store: bool,
    pub used_as_transfer_source: bool,
}

impl TextureFlags {
    pub const fn none() -> Self {
        Self {
            render_target: false,
            mip_mapped: false,
            used_with_load_store: false,
            used_as_transfer_source: false,
        }
    }

    pub const fn render_target() -> Self {
        Self {
            render_target: true,
            mip_mapped: false,
            used_with_load_store: false,
            used_as_transfer_source: false,
        }
    }
}

/// Description for creating a Texture resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextureDescription {
    pub texture_type: TextureType,
    pub format: TextureFormat,
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub array_size: u32,
    pub sample_count: u32,
    pub flags: TextureFlags,
}

impl TextureDescription {
    /// Creates a 2D texture description.
    pub const fn new_2d(width: u32, height: u32, format: TextureFormat, flags: TextureFlags) -> Self {
        Self {
            texture_type: TextureType::Texture2D,
            format,
            width,
            height,
            depth: 1,
            array_size: 1,
            sample_count: 1,
            flags,
        }
    }

    /// Creates a 2D color render target texture description.
    pub const fn render_target_2d(width: u32, height: u32, format: TextureFormat) -> Self {
        Self::new_2d(width, height, format, TextureFlags::render_target())
    }
}

/// Texture resource matching `QRhiTexture`.
#[derive(Debug)]
pub struct Texture {
    id: u64,
    desc: TextureDescription,
}

impl Texture {
    pub fn new(desc: TextureDescription) -> Self {
        Self {
            id: NEXT_TEXTURE_ID.fetch_add(1, Ordering::Relaxed),
            desc,
        }
    }
    /// Creates a texture with explicit backend-assigned id.
    pub fn with_id(id: u64, desc: TextureDescription) -> Self {
        Self { id, desc }
    }


    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn description(&self) -> &TextureDescription {
        &self.desc
    }

    pub fn width(&self) -> u32 {
        self.desc.width
    }

    pub fn height(&self) -> u32 {
        self.desc.height
    }

    pub fn format(&self) -> TextureFormat {
        self.desc.format
    }
}

/// Texture filtering modes matching `QRhiSampler::Filter`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SamplerFilter {
    Nearest,
    #[default]
    Linear,
}

/// Texture addressing modes matching `QRhiSampler::AddressMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AddressMode {
    #[default]
    Repeat,
    ClampToEdge,
    MirroredRepeat,
}

/// Sampler description matching `QRhiSampler`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SamplerDescription {
    pub mag_filter: SamplerFilter,
    pub min_filter: SamplerFilter,
    pub mipmap_mode: SamplerFilter,
    pub address_u: AddressMode,
    pub address_v: AddressMode,
    pub address_w: AddressMode,
}

impl Default for SamplerDescription {
    fn default() -> Self {
        Self {
            mag_filter: SamplerFilter::Linear,
            min_filter: SamplerFilter::Linear,
            mipmap_mode: SamplerFilter::Linear,
            address_u: AddressMode::ClampToEdge,
            address_v: AddressMode::ClampToEdge,
            address_w: AddressMode::ClampToEdge,
        }
    }
}

/// Texture sampler resource matching `QRhiSampler`.
#[derive(Debug)]
pub struct Sampler {
    id: u64,
    desc: SamplerDescription,
}

impl Sampler {
    pub fn new(desc: SamplerDescription) -> Self {
        Self {
            id: NEXT_SAMPLER_ID.fetch_add(1, Ordering::Relaxed),
            desc,
        }
    }
    /// Creates a sampler with explicit backend-assigned id.
    pub fn with_id(id: u64, desc: SamplerDescription) -> Self {
        Self { id, desc }
    }


    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn description(&self) -> &SamplerDescription {
        &self.desc
    }
}
