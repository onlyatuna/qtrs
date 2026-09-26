//! Comprehensive integration test suite for `qtrs-rhi`.
//!
//! Validates:
//! 1. `Rhi` device lifecycle and backend queries (`Null` backend).
//! 2. `Buffer` allocations (Vertex, Index, Uniform) and usage flags.
//! 3. `Texture` and `Sampler` resource creation and format utilities.
//! 4. `SwapChain` configuration, window resizing, and `SwapchainRenderTarget`.
//! 5. `TextureRenderTarget` offscreen target creation and descriptions.
//! 6. `GraphicsPipeline` creation with vertex layout and blend state.
//! 7. `ResourceUpdateBatch` queuing and buffer/texture data transfers.
//! 8. `CommandBuffer` pass recording and draw commands.
//! 9. Frame lifecycle: `begin_frame` -> `submit` -> `end_frame`.
//! 10. Texture readback verification.
//! 11. Qt 6 canonical type aliases (`QRhi`, `QRhiBuffer`, `QRhiTexture`, etc.).

use qtrs_rhi::*;

// =============================================================================
// 1. Device Lifecycle Tests
// =============================================================================

#[test]
fn test_rhi_device_creation() {
    let rhi = Rhi::new_null();
    assert_eq!(rhi.backend_type(), BackendType::Null);
    assert_eq!(rhi.backend_name(), "Null");
    assert!(!rhi.is_recording_frame());
}

#[test]
fn test_rhi_unavailable_backends_fail_explicitly() {
    for backend in [
        BackendType::Vulkan,
        BackendType::Metal,
        BackendType::D3D11,
        BackendType::D3D12,
        BackendType::OpenGLES2,
    ] {
        let error = match Rhi::create(backend, RhiFlags::default()) {
            Err(error) => error,
            Ok(_) => panic!("unavailable backend {backend:?} unexpectedly succeeded"),
        };
        assert!(error.contains(backend.name()));
        assert!(error.contains("unavailable"));
    }
}

#[test]
fn test_null_backend_reports_only_implemented_features() {
    let rhi = Rhi::new_null();
    for feature in [
        RhiFeature::MultisampleTexture,
        RhiFeature::MultisampleRenderBuffer,
        RhiFeature::DebugMarkers,
        RhiFeature::Timestamps,
        RhiFeature::Instancing,
        RhiFeature::CustomInstanceStepRate,
        RhiFeature::PrimitiveRestart,
        RhiFeature::NonDynamicUniformBuffers,
        RhiFeature::Compute,
        RhiFeature::WideLines,
        RhiFeature::ReadBackAnyTextureFormat,
        RhiFeature::ThreeDimensionalTextures,
        RhiFeature::TextureArrays,
        RhiFeature::Tessellation,
        RhiFeature::GeometryShader,
    ] {
        assert!(
            !rhi.is_feature_supported(feature),
            "Null backend must not claim {feature:?}"
        );
    }
}

// =============================================================================
// 2. Buffer Resource Tests
// =============================================================================

#[test]
fn test_rhi_buffers() {
    let mut rhi = Rhi::new_null();

    // Vertex buffer
    let vbuf = rhi
        .new_buffer(BufferDescription::vertex(1024))
        .expect("vertex buffer");
    assert_eq!(vbuf.size(), 1024);
    assert_eq!(vbuf.buffer_type(), BufferType::Static);
    assert!(vbuf.usage().vertex);
    assert!(!vbuf.usage().index);

    // Index buffer
    let ibuf = rhi
        .new_buffer(BufferDescription::index(512))
        .expect("index buffer");
    assert_eq!(ibuf.size(), 512);
    assert!(ibuf.usage().index);

    // Uniform buffer (Dynamic)
    let ubuf = rhi
        .new_buffer(BufferDescription::uniform(256))
        .expect("uniform buffer");
    assert_eq!(ubuf.size(), 256);
    assert_eq!(ubuf.buffer_type(), BufferType::Dynamic);
    assert!(ubuf.usage().uniform);
}

