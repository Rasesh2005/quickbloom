//! Hash utilities for Bloom filters.
//!
//! Uses double hashing to efficiently generate multiple hash values
//! from two base hashes.

use std::hash::{Hash, Hasher};
use twox_hash::XxHash64;

/// Generates Bloom filter hash indices using double hashing.
///
/// This struct is internal and optimized for speed and determinism.
pub(crate) struct HashGenerator {
    hash1: u64,
    hash2: u64,
}

impl HashGenerator {
    /// Create a new hash generator for a given item.
    #[inline]
    pub fn new<T: Hash>(item: &T) -> Self {
        let hash1 = Self::hash_with_seed(item, 0);
        let hash2 = Self::hash_with_seed(item, 1);

        // Ensure hash2 is non-zero to avoid duplicate indices
        let hash2 = if hash2 == 0 { 1 } else { hash2 };

        Self { hash1, hash2 }
    }

    /// Get the i-th hash value.
    #[inline]
    pub fn nth(&self, i: usize) -> u64 {
        self.hash1.wrapping_add((i as u64).wrapping_mul(self.hash2))
    }

    #[inline]
    fn hash_with_seed<T: Hash>(item: &T, seed: u64) -> u64 {
        let mut hasher = XxHash64::with_seed(seed);
        item.hash(&mut hasher);
        hasher.finish()
    }
}
