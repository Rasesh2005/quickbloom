use quickbloom::{AtomicBloomFilter, BloomConfig, BloomFilter, BloomMode, ScalableBloomFilter};
use std::path::Path;
use std::sync::Arc;

#[test]
fn test_standard_insert_contains() {
    let mut filter = BloomFilter::new(5000, 5);
    filter.insert(&"hello");
    assert!(filter.contains(&"hello"));
    assert!(!filter.contains(&"world"));
}

#[test]
fn test_blocked_insert_contains() {
    let mut filter = BloomFilter::with_mode(5000, 5, BloomMode::Blocked);
    filter.insert(&"blocked_item");
    assert!(filter.contains(&"blocked_item"));
    assert!(!filter.contains(&"missing_item"));
}

#[test]
fn test_atomic_insert_contains() {
    let filter = AtomicBloomFilter::new(10_000, 7);
    filter.insert(&"atomic_item");
    assert!(filter.contains(&"atomic_item"));
    assert!(!filter.contains(&"not_here"));
}

#[test]
fn test_atomic_concurrent_insert() {
    let filter = Arc::new(AtomicBloomFilter::new(100_000, 7));
    let handles: Vec<_> = (0..8)
        .map(|i| {
            let f = Arc::clone(&filter);
            std::thread::spawn(move || {
                f.insert(&format!("item_{}", i));
            })
        })
        .collect();
    for h in handles {
        h.join().unwrap();
    }
    for i in 0..8 {
        assert!(filter.contains(&format!("item_{}", i)));
    }
}

#[test]
fn test_standard_persistence() {
    let path = Path::new("it_standard_persist.bin");
    if path.exists() {
        std::fs::remove_file(path).unwrap();
    }

    {
        let mut filter = BloomFilter::new(5000, 5).with_persistence(path);
        filter.insert(&"persist_me");
        filter.save().unwrap();
    }

    {
        let filter = BloomFilter::load_or_new(path, 5000, 5);
        assert!(filter.contains(&"persist_me"));
        assert!(!filter.contains(&"other"));
    }

    let _ = std::fs::remove_file(path);
}

#[test]
fn test_blocked_persistence() {
    let path = Path::new("it_blocked_persist.bin");
    if path.exists() {
        std::fs::remove_file(path).unwrap();
    }

    {
        let mut filter = BloomFilter::with_mode(5000, 5, BloomMode::Blocked).with_persistence(path);
        filter.insert(&"blocked_persist");
        filter.save().unwrap();
    }

    {
        let filter = BloomFilter::load_or_new(path, 5000, 5);
        assert!(filter.contains(&"blocked_persist"));
    }

    let _ = std::fs::remove_file(path);
}

#[test]
fn test_scalable_persistence() {
    let path = Path::new("it_scalable_persist.bin");
    if path.exists() {
        std::fs::remove_file(path).unwrap();
    }

    {
        let config = BloomConfig::new(500, 0.01);
        let mut filter = ScalableBloomFilter::new(config).with_persistence(path);
        for i in 0..600u64 {
            filter.insert(&i);
        }
        filter.save().unwrap();
    }

    {
        let config = BloomConfig::new(500, 0.01);
        let filter = ScalableBloomFilter::load_or_new(path, config);
        assert!(filter.contains(&0u64));
        assert!(filter.contains(&599u64));
    }

    let _ = std::fs::remove_file(path);
}