// =============================================================================
// 3. Texture and Sampler Tests
// =============================================================================

#[test]
fn test_rhi_texture_and_sampler() {
    let mut rhi = Rhi::new_null();

    // 2D RGBA8 texture
    let tex_desc = TextureDescription::render_target_2d(800, 600, TextureFormat::Rgba8Unorm);
    let tex = rhi.new_texture(tex_desc).expect("texture creation");
    assert_eq!(tex.width(), 800);
    assert_eq!(tex.height(), 600);
    assert_eq!(tex.format(), TextureFormat::Rgba8Unorm);
    assert_eq!(tex.format().bytes_per_pixel(), 4);
    assert!(!tex.format().is_depth_stencil());

    // Depth format
    let depth_format = TextureFormat::Depth24Stencil8;
    assert!(depth_format.is_depth_stencil());

    // Sampler
    let sampler_desc = SamplerDescription::default();
    let sampler = rhi.new_sampler(sampler_desc).expect("sampler creation");
    assert_eq!(sampler.description().mag_filter, SamplerFilter::Linear);
    assert_eq!(sampler.description().address_u, AddressMode::ClampToEdge);
}

// =============================================================================
// 4. SwapChain and RenderTarget Tests
// =============================================================================

#[test]
fn test_rhi_swapchain_and_resize() {
    let mut rhi = Rhi::new_null();

    let mut sc = rhi.new_swapchain(Some(0x1234), 1920, 1080, SwapChainConfig::default());
    assert_eq!(sc.width(), 1920);
    assert_eq!(sc.height(), 1080);
    assert_eq!(sc.surface_handle(), Some(0x1234));
    assert_eq!(sc.render_target().width(), 1920);
    assert_eq!(sc.render_target().height(), 1080);

    // Resize
    sc.resize(2560, 1440);
    assert_eq!(sc.width(), 2560);
    assert_eq!(sc.height(), 1440);
    assert_eq!(sc.render_target().width(), 2560);
    assert_eq!(sc.render_target().height(), 1440);
}

#[test]
fn test_rhi_texture_render_target() {
    let rhi = Rhi::new_null();

    let color_att = ColorAttachment::new(1);
    let desc = TextureRenderTargetDescription::new(color_att, 1024, 768);
    let trt = rhi.new_texture_render_target(desc);

    assert_eq!(trt.width(), 1024);
    assert_eq!(trt.height(), 768);
    assert_eq!(trt.description().color_attachments.len(), 1);
}

// =============================================================================
// 5. Pipeline Creation Tests
// =============================================================================

#[test]
fn test_rhi_graphics_pipeline() {
    let mut rhi = Rhi::new_null();

    let layout = VertexInputLayout::new()
        .with_binding(VertexInputBinding::per_vertex(16))
        .with_attribute(VertexInputAttribute::new(0, 0, VertexFormat::Float2, 0))
        .with_attribute(VertexInputAttribute::new(1, 0, VertexFormat::Float2, 8));

    let vs = ShaderStage::vertex(ShaderSource::Wgsl("// vs".to_string()), "main");
    let fs = ShaderStage::fragment(ShaderSource::Wgsl("// fs".to_string()), "main");

    let desc = GraphicsPipelineDescription {
        shader_stages: vec![vs, fs],
        vertex_input_layout: layout,
        topology: PrimitiveTopology::TriangleList,
        cull_mode: CullMode::None,
        front_face: FrontFace::Ccw,
        blend_state: BlendState::standard_alpha(),
        sample_count: 1,
        shader_resource_bindings_id: None,
    };

    let pipeline = rhi.new_graphics_pipeline(desc).expect("pipeline");
    assert_eq!(pipeline.description().topology, PrimitiveTopology::TriangleList);
    assert!(pipeline.description().blend_state.enabled);
}

