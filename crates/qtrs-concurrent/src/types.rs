//! Canonical Qt Concurrent type aliases and `QtConcurrent` namespace wrapper.

use rayon::iter::IntoParallelIterator;

use qtrs_core::thread::future::Future;
use qtrs_core::thread::pool::ThreadPool;

use crate::filter::*;
use crate::map::*;
use crate::reduce::*;
use crate::run::*;
use crate::task::{task, TaskBuilder};

/// Canonical Qt alias for [`TaskBuilder`].
pub type QTaskBuilder<F> = TaskBuilder<F>;

/// Canonical Qt alias for [`Future`].
pub type QFuture<T> = Future<T>;

/// Canonical Qt alias for [`Promise`].
pub type QPromise<T> = qtrs_core::thread::future::Promise<T>;

/// Qt-style namespace struct matching C++ `QtConcurrent::*`.
///
/// Provides identical API ergonomics to Qt's C++ static member functions.
pub struct QtConcurrent;

impl QtConcurrent {
    /// Corresponds to `QtConcurrent::run(f)`.
    #[inline]
    pub fn run<F, R>(f: F) -> Future<R>
    where
        F: FnOnce() -> R + Send + 'static,
        R: Clone + Send + 'static,
    {
        run(f)
    }

    /// Corresponds to `QtConcurrent::run(QThreadPool *pool, f)`.
    #[inline]
    pub fn run_on<F, R>(pool: &ThreadPool, f: F) -> Future<R>
    where
        F: FnOnce() -> R + Send + 'static,
        R: Clone + Send + 'static,
    {
        run_on(pool, f)
    }

    /// Corresponds to `QtConcurrent::task(f)`.
    #[inline]
    pub fn task<F, R>(f: F) -> TaskBuilder<F>
    where
        F: FnOnce() -> R + Send + 'static,
        R: Clone + Send + 'static,
    {
        task(f)
    }

    /// Corresponds to `QtConcurrent::blockingMap(sequence, f)`.
    #[inline]
    pub fn blocking_map<T, F>(slice: &mut [T], f: F)
    where
        T: Send,
        F: Fn(&mut T) + Sync + Send,
    {
        blocking_map(slice, f);
    }

    /// Corresponds to `QtConcurrent::map(sequence, f)`.
    #[inline]
    pub fn map<T, F>(vec: Vec<T>, f: F) -> Future<Vec<T>>
    where
        T: Clone + Send + 'static,
        F: Fn(&mut T) + Sync + Send + 'static,
    {
        map(vec, f)
    }

    /// Corresponds to `QtConcurrent::blockingMapped(sequence, f)`.
    #[inline]
    pub fn blocking_mapped<T, R, C, F>(collection: C, f: F) -> Vec<R>
    where
        T: Send,
        R: Send,
        C: IntoParallelIterator<Item = T>,
        F: Fn(T) -> R + Sync + Send,
    {
        blocking_mapped(collection, f)
    }

    /// Corresponds to `QtConcurrent::mapped(sequence, f)`.
    #[inline]
    pub fn mapped<T, R, C, F>(collection: C, f: F) -> Future<Vec<R>>
    where
        T: Send + 'static,
        R: Clone + Send + 'static,
        C: IntoParallelIterator<Item = T> + Send + 'static,
        F: Fn(T) -> R + Sync + Send + 'static,
    {
        mapped(collection, f)
    }

    /// Corresponds to `QtConcurrent::blockingMappedReduced(sequence, map, reduce)`.
    #[inline]
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
        blocking_mapped_reduced(collection, map_fn, reduce_fn, initial_value)
    }

    /// Corresponds to `QtConcurrent::mappedReduced(sequence, map, reduce)`.
    #[inline]
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
        mapped_reduced(collection, map_fn, reduce_fn, initial_value)
    }

    /// Corresponds to `QtConcurrent::blockingFilter(sequence, predicate)`.
    #[inline]
    pub fn blocking_filter<T, P>(vec: &mut Vec<T>, predicate: P)
    where
        T: Send,
        P: Fn(&T) -> bool + Sync + Send,
    {
        blocking_filter(vec, predicate);
    }

    /// Corresponds to `QtConcurrent::filter(sequence, predicate)`.
    #[inline]
    pub fn filter<T, P>(vec: Vec<T>, predicate: P) -> Future<Vec<T>>
    where
        T: Clone + Send + 'static,
        P: Fn(&T) -> bool + Sync + Send + 'static,
    {
        filter(vec, predicate)
    }

    /// Corresponds to `QtConcurrent::blockingFiltered(sequence, predicate)`.
    #[inline]
    pub fn blocking_filtered<T, C, P>(collection: C, predicate: P) -> Vec<T>
    where
        T: Send,
        C: IntoParallelIterator<Item = T>,
        P: Fn(&T) -> bool + Sync + Send,
    {
        blocking_filtered(collection, predicate)
    }

    /// Corresponds to `QtConcurrent::filtered(sequence, predicate)`.
    #[inline]
    pub fn filtered<T, C, P>(collection: C, predicate: P) -> Future<Vec<T>>
    where
        T: Clone + Send + 'static,
        C: IntoParallelIterator<Item = T> + Send + 'static,
        P: Fn(&T) -> bool + Sync + Send + 'static,
    {
        filtered(collection, predicate)
    }

    /// Corresponds to `QtConcurrent::blockingFilteredReduced(sequence, filter, reduce)`.
    #[inline]
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
        blocking_filtered_reduced(collection, predicate, reduce_fn, initial_value)
    }

    /// Corresponds to `QtConcurrent::filteredReduced(sequence, filter, reduce)`.
    #[inline]
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
        filtered_reduced(collection, predicate, reduce_fn, initial_value)
    }

    /// Corresponds to blocking reduction.
    #[inline]
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
        blocking_reduce(collection, reduce_fn, initial_value)
    }

    /// Corresponds to asynchronous reduction.
    #[inline]
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
        reduce(collection, reduce_fn, initial_value)
    }
}
