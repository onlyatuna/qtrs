//! Core geometric, configuration, and result types matching Qt 6 QRhi.

/// Supported RHI backends matching `QRhi::Implementation`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum RhiBackend {
    #[default]
    Null,
    Vulkan,
    Metal,
    D3D11,
    D3D12,
    OpenGLES2,
}

impl RhiBackend {
    /// Returns the backend's name as a string slice matching Qt `QRhi::backendName`.
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Null => "Null",
            Self::Vulkan => "Vulkan",
            Self::Metal => "Metal",
            Self::D3D11 => "Direct3D 11",
            Self::D3D12 => "Direct3D 12",
            Self::OpenGLES2 => "OpenGL ES 2.0",
        }
    }
}

/// Flags controlling RHI creation and behavior matching `QRhi::Flags`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RhiFlags {
    pub enable_debug_markers: bool,
    pub prefer_software_renderer: bool,
    pub enable_pipeline_cache: bool,
    pub enable_timestamps: bool,
}

/// Result of frame operations matching `QRhi::FrameOpResult`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameOpResult {
    Success = 0,
    Error,
    SwapChainOutOfDate,
    DeviceLost,
}

/// Viewport description matching Qt's `QRhiViewport`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub min_depth: f32,
    pub max_depth: f32,
}

impl Viewport {
    /// Creates a new Viewport with default depth range `[0.0, 1.0]`.
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
            min_depth: 0.0,
            max_depth: 1.0,
        }
    }

    /// Creates a new Viewport with explicit depth range.
    pub const fn with_depth(
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        min_depth: f32,
        max_depth: f32,
    ) -> Self {
        Self {
            x,
            y,
            width,
            height,
            min_depth,
            max_depth,
        }
    }
}

impl Default for Viewport {
    fn default() -> Self {
        Self::new(0.0, 0.0, 0.0, 0.0)
    }
}

/// Scissor rectangle matching Qt's `QRhiScissor`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Scissor {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Scissor {
    pub const fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

/// Clear value for depth-stencil attachments matching `QRhiDepthStencilClearValue`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DepthStencilClearValue {
    pub depth: f32,
    pub stencil: u32,
}

impl Default for DepthStencilClearValue {
    fn default() -> Self {
        Self {
            depth: 1.0,
            stencil: 0,
        }
    }
}

impl DepthStencilClearValue {
    pub const fn new(depth: f32, stencil: u32) -> Self {
        Self { depth, stencil }
    }
}

/// Clear value for color attachments (RGBA f32).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColorClearValue {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl ColorClearValue {
    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    pub const fn transparent() -> Self {
        Self::new(0.0, 0.0, 0.0, 0.0)
    }

    pub const fn black() -> Self {
        Self::new(0.0, 0.0, 0.0, 1.0)
    }

    pub const fn white() -> Self {
        Self::new(1.0, 1.0, 1.0, 1.0)
    }
}

impl Default for ColorClearValue {
    fn default() -> Self {
        Self::transparent()
    }
}

/// Hardware features supported by an RHI implementation matching `QRhi::Feature`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RhiFeature {
    MultisampleTexture,
    MultisampleRenderBuffer,
    DebugMarkers,
    Timestamps,
    Instancing,
    CustomInstanceStepRate,
    PrimitiveRestart,
    NonDynamicUniformBuffers,
    Compute,
    WideLines,
    ReadBackAnyTextureFormat,
    ThreeDimensionalTextures,
    TextureArrays,
    Tessellation,
    GeometryShader,
}
