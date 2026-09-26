//! Graphics pipeline and shader state abstractions matching `QRhiGraphicsPipeline`.

use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_PIPELINE_ID: AtomicU64 = AtomicU64::new(1);

/// Vertex attribute data format matching `QRhiVertexInputAttribute::Format`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum VertexFormat {
    #[default]
    Float4,
    Float3,
    Float2,
    Float,
    UNormByte4,
    UNormByte2,
    UInt4,
    UInt2,
    UInt,
    SInt4,
    SInt2,
    SInt,
}

impl VertexFormat {
    pub const fn size_in_bytes(&self) -> usize {
        match self {
            Self::Float4 | Self::UInt4 | Self::SInt4 => 16,
            Self::Float3 => 12,
            Self::Float2 | Self::UInt2 | Self::SInt2 => 8,
            Self::Float | Self::UInt | Self::SInt | Self::UNormByte4 => 4,
            Self::UNormByte2 => 2,
        }
    }
}

/// Description of a single vertex attribute matching `QRhiVertexInputAttribute`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VertexInputAttribute {
    pub location: u32,
    pub binding: u32,
    pub format: VertexFormat,
    pub offset: u32,
}

impl VertexInputAttribute {
    pub const fn new(location: u32, binding: u32, format: VertexFormat, offset: u32) -> Self {
        Self {
            location,
            binding,
            format,
            offset,
        }
    }
}

/// Description of a vertex buffer binding matching `QRhiVertexInputBinding`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VertexInputBinding {
    pub stride: u32,
    pub per_instance: bool,
}

impl VertexInputBinding {
    pub const fn per_vertex(stride: u32) -> Self {
        Self {
            stride,
            per_instance: false,
        }
    }

    pub const fn per_instance(stride: u32) -> Self {
        Self {
            stride,
            per_instance: true,
        }
    }
}

/// Vertex input layout matching `QRhiVertexInputLayout`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VertexInputLayout {
    pub bindings: Vec<VertexInputBinding>,
    pub attributes: Vec<VertexInputAttribute>,
}

impl VertexInputLayout {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_binding(mut self, binding: VertexInputBinding) -> Self {
        self.bindings.push(binding);
        self
    }

    pub fn with_attribute(mut self, attribute: VertexInputAttribute) -> Self {
        self.attributes.push(attribute);
        self
    }
}

/// Shader stage type matching `QRhiShaderStage::Type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShaderStageType {
    Vertex,
    Fragment,
    Compute,
}

/// Portable shader code representation supporting SPIR-V, WGSL, GLSL, HLSL, and MSL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShaderSource {
    SpirV(Vec<u32>),
    Wgsl(String),
    Glsl(String),
    Hlsl(String),
    Msl(String),
}

/// Programmable shader stage matching `QRhiShaderStage`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShaderStage {
    pub stage: ShaderStageType,
    pub source: ShaderSource,
    pub entry_point: String,
}

impl ShaderStage {
    pub fn vertex(source: ShaderSource, entry_point: impl Into<String>) -> Self {
        Self {
            stage: ShaderStageType::Vertex,
            source,
            entry_point: entry_point.into(),
        }
    }

    pub fn fragment(source: ShaderSource, entry_point: impl Into<String>) -> Self {
        Self {
            stage: ShaderStageType::Fragment,
            source,
            entry_point: entry_point.into(),
        }
    }

    pub fn compute(source: ShaderSource, entry_point: impl Into<String>) -> Self {
        Self {
            stage: ShaderStageType::Compute,
            source,
            entry_point: entry_point.into(),
        }
    }
}

/// Primitive topology matching `QRhiGraphicsPipeline::Topology`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PrimitiveTopology {
    #[default]
    TriangleList,
    TriangleStrip,
    LineList,
    LineStrip,
    PointList,
}

/// Cull mode matching `QRhiGraphicsPipeline::CullMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CullMode {
    #[default]
    None,
    Front,
    Back,
}

/// Front face winding order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FrontFace {
    #[default]
    Ccw,
    Cw,
}

/// Blend factor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BlendFactor {
    #[default]
    One,
    Zero,
    SrcAlpha,
    OneMinusSrcAlpha,
    DstAlpha,
    OneMinusDstAlpha,
}

/// Color blend state for alpha blending.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlendState {
    pub enabled: bool,
    pub src_color: BlendFactor,
    pub dst_color: BlendFactor,
    pub src_alpha: BlendFactor,
    pub dst_alpha: BlendFactor,
}

impl Default for BlendState {
    fn default() -> Self {
        Self::premultiplied_alpha()
    }
}

impl BlendState {
    pub const fn disabled() -> Self {
        Self {
            enabled: false,
            src_color: BlendFactor::One,
            dst_color: BlendFactor::Zero,
            src_alpha: BlendFactor::One,
            dst_alpha: BlendFactor::Zero,
        }
    }

    pub const fn premultiplied_alpha() -> Self {
        Self {
            enabled: true,
            src_color: BlendFactor::One,
            dst_color: BlendFactor::OneMinusSrcAlpha,
            src_alpha: BlendFactor::One,
            dst_alpha: BlendFactor::OneMinusSrcAlpha,
        }
    }

    pub const fn standard_alpha() -> Self {
        Self {
            enabled: true,
            src_color: BlendFactor::SrcAlpha,
            dst_color: BlendFactor::OneMinusSrcAlpha,
            src_alpha: BlendFactor::One,
            dst_alpha: BlendFactor::OneMinusSrcAlpha,
        }
    }
}

/// Full description of a graphics pipeline.
#[derive(Debug, Clone, PartialEq)]
pub struct GraphicsPipelineDescription {
    pub shader_stages: Vec<ShaderStage>,
    pub vertex_input_layout: VertexInputLayout,
    pub topology: PrimitiveTopology,
    pub cull_mode: CullMode,
    pub front_face: FrontFace,
    pub blend_state: BlendState,
    pub sample_count: u32,
    pub shader_resource_bindings_id: Option<u64>,
}

impl Default for GraphicsPipelineDescription {
    fn default() -> Self {
        Self {
            shader_stages: Vec::new(),
            vertex_input_layout: VertexInputLayout::default(),
            topology: PrimitiveTopology::TriangleList,
            cull_mode: CullMode::None,
            front_face: FrontFace::Ccw,
            blend_state: BlendState::default(),
            sample_count: 1,
            shader_resource_bindings_id: None,
        }
    }
}

/// Graphics pipeline resource matching `QRhiGraphicsPipeline`.
#[derive(Debug)]
pub struct GraphicsPipeline {
    id: u64,
    desc: GraphicsPipelineDescription,
}

impl GraphicsPipeline {
    pub fn new(desc: GraphicsPipelineDescription) -> Self {
        Self {
            id: NEXT_PIPELINE_ID.fetch_add(1, Ordering::Relaxed),
            desc,
        }
    }
    /// Creates a graphics pipeline with explicit backend-assigned id.
    pub fn with_id(id: u64, desc: GraphicsPipelineDescription) -> Self {
        Self { id, desc }
    }


    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn description(&self) -> &GraphicsPipelineDescription {
        &self.desc
    }
}
