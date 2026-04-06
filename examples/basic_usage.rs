use quickbloom::{AtomicBloomFilter, BloomConfig, BloomFilter, BloomMode, ScalableBloomFilter};
use std::path::Path;
use std::sync::Arc;

fn main() {
    println!("=== quickbloom v0.2.0 feature demo ===\n");

    // ── 1. Standard filter with persistence ──────────────────────────────────
    {
        let path = Path::new("demo_standard.bin");
        let mut filter = BloomFilter::new(10_000, 7).with_persistence(path);
        filter.insert(&"alice");
        filter.insert(&"bob");
        println!("[Standard] alice: {}", filter.contains(&"alice"));
        println!("[Standard] dave:  {}", filter.contains(&"dave"));
        filter.save().expect("save failed");
        println!("[Standard] saved to {}\n", path.display());
        let _ = std::fs::remove_file(path);
    }

    // ── 2. Blocked (cache-friendly) layout ───────────────────────────────────
    {
        let config = BloomConfig::new(500_000, 0.01);
        let (size, hashes) = config.parameters();
        let mut filter = BloomFilter::with_mode(size, hashes, BloomMode::Blocked);
        filter.insert(&"cache_friendly");
        println!("[Blocked] cache_friendly: {}", filter.contains(&"cache_friendly"));
        println!("[Blocked] fill ratio:     {:.4}\n", filter.fill_ratio());
    }

    // ── 3. Scalable filter with persistence ──────────────────────────────────
    {
        let path = Path::new("demo_scalable.bin");
        let config = BloomConfig::new(100, 0.01);
        let mut filter = ScalableBloomFilter::new(config).with_persistence(path);
        for i in 0..200u64 {
            filter.insert(&i);
        }
        println!(
            "[Scalable] layers: {}, items: {}",
            filter.layers(),
            filter.len()
        );
        println!("[Scalable] contains 42: {}", filter.contains(&42u64));
        filter.save().expect("save failed");
        println!("[Scalable] saved to {}\n", path.display());
        let _ = std::fs::remove_file(path);
    }

    // ── 4. Lock-free AtomicBloomFilter across threads ────────────────────────
    {
        let filter = Arc::new(AtomicBloomFilter::new(50_000, 7));
        let handles: Vec<_> = (0..4)
            .map(|i| {
                let f = Arc::clone(&filter);
                std::thread::spawn(move || {
                    f.insert(&format!("thread_{}", i));
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
        println!("[Atomic] thread_0 present: {}", filter.contains(&"thread_0".to_string()));
        println!("[Atomic] thread_3 present: {}", filter.contains(&"thread_3".to_string()));
        println!("[Atomic] fill ratio: {:.4}", filter.fill_ratio());
    }
}
