//! # quickbloom
//!
//! An industry-grade, highly scalable Bloom filter library for Rust.
//!
//! ## Architecture Overview
//!
//! | Type | Concurrency | Growth | Persistence |
//! |------|-------------|--------|-------------|
//! | [`BloomFilter`] | single-threaded (or wrap in [`ConcurrentBloomFilter`]) | fixed | ✅ via path |
//! | [`ScalableBloomFilter`] | single-threaded (or wrap in [`ConcurrentBloomFilter`]) | ✅ auto | ✅ via path |
//! | [`AtomicBloomFilter`] | ✅ fully lock-free | fixed | manual |
//!
//! ## Bloom Mode
//!
//! Standard `BloomFilter` supports two internal layouts selected via [`BloomMode`]:
//!
//! - **[`BloomMode::Standard`]** – classic flat bit-array layout.
//! - **[`BloomMode::Blocked`]** – partitions the bit array into 512-bit (64-byte)
//!   blocks. Every hash for a single item maps exclusively within one block,
//!   ensuring each `insert` and `contains` touches exactly **one CPU cache line**.
//!   This often doubles throughput on modern hardware.
//!
//! ## Feature Summary
//! - [`BloomConfig`] – mathematically optimal sizing from `(n, fp_rate)`.
//! - Fast `ahash`-based enhanced double hashing.
//! - Seamless auto-save/load via user-supplied file paths.
//! - CI-tested, `#![forbid(unsafe_code)]`.
//!
//! ## Example
//!
//! ```rust
//! use quickbloom::{BloomConfig, BloomMode, BloomFilter};
//!
//! // Size automatically computed for 1M items at 1% FP rate
//! let config = BloomConfig::new(1_000_000, 0.01);
//! let (size, hashes) = config.parameters();
//!
//! // Cache-friendly blocked layout
//! let mut filter = BloomFilter::with_mode(size, hashes, BloomMode::Blocked);
//! filter.insert(&"alice");
//! assert!(filter.contains(&"alice"));
//! assert!(!filter.contains(&"bob"));
//! ```

#![warn(missing_docs)]
#![forbid(unsafe_code)]

mod concurrent;
mod config;
mod hashing;
mod scalable;
mod storage;

use bitvec::prelude::*;
use std::hash::Hash;
use std::path::{Path, PathBuf};

use hashing::HashGenerator;

pub use concurrent::{AtomicBloomFilter, ConcurrentBloomFilter};
pub use config::BloomConfig;
pub use scalable::ScalableBloomFilter;

/// Specific bit vector type used internally.
pub type BloomBitVec = BitVec<u8, Lsb0>;

/// The internal memory layout used by a [`BloomFilter`].
///
/// Choosing the right mode can significantly impact throughput depending on
/// your hardware and workload pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BloomMode {
    /// Classic flat bit-array layout.
    ///
    /// All `k` hash positions are spread uniformly across the entire bit array.
    /// Memory accesses may span multiple cache lines per operation, causing
    /// cache-miss pressure at large filter sizes.
    #[default]
    Standard,

    /// Blocked (cache-friendly) layout.
    ///
    /// The filter is partitioned into 512-bit (64-byte) blocks that each fit
    /// in exactly one CPU cache line. When inserting or querying an item, the
    /// block is selected with a single hash, and all `k` bit positions are
    /// confined within that one block. This trades a tiny increase in
    /// false-positive rate for a large reduction in memory latency.
    ///
    /// **Recommended** for large filters (> 1 M bits) where throughput matters.
    Blocked,
}

/// Size of a blocked-mode block in bits (64 bytes × 8 bits = 512 bits).
const BLOCK_BITS: usize = 512;

/// A fast, standard Bloom filter with optional cache-friendly blocked layout
/// and automatic file-based persistence.
///
/// # Choosing a Layout
///
/// ```rust
/// use quickbloom::{BloomFilter, BloomMode};
///
/// // Classic – simple, uniform hashing
/// let mut standard = BloomFilter::new(100_000, 7);
///
/// // Cache-friendly – better throughput on large filters
/// let mut blocked = BloomFilter::with_mode(100_000, 7, BloomMode::Blocked);
/// ```
///
/// # Persistence
///
/// Attach a file path so the filter serialises itself automatically when
/// it goes out of scope:
///
/// ```no_run
/// use quickbloom::BloomFilter;
///
/// let mut f = BloomFilter::new(100_000, 7).with_persistence("my_filter.bin");
/// f.insert(&"hello");
/// // saved automatically on Drop
/// ```
#[derive(Debug, Clone)]
pub struct BloomFilter {
    /// Underlying bit vector storage.
    pub(crate) bits: BloomBitVec,
    /// Number of bits in the filter.
    pub(crate) size: usize,
    /// Number of hash functions to use.
    pub(crate) hashes: usize,
    /// Total number of items inserted so far.
    pub(crate) items: usize,
    /// The internal memory layout.
    pub(crate) mode: BloomMode,

    /// Optional file path for persistence.
    pub(crate) target_path: Option<PathBuf>,
    /// Flag to track if the filter has unsaved changes.
    pub(crate) needs_save: bool,
}

