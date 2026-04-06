# quickbloom

![Crates.io](https://img.shields.io/crates/v/quickbloom.svg)
![License](https://img.shields.io/badge/license-MIT-blue.svg)
![GitHub Workflow Status](https://img.shields.io/github/actions/workflow/status/Rasesh2005/quickbloom/rust.yml)

**`quickbloom`** is an industry-grade Bloom filter library for Rust featuring:

- 🚀 **`ahash`** — fastest non-cryptographic hasher for short keys
- 🧠 **Enhanced double hashing** — `h(i) = h1 + i·h2 + i²` reduces bit clustering at high fill ratios
- 🗂️ **Blocked (cache-friendly) layout** — all `k` hashes for one item stay inside a single 64-byte CPU cache line
- ⚛️ **Lock-free `AtomicBloomFilter`** — insert and query from any number of threads simultaneously with zero locking
- 📈 **`ScalableBloomFilter`** — grows automatically when saturated; false-positive rate never deteriorates
- 💾 **Automatic persistence** — attach a file path and the filter saves itself on `Drop`

---

## Installation

```toml
[dependencies]
quickbloom = "0.2.0"
```

---

## Quick Start

```rust
use quickbloom::{BloomConfig, BloomFilter, BloomMode};

// Mathematically optimal size for 1M items at 1% false-positive rate
let config = BloomConfig::new(1_000_000, 0.01);
let (size, hashes) = config.parameters();

let mut filter = BloomFilter::with_mode(size, hashes, BloomMode::Blocked);

filter.insert(&"alice");
assert!(filter.contains(&"alice")); // true  – definitely present
assert!(!filter.contains(&"bob")); // false – definitely absent
```

---

## Types at a Glance

| Type | Best for | Concurrency | Grows? | Persistence |
|---|---|---|---|---|
| [`BloomFilter`](#bloomfilter) | Single-threaded, general use | No | No | ✅ |
| [`ScalableBloomFilter`](#scalablebloomfilter) | Unbounded / unknown data sets | No (wrap in `ConcurrentBloomFilter`) | ✅ | ✅ |
| [`ConcurrentBloomFilter`](#concurrentbloomfilter) | Read-heavy multi-thread workloads | `RwLock` | Depends on `F` | Depends on `F` |
| [`AtomicBloomFilter`](#atomicbloomfilter) | Write-heavy lock-free hot paths | Atomic | No | Manual |

---

## `BloomFilter`

The standard single-threaded Bloom filter. Supports two memory layouts:

### Standard layout (default)

Hash positions are spread uniformly across the entire bit array. Simple and accurate.

```rust
use quickbloom::BloomFilter;

let mut f = BloomFilter::new(100_000, 7);
f.insert(&"hello");
assert!(f.contains(&"hello"));
```

### Blocked layout (cache-friendly)

All `k` hashes for one item are confined to a single 64-byte block (CPU cache line). On large filters this often **doubles throughput** with a negligible increase in false-positive rate.

```rust
use quickbloom::{BloomFilter, BloomMode};

let mut f = BloomFilter::with_mode(1_000_000, 7, BloomMode::Blocked);
f.insert(&"cache_friendly");
```

### Persistence

Attach a path to enable automatic saving when the filter is dropped:

```rust
use quickbloom::BloomFilter;

{
    let mut f = BloomFilter::new(100_000, 7).with_persistence("my_filter.bin");
    f.insert(&"persist_me");
} // ← auto-saved here

// Load it back next time:
let f = BloomFilter::load_or_new("my_filter.bin", 100_000, 7);
assert!(f.contains(&"persist_me"));
```

You can also call `.save()` manually to flush at any point:

```rust,no_run
f.save().expect("I/O error");
```

### Sizing with `BloomConfig`

Instead of guessing raw bit counts, use `BloomConfig` to compute optimal parameters:

```rust
use quickbloom::BloomConfig;

let config = BloomConfig::new(500_000, 0.005); // 500K items, 0.5% FP rate
let (bits, hashes) = config.parameters();
println!("bits={bits}, hashes={hashes}");
```

---

## `ScalableBloomFilter`

Automatically provisions new layers when the current one exceeds 50% fill, preserving the configured false-positive rate as items grow without bound.

```rust
use quickbloom::{BloomConfig, ScalableBloomFilter};

let config = BloomConfig::new(1_000, 0.01);
let mut f = ScalableBloomFilter::new(config);

for i in 0..5_000u64 {
    f.insert(&i);
}

println!("layers: {}", f.layers()); // > 1 once the first layer fills
assert!(f.contains(&0u64));
assert!(f.contains(&4_999u64));
```

### Custom growth parameters

```rust
use quickbloom::{BloomConfig, ScalableBloomFilter};

let config = BloomConfig::new(1_000, 0.01);
let f = ScalableBloomFilter::with_parameters(
    config,
    0.85, // tightening ratio – each new layer's FP target is multiplied by this
    4,    // growth factor   – each new layer is 4× larger than the previous
);
```

### Persistence

```rust,no_run
use quickbloom::{BloomConfig, ScalableBloomFilter};

let config = BloomConfig::new(1_000, 0.01);

{
    let mut f = ScalableBloomFilter::new(config.clone())
        .with_persistence("scalable.bin");
    for i in 0..2_000u64 { f.insert(&i); }
} // ← auto-saved

let f = ScalableBloomFilter::load_or_new("scalable.bin", config);
assert!(f.contains(&0u64));
```

---

## `ConcurrentBloomFilter`

A generic `Arc<RwLock<F>>` wrapper. Use it when you need concurrent access to a filter that also supports persistence or dynamic scaling.

```rust
use quickbloom::{BloomFilter, ConcurrentBloomFilter};
use std::sync::Arc;

let f = ConcurrentBloomFilter::new(BloomFilter::new(100_000, 7));

// Share across threads
let f2 = f.clone();
std::thread::spawn(move || {
    f2.write(|inner| inner.insert(&"from_thread"));
}).join().unwrap();

assert!(f.read(|inner| inner.contains(&"from_thread")));
```

> **Tip:** For write-heavy workloads at high concurrency, prefer [`AtomicBloomFilter`](#atomicbloomfilter) which has zero lock overhead.

---

## `AtomicBloomFilter`

A fully lock-free Bloom filter backed by `AtomicU8` bytes. Any number of threads can `insert` and `contains` simultaneously without blocking.

```rust
use quickbloom::{AtomicBloomFilter, BloomConfig};
use std::sync::Arc;

let config = BloomConfig::new(500_000, 0.01);
let filter = Arc::new(AtomicBloomFilter::from_config(&config));

let handles: Vec<_> = (0..8)
    .map(|i| {
        let f = Arc::clone(&filter);
        std::thread::spawn(move || {
            f.insert(&format!("thread_{}", i));
        })
    })
    .collect();

for h in handles { h.join().unwrap(); }

assert!(filter.contains(&"thread_0".to_string()));
assert!(filter.contains(&"thread_7".to_string()));
```

### Monitoring saturation

```rust,no_run
println!("fill ratio: {:.2}", filter.fill_ratio());
// When fill_ratio > 0.5, false-positive rate rises quickly.
// At that point, create a new AtomicBloomFilter with a larger size.
```

---

## Benchmarking

Run the included benchmarks to compare the three approaches on your hardware:

```sh
cargo bench
```

The benchmark suite measures:
- `insert/standard` vs `insert/blocked` vs `insert/atomic_lock_free`
- `contains/standard` vs `contains/blocked` vs `contains/atomic_lock_free`
- `concurrent/atomic_threads` at 1, 4, and 8 threads

---

## Running Examples

```sh
cargo run --example basic_usage
```

---

## Running Tests

```sh
cargo test
```

---

## Hashing Details

`quickbloom` uses **ahash** with seeded `RandomState` for the two base hashes, and **enhanced double hashing** for index derivation:

```
h(i) = h1 + i × h2 + i²
```

The quadratic term `i²` eliminates the bit-clustering that can appear in pure double hashing when fill ratios exceed ~30%, resulting in a lower real-world false-positive rate.

---

## File Format

The binary persistence format is versioned (`v3`). Files saved by an older version of `quickbloom` will not load in `v3` (a `None` is returned and a fresh filter is created). The format stores:
- A 2-byte header: `[version][type]`
- For each filter: `[mode][size][hashes][items][raw_byte_count][raw_bytes…]`
- For scalable filters: config parameters, growth factors, then each layer as above.

---

## Authors

Rasesh Shetty

## License

MIT — see [LICENSE](LICENSE)
