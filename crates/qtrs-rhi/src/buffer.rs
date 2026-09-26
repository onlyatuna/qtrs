//! Buffer resource abstractions matching `QRhiBuffer`.

use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_BUFFER_ID: AtomicU64 = AtomicU64::new(1);

/// Type of buffer memory backing matching `QRhiBuffer::Type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum BufferType {
    /// Data is uploaded once and never updated.
    Immutable,
    /// Data is updated infrequently.
    #[default]
    Static,
    /// Data is updated frequently (per-frame).
    Dynamic,
}

/// Buffer usage flags matching `QRhiBuffer::UsageFlags`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BufferUsage {
    pub vertex: bool,
    pub index: bool,
    pub uniform: bool,
    pub storage: bool,
}

impl BufferUsage {
    pub const fn vertex() -> Self {
        Self {
            vertex: true,
            index: false,
            uniform: false,
            storage: false,
        }
    }

    pub const fn index() -> Self {
        Self {
            vertex: false,
            index: true,
            uniform: false,
            storage: false,
        }
    }

    pub const fn uniform() -> Self {
        Self {
            vertex: false,
            index: false,
            uniform: true,
            storage: false,
        }
    }

    pub const fn storage() -> Self {
        Self {
            vertex: false,
            index: false,
            uniform: false,
            storage: true,
        }
    }

    pub const fn vertex_and_index() -> Self {
        Self {
            vertex: true,
            index: true,
            uniform: false,
            storage: false,
        }
    }
}

/// Description of a buffer to be created.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BufferDescription {
    pub buffer_type: BufferType,
    pub usage: BufferUsage,
    pub size: usize,
}

impl BufferDescription {
    pub const fn new(buffer_type: BufferType, usage: BufferUsage, size: usize) -> Self {
        Self {
            buffer_type,
            usage,
            size,
        }
    }

    pub const fn vertex(size: usize) -> Self {
        Self::new(BufferType::Static, BufferUsage::vertex(), size)
    }

    pub const fn index(size: usize) -> Self {
        Self::new(BufferType::Static, BufferUsage::index(), size)
    }

    pub const fn uniform(size: usize) -> Self {
        Self::new(BufferType::Dynamic, BufferUsage::uniform(), size)
    }
}

/// RHI buffer resource matching `QRhiBuffer`.
#[derive(Debug)]
pub struct Buffer {
    id: u64,
    desc: BufferDescription,
}

impl Buffer {
    /// Creates a new buffer abstraction with the given description.
    pub fn new(desc: BufferDescription) -> Self {
        Self {
            id: NEXT_BUFFER_ID.fetch_add(1, Ordering::Relaxed),
            desc,
        }
    }
    /// Creates a buffer with explicit backend-assigned id.
    pub fn with_id(id: u64, desc: BufferDescription) -> Self {
        Self { id, desc }
    }


    /// Unique identifier for this buffer.
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Buffer description.
    pub fn description(&self) -> &BufferDescription {
        &self.desc
    }

    /// Buffer size in bytes.
    pub fn size(&self) -> usize {
        self.desc.size
    }

    /// Buffer type.
    pub fn buffer_type(&self) -> BufferType {
        self.desc.buffer_type
    }

    /// Buffer usage flags.
    pub fn usage(&self) -> BufferUsage {
        self.desc.usage
    }
}
