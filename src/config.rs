//! Bloom filter configuration helpers.

/// Configuration for creating a Bloom filter.
///
/// This allows users to specify expected number of items and
/// desired false positive probability instead of raw parameters.
#[derive(Debug, Clone)]
pub struct BloomConfig {
    /// Expected number of items to be inserted
    pub expected_items: usize,

    /// Desired false positive probability (e.g. 0.01 = 1%)
    pub false_positive_rate: f64,
}

impl BloomConfig {
    /// Create a new configuration.
    ///
    /// # Panics
    /// Panics if expected_items is 0, or false_positive_rate is not strictly between 0 and 1.
    pub fn new(expected_items: usize, false_positive_rate: f64) -> Self {
        assert!(expected_items > 0, "expected_items must be > 0");
        assert!(
            0.0 < false_positive_rate && false_positive_rate < 1.0,
            "false_positive_rate must be between 0 and 1"
        );

        Self {
            expected_items,
            false_positive_rate,
        }
    }

    /// Compute optimal Bloom filter size (number of bits).
    ///
    /// Formula:
    /// m = -(n * ln(p)) / (ln(2)^2)
    pub fn optimal_size(&self) -> usize {
        let n = self.expected_items as f64;
        let p = self.false_positive_rate;

        let m = -(n * p.ln()) / (2f64.ln().powi(2));
        m.ceil() as usize
    }

    /// Compute optimal number of hash functions.
    ///
    /// Formula:
    /// k = (m / n) * ln(2)
    pub fn optimal_hashes(&self) -> usize {
        let m = self.optimal_size() as f64;
        let n = self.expected_items as f64;

        let k = (m / n) * 2f64.ln();
        k.ceil().max(1.0) as usize
    }

    /// Convenience method returning both size and hashes.
    pub fn parameters(&self) -> (usize, usize) {
        (self.optimal_size(), self.optimal_hashes())
    }
}
