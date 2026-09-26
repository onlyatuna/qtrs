//! Cross-platform I/O subsystem and abstractions (`Qt corelib/io` equivalent).
//!
//! Provides file and stream devices, in-memory buffers, directory management,
//! path manipulation, virtual embedded resources, standard system paths,
//! external process execution, temporary files/directories, advisory lock files,
//! and storage volume inspection.

pub mod device;
pub mod buffer;
pub mod file;
pub mod path;
pub mod dir;
pub mod resource;
pub mod standard_paths;
pub mod process;
pub mod temp;
pub mod lock_file;
pub mod storage_info;

pub use device::*;
pub use buffer::*;
pub use file::*;
pub use path::*;
pub use dir::*;
pub use resource::*;
pub use standard_paths::*;
pub use process::*;
pub use temp::*;
pub use lock_file::*;
pub use storage_info::*;
