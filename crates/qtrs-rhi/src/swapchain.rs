//! Swapchain and presentation abstractions matching `QRhiSwapChain`.

use std::sync::atomic::{AtomicU64, Ordering};
use crate::render_target::SwapchainRenderTarget;
use crate::texture::TextureFormat;

static NEXT_SWAPCHAIN_ID: AtomicU64 = AtomicU64::new(1);

/// Presentation mode controlling VSync matching `QRhiSwapChain::PresentMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PresentMode {
    /// Immediate presentation with potential tearing (VSync off).
    Immediate,
    /// Triple-buffering without tearing, replaces queued images.
    Mailbox,
    /// Standard vertical-sync locked presentation (VSync on).
    #[default]
    Fifo,
}

/// Swapchain configuration description.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwapChainConfig {
    pub format: TextureFormat,
    pub present_mode: PresentMode,
    pub sample_count: u32,
    pub enable_depth_stencil: bool,
}

impl Default for SwapChainConfig {
    fn default() -> Self {
        Self {
            format: TextureFormat::Bgra8Unorm,
            present_mode: PresentMode::Fifo,
            sample_count: 1,
            enable_depth_stencil: false,
        }
    }
}

/// Swapchain abstraction matching Qt 6 `QRhiSwapChain`.
#[derive(Debug)]
pub struct SwapChain {
    id: u64,
    surface_handle: Option<usize>,
    config: SwapChainConfig,
    width: u32,
    height: u32,
    render_target: SwapchainRenderTarget,
}

impl SwapChain {
    pub fn new(surface_handle: Option<usize>, width: u32, height: u32, config: SwapChainConfig) -> Self {
        let id = NEXT_SWAPCHAIN_ID.fetch_add(1, Ordering::Relaxed);
        let render_target = SwapchainRenderTarget::new(id, width, height);
        Self {
            id,
            surface_handle,
            config,
            width,
            height,
            render_target,
        }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn surface_handle(&self) -> Option<usize> {
        self.surface_handle
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn config(&self) -> &SwapChainConfig {
        &self.config
    }

    pub fn render_target(&self) -> &SwapchainRenderTarget {
        &self.render_target
    }

    pub fn render_target_mut(&mut self) -> &mut SwapchainRenderTarget {
        &mut self.render_target
    }

    /// Resizes the swapchain to new window dimensions.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.render_target.resize(width, height);
    }
}
