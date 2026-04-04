//! Scalable Bloom filter implementation.
//!
//! Based on "Scalable Bloom Filters"
//! by Almeida et al. (2007).

use std::hash::Hash;
use std::path::PathBuf;

use crate::config::BloomConfig;
use crate::BloomFilter;

/// Default tightening ratio for false positives
const DEFAULT_TIGHTENING_RATIO: f64 = 0.9;

/// Default growth factor for capacity
const DEFAULT_GROWTH_FACTOR: usize = 2;

/// A scalable Bloom filter.
///
/// Automatically grows by adding new Bloom filters
/// when capacity is exceeded.
#[derive(Debug, Clone)]
pub struct ScalableBloomFilter {
    /// Ordered collection of standard Bloom filters.
    pub(crate) filters: Vec<BloomFilter>,
    /// Base configuration for the first layer.
    pub(crate) config: BloomConfig,
    /// Ratio by which FPP is tightened for each new layer.
    pub(crate) tightening_ratio: f64,
    /// Factor by which capacity grows for each new layer.
    pub(crate) growth_factor: usize,
    /// Optional file path for persistence.
    pub(crate) target_path: Option<PathBuf>,
    /// Flag to track if the filter has unsaved changes.
    pub(crate) needs_save: bool,
}

impl ScalableBloomFilter {
    /// Create a new scalable Bloom filter without persistence.
    pub fn new(config: BloomConfig) -> Self {
        Self::with_parameters(config, DEFAULT_TIGHTENING_RATIO, DEFAULT_GROWTH_FACTOR)
    }

    /// Create a scalable Bloom filter with custom parameters.
    pub fn with_parameters(
        config: BloomConfig,
        tightening_ratio: f64,
        growth_factor: usize,
    ) -> Self {
        assert!(
            0.0 < tightening_ratio && tightening_ratio < 1.0,
            "tightening_ratio must be between 0 and 1"
        );
        assert!(growth_factor >= 2, "growth_factor must be >= 2");

        let (size, hashes) = config.parameters();
        let filter = BloomFilter::new(size, hashes);

        Self {
            filters: vec![filter],
            config,
            tightening_ratio,
            growth_factor,
            target_path: None,
            needs_save: true,
        }
    }

    /// Attach a persistence path. This enables Auto-Saving on drop, or manual `.save()`.
    /// Does NOT load immediately, just sets the destination path.
    pub fn with_persistence<P: Into<PathBuf>>(mut self, path: P) -> Self {
        self.target_path = Some(path.into());
        self
    }

    /// Try to load a scalable Bloom filter from a specified path.
    /// If the file does not exist, initialize a new structured one tied to that path.
    pub fn load_or_new<P: AsRef<Path>>(path: P, config: BloomConfig) -> Self {
        if let Some(mut existing) = crate::storage::load_scalable(path.as_ref()) {
            existing.target_path = Some(path.as_ref().to_path_buf());
            existing
        } else {
            Self::new(config).with_persistence(path.as_ref().to_path_buf())
        }
    }

    /// Insert an item.
    pub fn insert<T: Hash>(&mut self, item: &T) {
        let last = self.filters.last_mut().unwrap();

        // If current filter is saturated (fill ratio > 50%), grow
        if last.fill_ratio() >= 0.5 {
            self.grow();
        }

        self.filters.last_mut().unwrap().insert(item);
        self.needs_save = true;
    }

    /// Check whether an item might exist.
    pub fn contains<T: Hash>(&self, item: &T) -> bool {
        // Check newest filters first
        for filter in self.filters.iter().rev() {
            if filter.contains(item) {
                return true;
            }
        }
        false
    }

    /// Number of Bloom filters currently in use.
    pub fn layers(&self) -> usize {
        self.filters.len()
    }

    /// Total number of items inserted.
    pub fn len(&self) -> usize {
        self.filters.iter().map(|f| f.len()).sum()
    }

    /// Returns `true` if empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Manually save to the persistence path, if configured.
    pub fn save(&mut self) -> std::io::Result<()> {
        if let Some(ref path) = self.target_path {
            if self.needs_save {
                crate::storage::save_scalable(self, path)?;
                self.needs_save = false;

                // Clear inner needs_save recursively
                for f in &mut self.filters {
                    f.needs_save = false;
                }
            }
        }
        Ok(())
    }

    // ---------- internal ----------

    fn grow(&mut self) {
        let layer = self.filters.len();

        let new_expected = (self.config.expected_items as f64
            * (self.growth_factor as f64).powi(layer as i32)) as usize;

        let new_fpp = self.config.false_positive_rate * self.tightening_ratio.powi(layer as i32);

        let cfg = BloomConfig::new(new_expected, new_fpp);
        let (size, hashes) = cfg.parameters();

        let filter = BloomFilter::new(size, hashes);
        self.filters.push(filter);
    }
}

/// Automatically save modified bits when dropped.
impl Drop for ScalableBloomFilter {
    fn drop(&mut self) {
        if self.needs_save && self.target_path.is_some() {
            let _ = self.save();
        }
    }
}
