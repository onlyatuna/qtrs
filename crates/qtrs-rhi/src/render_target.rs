//! Render target abstractions matching `QRhiRenderTarget`, `QRhiTextureRenderTarget`, and `QRhiSwapChainRenderTarget`.

use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_RT_ID: AtomicU64 = AtomicU64::new(1);

/// Attachment for a color render target matching `QRhiColorAttachment`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColorAttachment {
    pub texture_id: u64,
    pub level: u32,
    pub layer: u32,
    pub resolve_texture_id: Option<u64>,
}

impl ColorAttachment {
    pub const fn new(texture_id: u64) -> Self {
        Self {
            texture_id,
            level: 0,
            layer: 0,
            resolve_texture_id: None,
        }
    }

    pub const fn with_resolve(texture_id: u64, resolve_texture_id: u64) -> Self {
        Self {
            texture_id,
            level: 0,
            layer: 0,
            resolve_texture_id: Some(resolve_texture_id),
        }
    }
}

/// Description for offscreen texture render target matching `QRhiTextureRenderTargetDescription`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextureRenderTargetDescription {
    pub color_attachments: Vec<ColorAttachment>,
    pub depth_texture_id: Option<u64>,
    pub width: u32,
    pub height: u32,
}

impl TextureRenderTargetDescription {
    pub fn new(color_attachment: ColorAttachment, width: u32, height: u32) -> Self {
        Self {
            color_attachments: vec![color_attachment],
            depth_texture_id: None,
            width,
            height,
        }
    }

    pub fn with_depth(
        color_attachment: ColorAttachment,
        depth_texture_id: u64,
        width: u32,
        height: u32,
    ) -> Self {
        Self {
            color_attachments: vec![color_attachment],
            depth_texture_id: Some(depth_texture_id),
            width,
            height,
        }
    }
}

/// Offscreen texture render target matching `QRhiTextureRenderTarget`.
#[derive(Debug)]
pub struct TextureRenderTarget {
    id: u64,
    desc: TextureRenderTargetDescription,
}

impl TextureRenderTarget {
    pub fn new(desc: TextureRenderTargetDescription) -> Self {
        Self {
            id: NEXT_RT_ID.fetch_add(1, Ordering::Relaxed),
            desc,
        }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn description(&self) -> &TextureRenderTargetDescription {
        &self.desc
    }

    pub fn width(&self) -> u32 {
        self.desc.width
    }

    pub fn height(&self) -> u32 {
        self.desc.height
    }
}

/// Window on-screen swapchain render target matching `QRhiSwapChainRenderTarget`.
#[derive(Debug)]
pub struct SwapchainRenderTarget {
    id: u64,
    swapchain_id: u64,
    width: u32,
    height: u32,
}

impl SwapchainRenderTarget {
    pub fn new(swapchain_id: u64, width: u32, height: u32) -> Self {
        Self {
            id: NEXT_RT_ID.fetch_add(1, Ordering::Relaxed),
            swapchain_id,
            width,
            height,
        }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn swapchain_id(&self) -> u64 {
        self.swapchain_id
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
    }
}

/// General RenderTarget enum unifying swapchain and offscreen render targets.
#[derive(Debug)]
pub enum RenderTarget<'a> {
    Swapchain(&'a SwapchainRenderTarget),
    Texture(&'a TextureRenderTarget),
}

impl<'a> RenderTarget<'a> {
    pub fn width(&self) -> u32 {
        match self {
            Self::Swapchain(sc) => sc.width(),
            Self::Texture(t) => t.width(),
        }
    }

    pub fn height(&self) -> u32 {
        match self {
            Self::Swapchain(sc) => sc.height(),
            Self::Texture(t) => t.height(),
        }
    }

    pub fn id(&self) -> u64 {
        match self {
            Self::Swapchain(sc) => sc.id(),
            Self::Texture(t) => t.id(),
        }
    }
}
