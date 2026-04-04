//! # quickbloom
//!
//! A highly scalable, industry-grade Bloom filter with automatic persistence.
//!
//! ## Features
//! - Standard `BloomFilter`
//! - Fast growth-oriented `ScalableBloomFilter`
//! - Thread-safe `ConcurrentBloomFilter` wrapper
//! - Automatic or manual persistence serialization via explicitly defined paths

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

pub use concurrent::ConcurrentBloomFilter;
pub use config::BloomConfig;
pub use scalable::ScalableBloomFilter;

/// Specific bit vector type used internally.
pub type BloomBitVec = BitVec<u8, Lsb0>;

/// A fast, standard Bloom filter with automatic persistence capabilities.
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

    /// Optional file path for persistence.
    pub(crate) target_path: Option<PathBuf>,
    /// Flag to track if the filter has unsaved changes.
    pub(crate) needs_save: bool,
}

impl BloomFilter {
    /// Create a new Bloom filter.
    pub fn new(size: usize, hashes: usize) -> Self {
        assert!(size > 0, "Bloom filter size must be > 0");
        assert!(hashes > 0, "Number of hash functions must be > 0");

        Self {
            bits: BloomBitVec::repeat(false, size),
            size,
            hashes,
            items: 0,
            target_path: None,
            needs_save: true,
        }
    }

    /// Set an explicit persistence file target. Enables Auto-saving on `Drop`.
    pub fn with_persistence<P: Into<PathBuf>>(mut self, path: P) -> Self {
        self.target_path = Some(path.into());
        self
    }

    /// Try to load a Bloom filter from a specified path.
    /// If the file does not exist, initialize a new structured one tied to that path.
    pub fn load_or_new<P: AsRef<Path>>(path: P, size: usize, hashes: usize) -> Self {
        if let Some(mut existing) = storage::load(path.as_ref()) {
            existing.target_path = Some(path.as_ref().to_path_buf());
            existing
        } else {
            Self::new(size, hashes).with_persistence(path.as_ref().to_path_buf())
        }
    }

    /// Utility: fill_ratio used by scalable implementations
    pub fn fill_ratio(&self) -> f64 {
        self.bits.count_ones() as f64 / self.size as f64
    }

    /// Insert an item into the Bloom filter.
    #[inline]
    pub fn insert<T: Hash>(&mut self, item: &T) {
        let gen = HashGenerator::new(item);

        for i in 0..self.hashes {
            let idx = (gen.nth(i) % self.size as u64) as usize;
            self.bits.set(idx, true);
        }

        self.items += 1;
        self.needs_save = true;
    }

    /// Check whether an item might exist.
    #[inline]
    pub fn contains<T: Hash>(&self, item: &T) -> bool {
        let gen = HashGenerator::new(item);

        for i in 0..self.hashes {
            let idx = (gen.nth(i) % self.size as u64) as usize;
            if !self.bits[idx] {
                return false;
            }
        }

        true
    }

    /// Manually trigger a save to the attached persistence path.
    pub fn save(&mut self) -> std::io::Result<()> {
        if let Some(ref path) = self.target_path {
            if self.needs_save {
                storage::save(self, path)?;
                self.needs_save = false;
            }
        }
        Ok(())
    }

    /// Number of inserted items.
    #[inline]
    pub fn len(&self) -> usize {
        self.items
    }

    /// Returns `true` if the Bloom filter is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.items == 0
    }
}

/// Automatically save modified bits when the Bloom filter is dropped.
impl Drop for BloomFilter {
    fn drop(&mut self) {
        if self.needs_save && self.target_path.is_some() {
            let _ = self.save();
        }
    }
}
