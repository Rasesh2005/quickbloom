//! Thread-safe concurrent wrapper for Bloom filters.
//!
//! This module providing a wrapper that uses `Arc<RwLock>` to allow
//! shared access across multiple threads.

use std::sync::{Arc, RwLock};

/// Thread-safe Bloom filter wrapper.
///
/// Wraps any inner Bloom Filter implementation (Standard or Scalable)
/// into an Arc<RwLock> for seamless concurrency.
#[derive(Clone)]
pub struct ConcurrentBloomFilter<F> {
    inner: Arc<RwLock<F>>,
}

impl<F> ConcurrentBloomFilter<F> {
    /// Wrap an existing Bloom filter safely.
    pub fn new(filter: F) -> Self {
        Self {
            inner: Arc::new(RwLock::new(filter)),
        }
    }

    /// Gets the strong reference count to the underlying lock
    pub fn ref_count(&self) -> usize {
        Arc::strong_count(&self.inner)
    }

    /// Extract underlying filter (exclusive).
    /// Returns the inner filter if this is the only remaining reference,
    /// otherwise returns None.
    pub fn into_inner(self) -> Option<F> {
        Arc::try_unwrap(self.inner)
            .ok()
            .map(|rw| rw.into_inner().unwrap())
    }
}

// Implement specifically for types that implement a basic Bloom contract.
// We do this by expecting macro or custom trait, but here we just implement
// exact proxy methods if we wrapped standard filters.
// For better type erasure, we can proxy directly.
// To keep it simple, we implement it manually for the structures or rely on traits.

// Let's create a trait in lib.rs if needed, or simply duck type using generic constraints.
// For simplicity, we expose a direct read/write lock accessor.

impl<F> ConcurrentBloomFilter<F> {
    /// Execute a shared, read-only operation efficiently.
    pub fn read<R, OP: FnOnce(&F) -> R>(&self, op: OP) -> R {
        let guard = self.inner.read().expect("RwLock poisoned");
        op(&*guard)
    }

    /// Execute an exclusive, modifying operation.
    pub fn write<R, OP: FnOnce(&mut F) -> R>(&self, op: OP) -> R {
        let mut guard = self.inner.write().expect("RwLock poisoned");
        op(&mut *guard)
    }
}
