//! # qtrs-concurrent
//!
//! High-performance concurrent data-parallel algorithms and asynchronous task execution
//! subsystem for `qtrs`, matching and modernizing Qt's `QtConcurrent` module:
//!
//! - [`run`] / [`run_on`] / [`run_rayon`]: Concurrent asynchronous task execution (`QtConcurrent::run`).
//! - [`task`] / [`TaskBuilder`]: Fluent task configuration and dispatch matching Qt 6's `QTaskBuilder`.
//! - [`map`] / [`blocking_map`]: In-place concurrent vector and slice transformation (`QtConcurrent::map`).
//! - [`mapped`] / [`blocking_mapped`]: Out-of-place concurrent mapping (`QtConcurrent::mapped`).
//! - [`mapped_reduced`] / [`blocking_mapped_reduced`]: Concurrent map with sequential or parallel reduction.
//! - [`filter`] / [`blocking_filter`]: In-place concurrent filtering.
//! - [`filtered`] / [`blocking_filtered`]: Out-of-place concurrent filtering.
//! - [`filtered_reduced`] / [`blocking_filtered_reduced`]: Concurrent filter with reduction.
//! - [`reduce`] / [`blocking_reduce`]: Parallel reduction algorithms.
//! - [`QtConcurrent`]: Qt C++-identical static method namespace for familiar Qt API ergonomics.

pub mod filter;
pub mod map;
pub mod reduce;
pub mod run;
pub mod task;
pub mod types;

// Re-export core functions
pub use filter::{
    blocking_filter, blocking_filtered, blocking_filtered_reduced, filter, filtered,
    filtered_reduced,
};
pub use map::{
    blocking_map, blocking_mapped, blocking_mapped_reduced, map, mapped, mapped_reduced,
};
pub use reduce::{blocking_reduce, blocking_reduce_parallel, reduce, ReduceOption};
pub use run::{run, run_on, run_rayon};
pub use task::{task, TaskBuilder};
pub use types::{QFuture, QPromise, QTaskBuilder, QtConcurrent};
