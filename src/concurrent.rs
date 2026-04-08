//! Thread-safe concurrent wrapper for Bloom filters.
//!
//! Provides two concurrency strategies selectable at construction time:
//!
//! - **Lock-based** (`ConcurrentBloomFilter::new`): Wraps any filter in an
//!   `Arc<RwLock>`. Simple and correct but can contend under heavy write load.
//!
//! - **Lock-free** ([`AtomicBloomFilter`]): **[BETA]** Uses a flat array of
//!   `AtomicU8` bytes. Multiple threads can insert and query simultaneously
//!   without ever blocking.

use std::hash::Hash;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, RwLock};

use crate::hashing::HashGenerator;

// ─────────────────────────────────────────────────────────────────────────────
// Lock-based wrapper (wraps any F)
// ─────────────────────────────────────────────────────────────────────────────

/// A thread-safe wrapper around any Bloom filter using `Arc<RwLock>`.
///
/// # When to use
/// Use this when you need scalable growth or automatic persistence alongside
/// concurrent access. Reads are shared; writes obtain an exclusive lock.
///
/// # Example
/// ```rust
/// use quickbloom::{BloomFilter, ConcurrentBloomFilter};
///
/// let filter = BloomFilter::new(10_000, 7);
/// let cf = ConcurrentBloomFilter::new(filter);
///
/// let cf2 = cf.clone();
/// std::thread::spawn(move || {
///     cf2.write(|f| f.insert(&"hello"));
/// }).join().unwrap();
///
/// assert!(cf.read(|f| f.contains(&"hello")));
/// ```
#[derive(Clone)]
pub struct ConcurrentBloomFilter<F> {
    inner: Arc<RwLock<F>>,
}

impl<F> ConcurrentBloomFilter<F> {
    /// Wrap an existing Bloom filter in a thread-safe `Arc<RwLock>`.
    pub fn new(filter: F) -> Self {
        Self {
            inner: Arc::new(RwLock::new(filter)),
        }
    }

    /// Returns the strong reference count of the underlying `Arc`.
    pub fn ref_count(&self) -> usize {
        Arc::strong_count(&self.inner)
    }

    /// Execute a shared, read-only operation on the inner filter.
    ///
    /// Multiple threads may call `read` concurrently.
    pub fn read<R, OP: FnOnce(&F) -> R>(&self, op: OP) -> R {
        let guard = self.inner.read().expect("RwLock poisoned");
        op(&*guard)
    }

    /// Execute an exclusive, modifying operation on the inner filter.
    ///
    /// Blocks all readers and other writers until the closure returns.
    pub fn write<R, OP: FnOnce(&mut F) -> R>(&self, op: OP) -> R {
        let mut guard = self.inner.write().expect("RwLock poisoned");
        op(&mut *guard)
    }

    /// Extract the inner filter if this is the only remaining `Arc` reference.
    ///
    /// Returns `None` if other clones still exist.
    pub fn into_inner(self) -> Option<F> {
        Arc::try_unwrap(self.inner)
            .ok()
            .map(|rw| rw.into_inner().unwrap())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Lock-free AtomicBloomFilter
// ─────────────────────────────────────────────────────────────────────────────

/// A fully lock-free, thread-safe Bloom filter backed by `AtomicU8` bytes.
///
/// > [!IMPORTANT]
/// > **BETA PHASE**: This type is currently in active development.
///
/// All `insert` and `contains` calls use relaxed atomic operations so many
/// threads can operate simultaneously without ever blocking or locking.
///
/// # Limitations
/// - Fixed capacity: the size is determined at construction and cannot grow.
/// - No built-in persistence (save the underlying bytes manually if needed).
/// - Uses `Relaxed` ordering, which is sufficient for Bloom filters since a
///   missed recent insert only causes false negatives, which Bloom filters
///   already disallow by design – making the filter slightly conservative.
///
/// # When to use
/// Use this in tight hot paths (e.g. cache layer, rate-limiter) where maximum
/// throughput is required and exact synchronization is not critical.
///
/// # Example
/// ```rust
/// use quickbloom::AtomicBloomFilter;
/// use std::sync::Arc;
///
/// let filter = Arc::new(AtomicBloomFilter::new(100_000, 7));
/// let f2 = Arc::clone(&filter);
///
/// std::thread::spawn(move || {
///     f2.insert(&"rasesh");
/// }).join().unwrap();
///
/// assert!(filter.contains(&"rasesh"));
/// ```
pub struct AtomicBloomFilter {
    /// Bit storage as an array of atomic bytes.
    bytes: Vec<AtomicU8>,
    /// Total number of bits (logical size of the filter).
    size: usize,
    /// Number of hash functions per item.
    hashes: usize,
}

impl AtomicBloomFilter {
    /// Create a new lock-free Bloom filter.
    ///
    /// # Panics
    /// Panics if `size` or `hashes` is zero.
    pub fn new(size: usize, hashes: usize) -> Self {
        assert!(size > 0, "AtomicBloomFilter size must be > 0");
        assert!(hashes > 0, "Number of hash functions must be > 0");

        // Round up to the nearest byte boundary.
        let byte_count = size.div_ceil(8);
        let bytes = (0..byte_count).map(|_| AtomicU8::new(0)).collect();

        Self {
            bytes,
            size,
            hashes,
        }
    }

    /// Create a lock-free filter from a [`BloomConfig`](crate::BloomConfig).
    ///
    /// This is the recommended way to size the filter correctly.
    pub fn from_config(config: &crate::BloomConfig) -> Self {
        let (size, hashes) = config.parameters();
        Self::new(size, hashes)
    }

    /// Insert an item. Safe to call from any number of threads simultaneously.
    #[inline]
    pub fn insert<T: Hash>(&self, item: &T) {
        let gen = HashGenerator::new(item);
        for i in 0..self.hashes {
            let bit_idx = (gen.nth(i) % self.size as u64) as usize;
            let byte_idx = bit_idx / 8;
            let bit_mask = 1u8 << (bit_idx % 8);
            // Relaxed: we only need the bit to be set eventually, not ordered.
            self.bytes[byte_idx].fetch_or(bit_mask, Ordering::Relaxed);
        }
    }

    /// Check whether an item might be present. Safe to call concurrently.
    ///
    /// Returns `false` if the item is definitely absent.
    /// Returns `true` if the item is probably present.
    #[inline]
    pub fn contains<T: Hash>(&self, item: &T) -> bool {
        let gen = HashGenerator::new(item);
        for i in 0..self.hashes {
            let bit_idx = (gen.nth(i) % self.size as u64) as usize;
            let byte_idx = bit_idx / 8;
            let bit_mask = 1u8 << (bit_idx % 8);
            if self.bytes[byte_idx].load(Ordering::Relaxed) & bit_mask == 0 {
                return false;
            }
        }
        true
    }

    /// Estimated fill ratio (fraction of bits set to 1).
    ///
    /// Useful for monitoring saturation. When this exceeds ~0.5, false-positive
    /// rates rise quickly.
    pub fn fill_ratio(&self) -> f64 {
        let set: u64 = self
            .bytes
            .iter()
            .map(|b| b.load(Ordering::Relaxed).count_ones() as u64)
            .sum();
        let total_bits = self.bytes.len() * 8;
        set as f64 / total_bits as f64
    }

    /// Returns the logical size of the filter in bits.
    pub fn size(&self) -> usize {
        self.size
    }

    /// Returns the number of hash functions used.
    pub fn hashes(&self) -> usize {
        self.hashes
    }
}
