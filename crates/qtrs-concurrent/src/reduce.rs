//! Parallel reduction algorithms matching Qt Concurrent reduce operations.

use rayon::prelude::*;
use qtrs_core::thread::future::{Future, Promise};

/// Options controlling reduction order and parallelism matching Qt's `QtConcurrent::ReduceOptions`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReduceOption {
    /// Preserves sequential accumulation order.
    SequentialReduce,
    /// Allows out-of-order accumulation for maximum parallel throughput.
    UnorderedReduce,
    /// Preserves original sequence ordering.
    OrderedReduce,
}

/// Accumulates elements of a collection into a summary value, blocking until complete.
///
/// Corresponds to sequential/blocking reduction in Qt Concurrent.
///
/// # Examples
/// ```
/// use qtrs_concurrent::blocking_reduce;
///
/// let numbers = vec![1, 2, 3, 4, 5];
/// let sum = blocking_reduce(numbers, |acc, n| *acc += n, 0);
/// assert_eq!(sum, 15);
/// ```
pub fn blocking_reduce<T, Acc, C, Red>(
    collection: C,
    reduce_fn: Red,
    initial_value: Acc,
) -> Acc
where
    T: Send,
    Acc: Send,
    C: IntoParallelIterator<Item = T>,
    Red: Fn(&mut Acc, T) + Sync + Send,
{
    let items: Vec<T> = collection.into_par_iter().collect();
    let mut accumulator = initial_value;
    for item in items {
        reduce_fn(&mut accumulator, item);
    }
    accumulator
}

/// Accumulates elements of a collection in full parallel tree-reduction when operation is associative.
///
/// # Examples
/// ```
/// use qtrs_concurrent::blocking_reduce_parallel;
///
/// let numbers = vec![1, 2, 3, 4, 5];
/// let product = blocking_reduce_parallel(numbers, || 1, |a, b| a * b);
/// assert_eq!(product, 120);
/// ```
pub fn blocking_reduce_parallel<T, C, Id, Red>(
    collection: C,
    identity: Id,
    reduce_op: Red,
) -> T
where
    T: Send + Sync + Copy,
    C: IntoParallelIterator<Item = T>,
    Id: Fn() -> T + Sync + Send,
    Red: Fn(T, T) -> T + Sync + Send,
{
    collection
        .into_par_iter()
        .reduce(identity, reduce_op)
}

/// Accumulates elements of a collection into a summary value asynchronously, returning a [`Future`].
///
/// # Examples
/// ```
/// use qtrs_concurrent::reduce;
///
/// let numbers = vec![10, 20, 30];
/// let future = reduce(numbers, |acc, n| *acc += n, 0);
/// assert_eq!(future.wait_result(), Some(60));
/// ```
pub fn reduce<T, Acc, C, Red>(
    collection: C,
    reduce_fn: Red,
    initial_value: Acc,
) -> Future<Acc>
where
    T: Send + 'static,
    Acc: Clone + Send + 'static,
    C: IntoParallelIterator<Item = T> + Send + 'static,
    Red: Fn(&mut Acc, T) + Sync + Send + 'static,
{
    let promise = Promise::new();
    let fut = promise.future();

    rayon::spawn(move || {
        let result = blocking_reduce(collection, reduce_fn, initial_value);
        promise.set_value(result);
    });

    fut
}