impl BloomFilter {
    /// Create a new Bloom filter using the [`BloomMode::Standard`] layout.
    pub fn new(size: usize, hashes: usize) -> Self {
        Self::with_mode(size, hashes, BloomMode::Standard)
    }

    /// Create a new Bloom filter with an explicit [`BloomMode`].
    ///
    /// Use [`BloomMode::Blocked`] for better cache locality on large filters.
    pub fn with_mode(size: usize, hashes: usize, mode: BloomMode) -> Self {
        assert!(size > 0, "Bloom filter size must be > 0");
        assert!(hashes > 0, "Number of hash functions must be > 0");

        // In blocked mode round up to the nearest full block.
        let actual_size = if mode == BloomMode::Blocked {
            size.div_ceil(BLOCK_BITS) * BLOCK_BITS
        } else {
            size
        };

        Self {
            bits: BloomBitVec::repeat(false, actual_size),
            size: actual_size,
            hashes,
            items: 0,
            mode,
            target_path: None,
            needs_save: true,
        }
    }

    /// Set an explicit persistence file target. Enables auto-saving on `Drop`.
    pub fn with_persistence<P: Into<PathBuf>>(mut self, path: P) -> Self {
        self.target_path = Some(path.into());
        self
    }

    /// Try to load a Bloom filter from `path`; create a fresh one if absent.
    ///
    /// The loaded filter is automatically bound to the same path, so subsequent
    /// `Drop` or `.save()` calls persist it there.
    pub fn load_or_new<P: AsRef<Path>>(path: P, size: usize, hashes: usize) -> Self {
        if let Some(mut existing) = storage::load(path.as_ref()) {
            existing.target_path = Some(path.as_ref().to_path_buf());
            existing
        } else {
            Self::new(size, hashes).with_persistence(path.as_ref().to_path_buf())
        }
    }

    /// Returns the fraction of bits currently set to 1.
    ///
    /// Used internally by [`ScalableBloomFilter`] to decide when to provision
    /// a new layer. A value above ~0.5 indicates the filter is saturating.
    pub fn fill_ratio(&self) -> f64 {
        self.bits.count_ones() as f64 / self.size as f64
    }

    /// Returns the active [`BloomMode`] of this filter.
    pub fn mode(&self) -> BloomMode {
        self.mode
    }

    /// Insert an item into the Bloom filter.
    ///
    /// After this call, [`contains`](Self::contains) is guaranteed to return
    /// `true` for the same item.
    #[inline]
    pub fn insert<T: Hash>(&mut self, item: &T) {
        let gen = HashGenerator::new(item);

        match self.mode {
            BloomMode::Standard => {
                for i in 0..self.hashes {
                    let idx = (gen.nth(i) % self.size as u64) as usize;
                    self.bits.set(idx, true);
                }
            }
            BloomMode::Blocked => {
                let num_blocks = self.size / BLOCK_BITS;
                // One hash selects the block; remaining hashes probe within it.
                let block = (gen.nth(0) % num_blocks as u64) as usize;
                let block_start = block * BLOCK_BITS;
                for i in 1..=self.hashes {
                    let bit_within = (gen.nth(i) % BLOCK_BITS as u64) as usize;
                    self.bits.set(block_start + bit_within, true);
                }
            }
        }

        self.items += 1;
        self.needs_save = true;
    }

    /// Check whether an item might be present in the filter.
    ///
    /// - Returns `false` → the item is **definitely** not present.
    /// - Returns `true`  → the item is **probably** present (false positives possible).
    #[inline]
    pub fn contains<T: Hash>(&self, item: &T) -> bool {
        let gen = HashGenerator::new(item);

        match self.mode {
            BloomMode::Standard => {
                for i in 0..self.hashes {
                    let idx = (gen.nth(i) % self.size as u64) as usize;
                    if !self.bits[idx] {
                        return false;
                    }
                }
            }
            BloomMode::Blocked => {
                let num_blocks = self.size / BLOCK_BITS;
                let block = (gen.nth(0) % num_blocks as u64) as usize;
                let block_start = block * BLOCK_BITS;
                for i in 1..=self.hashes {
                    let bit_within = (gen.nth(i) % BLOCK_BITS as u64) as usize;
                    if !self.bits[block_start + bit_within] {
                        return false;
                    }
                }
            }
        }

        true
    }

    /// Manually persist state to the attached path, if any.
    ///
    /// Returns `Ok(())` if there is no path configured.
    pub fn save(&mut self) -> std::io::Result<()> {
        if let Some(ref path) = self.target_path {
            if self.needs_save {
                storage::save(self, path)?;
                self.needs_save = false;
            }
        }
        Ok(())
    }

    /// Number of items inserted so far.
    #[inline]
    pub fn len(&self) -> usize {
        self.items
    }

    /// Returns `true` if no items have been inserted.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.items == 0
    }
}

/// Automatically saves the filter when it drops, if a path is configured
/// and there are unsaved changes.
impl Drop for BloomFilter {
    fn drop(&mut self) {
        if self.needs_save && self.target_path.is_some() {
            let _ = self.save();
        }
    }
}
