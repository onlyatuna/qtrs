//! Command buffer and pass recording abstractions matching `QRhiCommandBuffer`.

use crate::shader_resource::ResourceUpdateBatch;
use crate::types::{ColorClearValue, DepthStencilClearValue, Scissor, Viewport};

/// Recorded drawing command in a pass.
#[derive(Debug, Clone, PartialEq)]
pub enum DrawCommand {
    SetGraphicsPipeline {
        pipeline_id: u64,
    },
    SetShaderResources {
        srb_id: u64,
    },
    SetVertexBuffer {
        binding: u32,
        buffer_id: u64,
        offset: usize,
    },
    SetIndexBuffer {
        buffer_id: u64,
        offset: usize,
        is_u32: bool,
    },
    SetViewport {
        viewport: Viewport,
    },
    SetScissor {
        scissor: Scissor,
    },
    Draw {
        vertex_count: u32,
        instance_count: u32,
        first_vertex: u32,
        first_instance: u32,
    },
    DrawIndexed {
        index_count: u32,
        instance_count: u32,
        first_index: u32,
        vertex_offset: i32,
        first_instance: u32,
    },
}

/// Recorded render pass in a command buffer.
#[derive(Debug, Clone)]
pub struct RenderPass {
    pub render_target_id: u64,
    pub clear_color: Option<ColorClearValue>,
    pub clear_depth_stencil: Option<DepthStencilClearValue>,
    pub resource_updates: Option<ResourceUpdateBatch>,
    pub commands: Vec<DrawCommand>,
}

/// Command buffer recording render passes and execution matching `QRhiCommandBuffer`.
#[derive(Debug, Default)]
pub struct CommandBuffer {
    passes: Vec<RenderPass>,
    current_pass: Option<RenderPass>,
}

impl CommandBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Begins a new render pass targeting the given render target.
    pub fn begin_pass(
        &mut self,
        render_target_id: u64,
        clear_color: Option<ColorClearValue>,
        clear_depth_stencil: Option<DepthStencilClearValue>,
        resource_updates: Option<ResourceUpdateBatch>,
    ) {
        assert!(self.current_pass.is_none(), "Pass already in progress");
        self.current_pass = Some(RenderPass {
            render_target_id,
            clear_color,
            clear_depth_stencil,
            resource_updates,
            commands: Vec::new(),
        });
    }

    /// Ends the current render pass.
    pub fn end_pass(&mut self) {
        if let Some(pass) = self.current_pass.take() {
            self.passes.push(pass);
        }
    }

    /// Sets the current active graphics pipeline.
    pub fn set_graphics_pipeline(&mut self, pipeline_id: u64) {
        if let Some(pass) = self.current_pass.as_mut() {
            pass.commands.push(DrawCommand::SetGraphicsPipeline { pipeline_id });
        }
    }

    /// Sets the active shader resource bindings.
    pub fn set_shader_resources(&mut self, srb_id: u64) {
        if let Some(pass) = self.current_pass.as_mut() {
            pass.commands.push(DrawCommand::SetShaderResources { srb_id });
        }
    }

    /// Binds a vertex buffer to a binding slot.
    pub fn set_vertex_buffer(&mut self, binding: u32, buffer_id: u64, offset: usize) {
        if let Some(pass) = self.current_pass.as_mut() {
            pass.commands.push(DrawCommand::SetVertexBuffer {
                binding,
                buffer_id,
                offset,
            });
        }
    }

    /// Binds an index buffer.
    pub fn set_index_buffer(&mut self, buffer_id: u64, offset: usize, is_u32: bool) {
        if let Some(pass) = self.current_pass.as_mut() {
            pass.commands.push(DrawCommand::SetIndexBuffer {
                buffer_id,
                offset,
                is_u32,
            });
        }
    }

    /// Sets dynamic viewport.
    pub fn set_viewport(&mut self, viewport: Viewport) {
        if let Some(pass) = self.current_pass.as_mut() {
            pass.commands.push(DrawCommand::SetViewport { viewport });
        }
    }

    /// Sets dynamic scissor.
    pub fn set_scissor(&mut self, scissor: Scissor) {
        if let Some(pass) = self.current_pass.as_mut() {
            pass.commands.push(DrawCommand::SetScissor { scissor });
        }
    }

    /// Records a non-indexed draw command.
    pub fn draw(&mut self, vertex_count: u32, instance_count: u32, first_vertex: u32, first_instance: u32) {
        if let Some(pass) = self.current_pass.as_mut() {
            pass.commands.push(DrawCommand::Draw {
                vertex_count,
                instance_count,
                first_vertex,
                first_instance,
            });
        }
    }

    /// Records an indexed draw command.
    pub fn draw_indexed(
        &mut self,
        index_count: u32,
        instance_count: u32,
        first_index: u32,
        vertex_offset: i32,
        first_instance: u32,
    ) {
        if let Some(pass) = self.current_pass.as_mut() {
            pass.commands.push(DrawCommand::DrawIndexed {
                index_count,
                instance_count,
                first_index,
                vertex_offset,
                first_instance,
            });
        }
    }

    /// Returns recorded passes.
    pub fn passes(&self) -> &[RenderPass] {
        &self.passes
    }

    /// Clears recorded passes for reuse.
    pub fn reset(&mut self) {
        self.passes.clear();
        self.current_pass = None;
    }
}
