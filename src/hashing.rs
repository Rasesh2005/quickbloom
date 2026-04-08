//! Hash utilities for Bloom filters.
//!
//! Uses enhanced double hashing with `ahash` to efficiently generate multiple
//! high-quality hash values from two independent base hashes.
//!
//! ## Algorithm
//! Given two base hashes `h1` and `h2`, the i-th hash is computed as:
//! ```text
//! h(i) = h1 + i * h2 + i^2   (enhanced double hashing)
//! ```
//! The quadratic term `i^2` reduces clustering that can occur in pure double
//! hashing at high fill ratios, improving the false-positive rate in practice.

use std::hash::Hash;

/// Generates Bloom filter hash indices using enhanced double hashing.
///
/// This struct is internal and optimized for speed and low collision rates.
pub(crate) struct HashGenerator {
    hash1: u64,
    hash2: u64,
}

impl HashGenerator {
    /// Create a new hash generator for a given item.
    #[inline]
    pub fn new<T: Hash>(item: &T) -> Self {
        let hash1 = Self::ahash(item, 0xdead_beef_cafe_babe_u128);
        let hash2 = Self::ahash(item, 0x0123_4567_89ab_cdef_u128);

        // Make hash2 odd and non-zero so the modular walk hits every slot.
        let hash2 = hash2 | 1;

        Self { hash1, hash2 }
    }

    /// Get the i-th hash value using enhanced double hashing.
    ///
    /// `h(i) = h1 + i*h2 + i^2` reduces bit clustering at high fill ratios
    /// compared to pure linear or double hashing.
    #[inline]
    pub fn nth(&self, i: usize) -> u64 {
        let i = i as u64;
        self.hash1
            .wrapping_add(i.wrapping_mul(self.hash2))
            .wrapping_add(i.wrapping_mul(i))
    }

    #[inline]
    fn ahash<T: Hash>(item: &T, seed: u128) -> u64 {
        // ahash::RandomState::with_seeds provides stable seeded hashing.
        let lo = seed as u64;
        let hi = (seed >> 64) as u64;
        let state = ahash::RandomState::with_seeds(lo, hi, lo ^ hi, lo.wrapping_add(hi));
        state.hash_one(item)
    }
}
