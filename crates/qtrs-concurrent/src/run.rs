//! Asynchronous execution primitives matching `QtConcurrent::run`.

use qtrs_core::thread::future::{Future, Promise};
use qtrs_core::thread::pool::ThreadPool;

/// Runs a callable asynchronously on the global thread pool, returning a [`Future`].
///
/// Corresponds to `QtConcurrent::run(f)`.
///
/// # Examples
/// ```
/// use qtrs_concurrent::run;
///
/// let future = run(|| 40 + 2);
/// assert_eq!(future.wait_result(), Some(42));
/// ```
pub fn run<F, R>(f: F) -> Future<R>
where
    F: FnOnce() -> R + Send + 'static,
    R: Clone + Send + 'static,
{
    let promise = Promise::new();
    let fut = promise.future();

    ThreadPool::global().spawn(move || {
        let result = f();
        promise.set_value(result);
    });

    fut
}

/// Runs a callable asynchronously on the specified [`ThreadPool`], returning a [`Future`].
///
/// Corresponds to `QtConcurrent::run(QThreadPool *pool, f)`.
pub fn run_on<F, R>(pool: &ThreadPool, f: F) -> Future<R>
where
    F: FnOnce() -> R + Send + 'static,
    R: Clone + Send + 'static,
{
    let promise = Promise::new();
    let fut = promise.future();

    pool.spawn(move || {
        let result = f();
        promise.set_value(result);
    });

    fut
}

/// Runs a callable asynchronously on Rayon's global work-stealing thread pool, returning a [`Future`].
pub fn run_rayon<F, R>(f: F) -> Future<R>
where
    F: FnOnce() -> R + Send + 'static,
    R: Clone + Send + 'static,
{
    let promise = Promise::new();
    let fut = promise.future();

    rayon::spawn(move || {
        let result = f();
        promise.set_value(result);
    });

    fut
}
