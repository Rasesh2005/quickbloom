# quickbloom

![Crates.io](https://img.shields.io/crates/v/quickbloom.svg)
![License](https://img.shields.io/badge/license-MIT-blue.svg)
![GitHub Workflow Status](https://img.shields.io/github/actions/workflow/status/Rasesh2005/quickbloom/rust.yml)

`quickbloom` is a high-performance Bloom filter library for Rust. Highlights:

- **ahash**: Optimized hashing for short keys.
- **Enhanced double hashing**: Uses `h(i) = h1 + i*h2 + i^2` to minimize bit clustering.
- **Blocked Layout**: Partitions the filter into 64-byte blocks for cache locality.
- **Lock-free writes**: `AtomicBloomFilter` for multi-threaded ingestion without locks.
- **Auto-Scaling**: `ScalableBloomFilter` adds layers as it fills to maintain false-positive rates.
- **Persistence**: Built-in binary serialization that triggers on `Drop`.

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
quickbloom = "0.2.0"
```

## Quick Start

```rust
use quickbloom::{BloomConfig, BloomFilter, BloomMode};

// Optimal size for 1M items @ 1% false-positive rate
let config = BloomConfig::new(1_000_000, 0.01);
let (size, hashes) = config.parameters();

let mut filter = BloomFilter::with_mode(size, hashes, BloomMode::Blocked);
filter.insert(&"alice");

assert!(filter.contains(&"alice"));
assert!(!filter.contains(&"bob"));
```

## Implementation Options

| Type | Best for | Thread-safe? | Grows? | Persistence |
|---|---|---|---|---|
| `BloomFilter` | General use cases | No | No | Yes |
| `ScalableBloomFilter` | Unknown data sizes | No | Yes | Yes |
| `ConcurrentBloomFilter` | Read-heavy concurrent access | Yes | Optional | Optional |
| `AtomicBloomFilter` | Write-heavy ingestion | Yes (Beta) | No | Manual |

## Layout Modes

`BloomFilter` supports two memory layouts via `BloomMode`:

- **Standard**: Uniformly distributed hashes. Use for small filters.
- **Blocked**: Probes stay within a single 512-bit (64-byte) cache line. Recommended for large filters to avoid cache misses.

```rust
let filter = BloomFilter::with_mode(size, hashes, BloomMode::Blocked);
```

## Scaling & Persistence

`ScalableBloomFilter` provision new layers when the fill ratio exceeds 0.5. To persist a filter to disk, provide a path:

```rust
{
    let mut f = BloomFilter::new(100_000, 7).with_persistence("data.bin");
    f.insert(&"data");
} // Saved to data.bin on drop
```

Load existing data using `load_or_new`:

```rust
let f = BloomFilter::load_or_new("data.bin", 100_000, 7);
```

## Atomic Bloom Filter (BETA)

> [!WARNING]
> `AtomicBloomFilter` is currently in **Beta**. While functionally complete and high-performance, the API and internal representation may evolve. Use with caution in critical production paths.

`AtomicBloomFilter` uses `AtomicU8` segments for lock-free insertion.

```rust
use std::sync::Arc;
use quickbloom::AtomicBloomFilter;

let filter = Arc::new(AtomicBloomFilter::new(500_000, 7));
// Multiple threads can call .insert() safely
```

### Monitoring saturation

You can estimate the current fill ratio to monitor for saturation:

```rust
println!("fill ratio: {:.2}", filter.fill_ratio());
```

When `fill_ratio > 0.5`, the false-positive rate begins to rise significantly.

---

## Hashing Details

Indices are derived using **ahash** and **enhanced double hashing**:

```
h(i) = h1 + i * h2 + i^2
```

The quadratic term `i^2` reduces bit-clustering at high fill ratios compared to linear double hashing.

## Binary File Format (v3)

`quickbloom` uses a custom versioned format for persistence:

- **Header**: 2 bytes (`[version][type]`)
- **Filter Data**: `[mode: u8][size: u64][hashes: u64][items: u64][raw_bytes...]`
- **Scalable Filters**: Parameters, growth factor, followed by each layer's data.

Files from older versions are ignored to prevent corruption; a fresh filter is created instead.

## Benchmarks

Measurements taken on **Apple M3 Pro** (1M items, 1M bits, 7 hashes):

| Crate | Mode | Avg Latency (Insert) |
| :--- | :--- | :--- |
| **quickbloom v0.2.0** | Blocked (Cache-Line) | **13.92 ns** |
| **quickbloom v0.2.0** | Standard | **15.75 ns** |
| `fastbloom v0.9.0` | Standard | 18.45 ns |
| `fastbloom v0.9.0` | Blocked (512-bit) | 18.77 ns |

---

## Running Benchmarks & Examples

To run the internal benchmark suite:
```sh
cargo bench
```

To run the included example:
```sh
cargo run --example basic_usage
```

## Authors

Rasesh Shetty

## License

MIT — see [LICENSE](LICENSE)
