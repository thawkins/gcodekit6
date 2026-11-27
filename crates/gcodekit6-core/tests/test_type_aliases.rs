//! Tests for `gcodekit6_core::types::aliases` (moved from the inline test module).

use gcodekit6_core::types::aliases::*;

#[test]
fn test_thread_safe_creation() {
    let value: ThreadSafe<i32> = thread_safe(42);
    assert_eq!(*value.lock(), 42);

    *value.lock() = 100;
    assert_eq!(*value.lock(), 100);
}

#[test]
fn test_thread_safe_vec() {
    let vec: ThreadSafeVec<String> = thread_safe_vec();
    vec.lock().push("item1".to_string());
    vec.lock().push("item2".to_string());

    assert_eq!(vec.lock().len(), 2);
}

#[test]
fn test_thread_safe_rw() {
    let value: ThreadSafeRw<i32> = thread_safe_rw(42);

    // Multiple readers
    assert_eq!(*value.read(), 42);
    assert_eq!(*value.read(), 42);

    // Writer
    *value.write() = 100;
    assert_eq!(*value.read(), 100);
}

#[test]
fn test_boxed_iterator() {
    // Forward iterator
    let forward: BoxedIterator<u32> = Box::new(0..5);
    let result: Vec<u32> = forward.collect();
    assert_eq!(result, vec![0, 1, 2, 3, 4]);

    // Reverse iterator
    let reverse: BoxedIterator<u32> = Box::new((0..5).rev());
    let result: Vec<u32> = reverse.collect();
    assert_eq!(result, vec![4, 3, 2, 1, 0]);

    // Conditional direction (simulating bidirectional scanning)
    let ascending = true;
    let iter: BoxedIterator<u32> = if ascending {
        Box::new(0..3)
    } else {
        Box::new((0..3).rev())
    };
    let result: Vec<u32> = iter.collect();
    assert_eq!(result, vec![0, 1, 2]);
}

#[test]
fn test_boxed_result() {
    fn fallible_operation(succeed: bool) -> BoxedResult<i32> {
        if succeed {
            Ok(42)
        } else {
            Err("operation failed".into())
        }
    }

    assert!(fallible_operation(true).is_ok());
    assert!(fallible_operation(false).is_err());
}
