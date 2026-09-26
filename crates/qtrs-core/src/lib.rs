//! qtrs-core: Core primitives, event system, and object model.

pub mod event;
pub mod event_loop;
pub mod object;
pub mod signal;
pub mod timer;
pub mod variant;
pub mod animation;
pub mod settings;
pub mod fs;
pub mod property;
pub mod meta;
pub mod thread;
pub mod types;
pub mod io;
pub mod json;
pub mod serialize;

pub use event::*;
pub use event_loop::*;
pub use object::*;
pub use signal::*;
pub use timer::*;
pub use variant::*;
pub use animation::*;
pub use settings::*;
pub use fs::*;
pub use property::*;
pub use meta::*;
pub use thread::*;
pub use types::*;
pub use io::*;
pub use json::*;
pub use serialize::*;
pub mod application;
pub use application::*;

#[cfg(feature = "derive")]
pub use qtrs_derive::QObject;