// =============================================================================
// 6. Command Recording and Frame Lifecycle Tests
// =============================================================================

#[test]
fn test_rhi_frame_lifecycle_and_commands() {
    let mut rhi = Rhi::new_null();
    let mut sc = rhi.new_swapchain(None, 800, 600, SwapChainConfig::default());

    // Begin frame
    let res = rhi.begin_frame(&mut sc);
    assert_eq!(res, FrameOpResult::Success);
    assert!(rhi.is_recording_frame());

    // Duplicate begin_frame fails
    let err_res = rhi.begin_frame(&mut sc);
    assert_eq!(err_res, FrameOpResult::Error);

    // Record commands in CommandBuffer
    let mut cmd = CommandBuffer::new();
    let updates = rhi.next_resource_update_batch();

    cmd.begin_pass(
        sc.render_target().id(),
        Some(ColorClearValue::black()),
        Some(DepthStencilClearValue::default()),
        Some(updates),
    );

    cmd.set_viewport(Viewport::new(0.0, 0.0, 800.0, 600.0));
    cmd.set_scissor(Scissor::new(0, 0, 800, 600));
    cmd.set_graphics_pipeline(100);
    cmd.set_vertex_buffer(0, 200, 0);
    cmd.set_index_buffer(300, 0, false);
    cmd.draw_indexed(6, 1, 0, 0, 0);
    cmd.end_pass();

    assert_eq!(cmd.passes().len(), 1);
    assert_eq!(cmd.passes()[0].commands.len(), 6);

    // Submit commands
    rhi.submit(&cmd).expect("submit");

    // End frame
    let end_res = rhi.end_frame(&mut sc);
    assert_eq!(end_res, FrameOpResult::Success);
    assert!(!rhi.is_recording_frame());
}

// =============================================================================
// 7. Resource Upload Batch and Texture Readback Tests
// =============================================================================

#[test]
fn test_rhi_resource_batch_and_readback() {
    let mut rhi = Rhi::new_null();

    // Create a 2x2 RGBA8 texture (16 bytes)
    let tex = rhi
        .new_texture(TextureDescription::new_2d(
            2,
            2,
            TextureFormat::Rgba8Unorm,
            TextureFlags::none(),
        ))
        .expect("tex");

    let pixels: [u8; 16] = [
        255, 0, 0, 255, // Red
        0, 255, 0, 255, // Green
        0, 0, 255, 255, // Blue
        255, 255, 0, 255, // Yellow
    ];

    let mut batch = rhi.next_resource_update_batch();
    batch.upload_texture_2d(tex.id(), 0, 0, 2, 2, &pixels);

    let mut cmd = CommandBuffer::new();
    cmd.begin_pass(1, None, None, Some(batch));
    cmd.end_pass();

    rhi.submit(&cmd).expect("submit batch");

    // Read back texture and verify uploaded pixel data
    let readback = rhi.readback_texture(&tex).expect("readback");
    assert_eq!(readback.len(), 16);
    assert_eq!(&readback[..], &pixels[..]);
}

// =============================================================================
// 8. Canonical Qt 6 Type Aliases Tests
// =============================================================================

#[test]
fn test_canonical_qt_type_aliases() {
    let mut rhi: QRhi = QRhi::new_null();
    let _buf: QRhiBuffer = rhi
        .new_buffer(BufferDescription::vertex(64))
        .expect("buffer");
    let sc: QRhiSwapChain = rhi.new_swapchain(None, 640, 480, SwapChainConfig::default());
    let _cmd: QRhiCommandBuffer = QRhiCommandBuffer::new();
    let _vp: QRhiViewport = QRhiViewport::new(0.0, 0.0, 640.0, 480.0);
    let _sc_rect: QRhiScissor = QRhiScissor::new(0, 0, 640, 480);
    let _batch: QRhiResourceUpdateBatch = rhi.next_resource_update_batch();

    assert_eq!(sc.width(), 640);
}
