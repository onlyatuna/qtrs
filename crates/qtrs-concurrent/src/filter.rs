//! Parallel filter and filtered-reduced algorithms matching `QtConcurrent::filter` and `filteredReduced`.

use rayon::prelude::*;
use qtrs_core::thread::future::{Future, Promise};

/// Filters elements of a vector in-place concurrently, blocking until completion.
///
/// Corresponds to `QtConcurrent::blockingFilter(sequence, filterFunction)`.
///
/// # Examples
/// ```
/// use qtrs_concurrent::blocking_filter;
///
/// let mut numbers = vec![1, 2, 3, 4, 5, 6];
/// blocking_filter(&mut numbers, |n| n % 2 == 0);
/// assert_eq!(numbers, vec![2, 4, 6]);
/// ```
pub fn blocking_filter<T, P>(vec: &mut Vec<T>, predicate: P)
where
    T: Send,
    P: Fn(&T) -> bool + Sync + Send,
{
    let kept: Vec<T> = std::mem::take(vec)
        .into_par_iter()
        .filter(predicate)
        .collect();
    *vec = kept;
}

/// Filters elements of an owned vector concurrently in the background, returning a [`Future`].
///
/// Corresponds to `QtConcurrent::filter(sequence, filterFunction)`.
pub fn filter<T, P>(vec: Vec<T>, predicate: P) -> Future<Vec<T>>
where
    T: Clone + Send + 'static,
    P: Fn(&T) -> bool + Sync + Send + 'static,
{
    let promise = Promise::new();
    let fut = promise.future();

    rayon::spawn(move || {
        let kept: Vec<T> = vec.into_par_iter().filter(predicate).collect();
        promise.set_value(kept);
    });

    fut
}

/// Concurrently filters elements into a new vector, blocking until completion.
///
/// Corresponds to `QtConcurrent::blockingFiltered(sequence, filterFunction)`.
///
/// # Examples
/// ```
/// use qtrs_concurrent::blocking_filtered;
///
/// let words = vec!["apple", "banana", "avocado", "cherry"];
/// let a_words = blocking_filtered(words, |w| w.starts_with('a'));
/// assert_eq!(a_words, vec!["apple", "avocado"]);
/// ```
pub fn blocking_filtered<T, C, P>(collection: C, predicate: P) -> Vec<T>
where
    T: Send,
    C: IntoParallelIterator<Item = T>,
    P: Fn(&T) -> bool + Sync + Send,
{
    collection.into_par_iter().filter(predicate).collect()
}

/// Concurrently filters elements into a new vector in the background, returning a [`Future`].
///
/// Corresponds to `QtConcurrent::filtered(sequence, filterFunction)`.
pub fn filtered<T, C, P>(collection: C, predicate: P) -> Future<Vec<T>>
where
    T: Clone + Send + 'static,
    C: IntoParallelIterator<Item = T> + Send + 'static,
    P: Fn(&T) -> bool + Sync + Send + 'static,
{
    let promise = Promise::new();
    let fut = promise.future();

    rayon::spawn(move || {
        let kept: Vec<T> = collection.into_par_iter().filter(predicate).collect();
        promise.set_value(kept);
    });

    fut
}

/// Filters elements concurrently and accumulates matching items, blocking until complete.
///
/// Corresponds to `QtConcurrent::blockingFilteredReduced(sequence, filterFunction, reduceFunction)`.
///
/// # Examples
/// ```
/// use qtrs_concurrent::blocking_filtered_reduced;
///
/// let numbers = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
/// let even_sum = blocking_filtered_reduced(
///     numbers,
///     |n| n % 2 == 0,
///     |acc, n| *acc += n,
///     0,
/// );
/// assert_eq!(even_sum, 30);
/// ```
pub fn blocking_filtered_reduced<T, Acc, C, P, Red>(
    collection: C,
    predicate: P,
    reduce_fn: Red,
    initial_value: Acc,
) -> Acc
where
    T: Send,
    Acc: Send,
    C: IntoParallelIterator<Item = T>,
    P: Fn(&T) -> bool + Sync + Send,
    Red: Fn(&mut Acc, T) + Sync + Send,
{
    let filtered_items: Vec<T> = collection.into_par_iter().filter(predicate).collect();
    let mut accumulator = initial_value;
    for item in filtered_items {
        reduce_fn(&mut accumulator, item);
    }
    accumulator
}

/// Filters elements concurrently and accumulates matching items asynchronously, returning a [`Future`].
///
/// Corresponds to `QtConcurrent::filteredReduced(sequence, filterFunction, reduceFunction)`.
pub fn filtered_reduced<T, Acc, C, P, Red>(
    collection: C,
    predicate: P,
    reduce_fn: Red,
    initial_value: Acc,
) -> Future<Acc>
where
    T: Send + 'static,
    Acc: Clone + Send + 'static,
    C: IntoParallelIterator<Item = T> + Send + 'static,
    P: Fn(&T) -> bool + Sync + Send + 'static,
    Red: Fn(&mut Acc, T) + Sync + Send + 'static,
{
    let promise = Promise::new();
    let fut = promise.future();

    rayon::spawn(move || {
        let result = blocking_filtered_reduced(collection, predicate, reduce_fn, initial_value);
        promise.set_value(result);
    });

    fut
}
