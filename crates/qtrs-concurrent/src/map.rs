//! Parallel map and mapped-reduced algorithms matching `QtConcurrent::map` and `mappedReduced`.

use rayon::prelude::*;
use qtrs_core::thread::future::{Future, Promise};

/// Modifies each element of the slice in-place concurrently, blocking until completion.
///
/// Corresponds to `QtConcurrent::blockingMap(sequence, mapFunction)`.
///
/// # Examples
/// ```
/// use qtrs_concurrent::blocking_map;
///
/// let mut numbers = vec![1, 2, 3, 4];
/// blocking_map(&mut numbers, |n| *n *= 2);
/// assert_eq!(numbers, vec![2, 4, 6, 8]);
/// ```
pub fn blocking_map<T, F>(slice: &mut [T], f: F)
where
    T: Send,
    F: Fn(&mut T) + Sync + Send,
{
    slice.par_iter_mut().for_each(f);
}

/// Modifies each element of an owned vector concurrently in the background, returning a [`Future`].
///
/// Corresponds to `QtConcurrent::map(sequence, mapFunction)`.
pub fn map<T, F>(mut vec: Vec<T>, f: F) -> Future<Vec<T>>
where
    T: Clone + Send + 'static,
    F: Fn(&mut T) + Sync + Send + 'static,
{
    let promise = Promise::new();
    let fut = promise.future();

    rayon::spawn(move || {
        vec.par_iter_mut().for_each(f);
        promise.set_value(vec);
    });

    fut
}

/// Transforms each item of a collection concurrently, returning a new vector.
///
/// Blocks until all elements have been transformed.
///
/// Corresponds to `QtConcurrent::blockingMapped(sequence, mapFunction)`.
///
/// # Examples
/// ```
/// use qtrs_concurrent::blocking_mapped;
///
/// let words = vec!["hello", "world"];
/// let lengths = blocking_mapped(words, |w| w.len());
/// assert_eq!(lengths, vec![5, 5]);
/// ```
pub fn blocking_mapped<T, R, C, F>(collection: C, f: F) -> Vec<R>
where
    T: Send,
    R: Send,
    C: IntoParallelIterator<Item = T>,
    F: Fn(T) -> R + Sync + Send,
{
    collection.into_par_iter().map(f).collect()
}

/// Transforms each item of a collection concurrently in the background, returning a [`Future`].
///
/// Corresponds to `QtConcurrent::mapped(sequence, mapFunction)`.
pub fn mapped<T, R, C, F>(collection: C, f: F) -> Future<Vec<R>>
where
    T: Send + 'static,
    R: Clone + Send + 'static,
    C: IntoParallelIterator<Item = T> + Send + 'static,
    F: Fn(T) -> R + Sync + Send + 'static,
{
    let promise = Promise::new();
    let fut = promise.future();

    rayon::spawn(move || {
        let results: Vec<R> = collection.into_par_iter().map(f).collect();
        promise.set_value(results);
    });

    fut
}

/// Maps each item concurrently and accumulates results via `reduce_fn`, blocking until complete.
///
/// Corresponds to `QtConcurrent::blockingMappedReduced(sequence, mapFunction, reduceFunction)`.
///
/// # Examples
/// ```
/// use qtrs_concurrent::blocking_mapped_reduced;
///
/// let numbers = vec![1, 2, 3, 4, 5];
/// let sum_of_squares = blocking_mapped_reduced(
///     numbers,
///     |n| n * n,
///     |acc, sq| *acc += sq,
///     0,
/// );
/// assert_eq!(sum_of_squares, 55);
/// ```
pub fn blocking_mapped_reduced<T, R, Acc, C, M, Red>(
    collection: C,
    map_fn: M,
    reduce_fn: Red,
    initial_value: Acc,
) -> Acc
where
    T: Send,
    R: Send,
    Acc: Send,
    C: IntoParallelIterator<Item = T>,
    M: Fn(T) -> R + Sync + Send,
    Red: Fn(&mut Acc, R) + Sync + Send,
{
    let mapped_items: Vec<R> = collection.into_par_iter().map(map_fn).collect();
    let mut accumulator = initial_value;
    for item in mapped_items {
        reduce_fn(&mut accumulator, item);
    }
    accumulator
}

/// Maps each item concurrently and accumulates results asynchronously, returning a [`Future`].
///
/// Corresponds to `QtConcurrent::mappedReduced(sequence, mapFunction, reduceFunction)`.
pub fn mapped_reduced<T, R, Acc, C, M, Red>(
    collection: C,
    map_fn: M,
    reduce_fn: Red,
    initial_value: Acc,
) -> Future<Acc>
where
    T: Send + 'static,
    R: Send + 'static,
    Acc: Clone + Send + 'static,
    C: IntoParallelIterator<Item = T> + Send + 'static,
    M: Fn(T) -> R + Sync + Send + 'static,
    Red: Fn(&mut Acc, R) + Sync + Send + 'static,
{
    let promise = Promise::new();
    let fut = promise.future();

    rayon::spawn(move || {
        let result = blocking_mapped_reduced(collection, map_fn, reduce_fn, initial_value);
        promise.set_value(result);
    });

    fut
}
