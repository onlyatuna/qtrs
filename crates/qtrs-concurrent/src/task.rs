//! Fluent task builder matching Qt 6's `QTaskBuilder` (`QtConcurrent::task`).

use std::sync::Arc;
use qtrs_core::thread::future::{Future, Promise};
use qtrs_core::thread::pool::ThreadPool;
use qtrs_core::thread::task::CancellationToken;
use qtrs_core::thread::thread::ThreadPriority;

/// Fluent task configuration builder matching `QTaskBuilder`.
///
/// # Examples
/// ```
/// use qtrs_concurrent::task;
/// use qtrs_core::thread::ThreadPriority;
///
/// let future = task(|| "computed value")
///     .with_priority(ThreadPriority::High)
///     .spawn();
///
/// assert_eq!(future.wait_result(), Some("computed value"));
/// ```
pub struct TaskBuilder<F> {
    func: F,
    pool: Option<Arc<ThreadPool>>,
    priority: ThreadPriority,
    cancellation: Option<CancellationToken>,
    use_rayon: bool,
}

/// Creates a new fluent [`TaskBuilder`] for configuring and launching an asynchronous task.
///
/// Corresponds to `QtConcurrent::task(f)`.
pub fn task<F, R>(f: F) -> TaskBuilder<F>
where
    F: FnOnce() -> R + Send + 'static,
    R: Clone + Send + 'static,
{
    TaskBuilder {
        func: f,
        pool: None,
        priority: ThreadPriority::Normal,
        cancellation: None,
        use_rayon: false,
    }
}

impl<F, R> TaskBuilder<F>
where
    F: FnOnce() -> R + Send + 'static,
    R: Clone + Send + 'static,
{
    /// Specifies the thread pool to execute the task on.
    ///
    /// Corresponds to `QTaskBuilder::onThreadPool(QThreadPool *pool)`.
    pub fn on_thread_pool(mut self, pool: Arc<ThreadPool>) -> Self {
        self.pool = Some(pool);
        self
    }

    /// Sets the task execution priority.
    ///
    /// Corresponds to `QTaskBuilder::withPriority(int priority)`.
    pub fn with_priority(mut self, priority: ThreadPriority) -> Self {
        self.priority = priority;
        self
    }

    /// Attaches a cooperative [`CancellationToken`] to this task.
    pub fn with_cancellation(mut self, token: CancellationToken) -> Self {
        self.cancellation = Some(token);
        self
    }

    /// Directs execution to Rayon's global work-stealing pool.
    pub fn on_rayon(mut self) -> Self {
        self.use_rayon = true;
        self
    }

    /// Spawns the task asynchronously and returns a [`Future`].
    ///
    /// Corresponds to `QTaskBuilder::spawn()`.
    pub fn spawn(self) -> Future<R> {
        let promise = Promise::new();
        let fut = promise.future();
        let cancel = self.cancellation;
        let func = self.func;

        let runner = move || {
            if let Some(ref c) = cancel {
                if c.is_canceled() {
                    return;
                }
            }
            let res = func();
            if let Some(ref c) = cancel {
                if c.is_canceled() {
                    return;
                }
            }
            promise.set_value(res);
        };

        if self.use_rayon {
            rayon::spawn(runner);
        } else if let Some(pool) = self.pool {
            pool.spawn(runner);
        } else {
            ThreadPool::global().spawn(runner);
        }

        fut
    }
}
