//! Comprehensive integration test suite for `qtrs-concurrent`.
//!
//! Validates:
//! 1. `run` and `run_rayon` asynchronous future execution.
//! 2. `task` / `TaskBuilder` fluent configuration and cancellation.
//! 3. `blocking_map` and `map` in-place transformation.
//! 4. `blocking_mapped` and `mapped` transformation into new collections.
//! 5. `blocking_mapped_reduced` and `mapped_reduced` map with reduction.
//! 6. `blocking_filter` and `filter` in-place filtering.
//! 7. `blocking_filtered` and `filtered` out-of-place filtering.
//! 8. `blocking_filtered_reduced` and `filtered_reduced` filter with reduction.
//! 9. `blocking_reduce`, `blocking_reduce_parallel`, and `reduce`.
//! 10. `QtConcurrent` namespace static API ergonomics.
//! 11. Canonical Qt aliases (`QFuture`, `QPromise`, `QTaskBuilder`).

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use qtrs_concurrent::*;
use qtrs_core::thread::task::CancellationToken;
use qtrs_core::thread::thread::ThreadPriority;

// =============================================================================
// 1. Concurrent Run Tests
// =============================================================================

#[test]
fn test_concurrent_run() {
    let future = run(|| {
        let mut sum = 0;
        for i in 1..=100 {
            sum += i;
        }
        sum
    });

    let result = future.wait_result();
    assert_eq!(result, Some(5050));
}

#[test]
fn test_concurrent_run_rayon() {
    let future = run_rayon(|| "computed in rayon");
    let result = future.wait_result();
    assert_eq!(result, Some("computed in rayon"));
}

// =============================================================================
// 2. TaskBuilder Tests
// =============================================================================

#[test]
fn test_task_builder_fluent() {
    let future = task(|| 123 * 2)
        .with_priority(ThreadPriority::High)
        .on_rayon()
        .spawn();

    assert_eq!(future.wait_result(), Some(246));
}

#[test]
fn test_task_builder_cancellation() {
    let cancel = CancellationToken::new();
    cancel.cancel(); // Pre-canceled

    let executed = Arc::new(AtomicUsize::new(0));
    let exec_clone = Arc::clone(&executed);

    let future = task(move || {
        exec_clone.fetch_add(1, Ordering::SeqCst);
        42
    })
    .with_cancellation(cancel)
    .spawn();

    // Cancellation causes the task to skip execution
    let result = future.wait_timeout(Duration::from_millis(50));
    assert_eq!(result, None);
    assert_eq!(executed.load(Ordering::SeqCst), 0);
}

// =============================================================================
// 3. Map & Mapped Tests
// =============================================================================

#[test]
fn test_blocking_map_and_async_map() {
    // In-place blocking map
    let mut numbers = vec![1, 2, 3, 4, 5];
    blocking_map(&mut numbers, |n| *n *= 10);
    assert_eq!(numbers, vec![10, 20, 30, 40, 50]);

    // Asynchronous map returning Future<Vec<T>>
    let future = map(vec![1, 2, 3], |n| *n += 100);
    let mapped_vec = future.wait_result().expect("async map");
    assert_eq!(mapped_vec, vec![101, 102, 103]);
}

#[test]
fn test_blocking_mapped_and_async_mapped() {
    let words = vec!["qt", "concurrent", "rust"];

    // Blocking mapped
    let upper = blocking_mapped(words.clone(), |s| s.to_uppercase());
    assert_eq!(upper, vec!["QT", "CONCURRENT", "RUST"]);

    // Asynchronous mapped
    let future = mapped(words, |s| s.len());
    let lengths = future.wait_result().expect("async mapped");
    assert_eq!(lengths, vec![2, 10, 4]);
}

#[test]
fn test_mapped_reduced() {
    let numbers = vec![1, 2, 3, 4, 5];

    // Blocking mapped reduced: square then sum
    let sum_of_squares = blocking_mapped_reduced(
        numbers.clone(),
        |n| n * n,
        |acc, sq| *acc += sq,
        0,
    );
    assert_eq!(sum_of_squares, 1 + 4 + 9 + 16 + 25);

    // Asynchronous mapped reduced
    let future = mapped_reduced(
        numbers,
        |n| n * 2,
        |acc, doubled| *acc += doubled,
        0,
    );
    assert_eq!(future.wait_result(), Some(30));
}

