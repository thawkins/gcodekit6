//! Type aliases for commonly used complex types.
//!
//! This module provides type aliases to improve code readability by giving
//! meaningful names to complex nested types commonly used throughout the codebase.
//!
//! ## Rationale
//!
//! Complex types like `Arc<Mutex<Vec<Box<dyn Fn() + Send + Sync>>>>` are hard to read and
//! understand at a glance. Type aliases provide:
//! - **Readability**: `CallbackList` is clearer than the full type
//! - **Consistency**: Same pattern used the same way across crates
//! - **Documentation**: Alias names convey intent
//! - **Refactoring**: Change the underlying type in one place
//!
//! ## Usage
//!
//! ```rust,ignore
//! use gcodekit6_core::types::*;
//!
//! // Instead of: Arc<Mutex<Vec<String>>>
//! let queue: ThreadSafeVec<String> = Arc::new(Mutex::new(Vec::new()));
//! ```

use parking_lot::{Mutex, RwLock};
use std::collections::HashMap;
use std::sync::Arc;

// =============================================================================
// THREAD-SAFE SHARED TYPES (Arc<Mutex<T>> / Arc<RwLock<T>>)
// =============================================================================

/// A thread-safe, mutex-protected wrapper for cross-thread sharing.
///
/// Use when you need to share mutable state across threads (e.g., async tasks).
/// Uses `parking_lot::Mutex` for better performance than `std::sync::Mutex`.
///
/// # Example
/// ```rust,ignore
/// let state: ThreadSafe<AppState> = Arc::new(Mutex::new(AppState::default()));
/// state.lock().update();
/// ```
pub type ThreadSafe<T> = Arc<Mutex<T>>;

/// A thread-safe vector for cross-thread collection management.
pub type ThreadSafeVec<T> = Arc<Mutex<Vec<T>>>;

/// A thread-safe reader-writer lock wrapper for read-heavy workloads.
///
/// Use when reads greatly outnumber writes. Multiple readers can access
/// concurrently, but writes require exclusive access.
pub type ThreadSafeRw<T> = Arc<RwLock<T>>;

/// A thread-safe reader-writer hash map.
pub type ThreadSafeRwMap<K, V> = Arc<RwLock<HashMap<K, V>>>;

// =============================================================================
// CALLBACK TYPES
// =============================================================================

/// A simple callback with no parameters or return value.
///
/// Thread-safe, suitable for cross-thread event notification.
pub type Callback = Box<dyn Fn() + Send + Sync>;

/// A callback that receives a single parameter.
///
/// Thread-safe, suitable for cross-thread data notification.
pub type DataCallback<T> = Box<dyn Fn(T) + Send + Sync>;

// =============================================================================
// LISTENER/HANDLER TYPES
// =============================================================================

/// A list of callbacks for multi-listener patterns.
pub type CallbackList = ThreadSafeVec<Callback>;

/// A map of subscription IDs to callbacks for event bus patterns.
pub type SubscriptionMap<K, V> = ThreadSafeRwMap<K, V>;

// =============================================================================
// DYNAMIC DISPATCH TYPES (Box<dyn T>)
// =============================================================================
// These types use trait objects for legitimate runtime polymorphism.
// Each is documented with why dynamic dispatch is necessary.

/// A boxed dynamically-typed iterator.
///
/// Used when the concrete iterator type varies at runtime (e.g., forward vs reverse).
/// This is necessary because `Range<T>` and `Rev<Range<T>>` are different types.
///
/// # When to use
/// - Conditional iteration direction (forward/reverse)
/// - Iterator type determined at runtime
///
/// # Example
/// ```rust,ignore
/// let iter: BoxedIterator<u32> = if ascending {
///     Box::new(0..10)
/// } else {
///     Box::new((0..10).rev())
/// };
/// ```
pub type BoxedIterator<T> = Box<dyn Iterator<Item = T>>;

/// A boxed error type for simplified error handling.
///
/// Use `anyhow::Error` for most cases. This type is provided for compatibility
/// with APIs that require `Box<dyn Error>`.
///
/// # When to prefer alternatives
/// - Use `anyhow::Result<T>` for application code
/// - Use `thiserror` for library error types
/// - Use this only when interfacing with external APIs
pub type BoxedError = Box<dyn std::error::Error>;

/// A boxed error type that is thread-safe.
///
/// Use when errors need to cross thread boundaries.
pub type BoxedSendError = Box<dyn std::error::Error + Send + Sync>;

/// A result type with boxed error for legacy compatibility.
///
/// Prefer `anyhow::Result<T>` for new code.
pub type BoxedResult<T> = Result<T, BoxedError>;

/// A result type with thread-safe boxed error.
pub type BoxedSendResult<T> = Result<T, BoxedSendError>;

// =============================================================================
// CONSTRUCTOR HELPERS
// =============================================================================

/// Create a new `ThreadSafe<T>` from a value.
///
/// # Example
/// ```rust,ignore
/// let state = thread_safe(AppState::default());
/// ```
#[inline]
pub fn thread_safe<T>(value: T) -> ThreadSafe<T> {
    Arc::new(Mutex::new(value))
}

/// Create a new empty `ThreadSafeVec<T>`.
#[inline]
pub fn thread_safe_vec<T>() -> ThreadSafeVec<T> {
    Arc::new(Mutex::new(Vec::new()))
}

/// Create a new `ThreadSafeRw<T>` from a value.
#[inline]
pub fn thread_safe_rw<T>(value: T) -> ThreadSafeRw<T> {
    Arc::new(RwLock::new(value))
}