// =============================================================================
// 4. Filter & Filtered Tests
// =============================================================================

#[test]
fn test_blocking_filter_and_async_filter() {
    // In-place blocking filter
    let mut numbers = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    blocking_filter(&mut numbers, |n| n % 2 == 0);
    assert_eq!(numbers, vec![2, 4, 6, 8, 10]);

    // Asynchronous filter
    let future = filter(vec![10, 15, 20, 25], |n| *n >= 20);
    assert_eq!(future.wait_result(), Some(vec![20, 25]));
}

#[test]
fn test_blocking_filtered_and_async_filtered() {
    let items = vec!["rust", "c++", "python", "ruby"];

    // Blocking filtered
    let filtered_items = blocking_filtered(items.clone(), |s| s.starts_with('r'));
    assert_eq!(filtered_items, vec!["rust", "ruby"]);

    // Asynchronous filtered
    let future = filtered(items, |s| s.len() > 3);
    assert_eq!(future.wait_result(), Some(vec!["rust", "python", "ruby"]));
}

#[test]
fn test_filtered_reduced() {
    let numbers = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

    // Blocking filtered reduced: even numbers product
    let even_product = blocking_filtered_reduced(
        numbers.clone(),
        |n| n % 2 == 0,
        |acc, n| *acc *= n,
        1,
    );
    assert_eq!(even_product, 2 * 4 * 6 * 8 * 10);

    // Asynchronous filtered reduced: sum of odd numbers
    let future = filtered_reduced(
        numbers,
        |n| n % 2 != 0,
        |acc, n| *acc += n,
        0,
    );
    assert_eq!(future.wait_result(), Some(1 + 3 + 5 + 7 + 9));
}

// =============================================================================
// 5. Reduce Tests
// =============================================================================

#[test]
fn test_reduce_and_parallel_reduce() {
    let numbers = vec![1, 2, 3, 4, 5];

    // Blocking reduce
    let sum = blocking_reduce(numbers.clone(), |acc, n| *acc += n, 0);
    assert_eq!(sum, 15);

    // Parallel tree reduction
    let product = blocking_reduce_parallel(numbers.clone(), || 1, |a, b| a * b);
    assert_eq!(product, 120);

    // Asynchronous reduce
    let future = reduce(numbers, |acc, n| *acc += n, 100);
    assert_eq!(future.wait_result(), Some(115));
}

// =============================================================================
// 6. QtConcurrent Namespace Static API Tests
// =============================================================================

#[test]
fn test_qt_concurrent_namespace_api() {
    // QtConcurrent::run
    let fut_run = QtConcurrent::run(|| "hello from QtConcurrent");
    assert_eq!(fut_run.wait_result(), Some("hello from QtConcurrent"));

    // QtConcurrent::task
    let fut_task = QtConcurrent::task(|| 42).spawn();
    assert_eq!(fut_task.wait_result(), Some(42));

    // QtConcurrent::mapped
    let fut_mapped = QtConcurrent::mapped(vec![1, 2, 3], |x| x * 10);
    assert_eq!(fut_mapped.wait_result(), Some(vec![10, 20, 30]));

    // QtConcurrent::filtered
    let fut_filtered = QtConcurrent::filtered(vec![1, 2, 3, 4], |x| *x % 2 == 0);
    assert_eq!(fut_filtered.wait_result(), Some(vec![2, 4]));

    // QtConcurrent::reduce
    let fut_reduced = QtConcurrent::reduce(vec![10, 20, 30], |acc, x| *acc += x, 0);
    assert_eq!(fut_reduced.wait_result(), Some(60));
}

// =============================================================================
// 7. Canonical Qt Type Aliases
// =============================================================================

#[test]
fn test_canonical_qt_type_aliases() {
    let fut: QFuture<i32> = QtConcurrent::run(|| 999);
    assert_eq!(fut.wait_result(), Some(999));

    let _builder: QTaskBuilder<_> = QtConcurrent::task(|| "task");
}
