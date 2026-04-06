use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use quickbloom::{AtomicBloomFilter, BloomFilter, BloomMode};
use std::sync::Arc;

const SIZE: usize = 1_000_000;
const HASHES: usize = 7;

fn bench_insert_standard(c: &mut Criterion) {
    let mut filter = BloomFilter::new(SIZE, HASHES);
    c.bench_function("insert/standard", |b| {
        b.iter(|| filter.insert(black_box(&"benchmark_string")))
    });
}

fn bench_insert_blocked(c: &mut Criterion) {
    let mut filter = BloomFilter::with_mode(SIZE, HASHES, BloomMode::Blocked);
    c.bench_function("insert/blocked", |b| {
        b.iter(|| filter.insert(black_box(&"benchmark_string")))
    });
}

fn bench_insert_atomic(c: &mut Criterion) {
    let filter = AtomicBloomFilter::new(SIZE, HASHES);
    c.bench_function("insert/atomic_lock_free", |b| {
        b.iter(|| filter.insert(black_box(&"benchmark_string")))
    });
}

fn bench_contains_standard(c: &mut Criterion) {
    let mut filter = BloomFilter::new(SIZE, HASHES);
    filter.insert(&"benchmark_string");
    c.bench_function("contains/standard", |b| {
        b.iter(|| filter.contains(black_box(&"benchmark_string")))
    });
}

fn bench_contains_blocked(c: &mut Criterion) {
    let mut filter = BloomFilter::with_mode(SIZE, HASHES, BloomMode::Blocked);
    filter.insert(&"benchmark_string");
    c.bench_function("contains/blocked", |b| {
        b.iter(|| filter.contains(black_box(&"benchmark_string")))
    });
}

fn bench_contains_atomic(c: &mut Criterion) {
    let filter = AtomicBloomFilter::new(SIZE, HASHES);
    filter.insert(&"benchmark_string");
    c.bench_function("contains/atomic_lock_free", |b| {
        b.iter(|| filter.contains(black_box(&"benchmark_string")))
    });
}

fn bench_concurrent_atomic(c: &mut Criterion) {
    let filter = Arc::new(AtomicBloomFilter::new(SIZE, HASHES));
    let mut group = c.benchmark_group("concurrent");
    for threads in [1usize, 4, 8].iter() {
        group.bench_with_input(BenchmarkId::new("atomic_threads", threads), threads, |b, &n| {
            b.iter(|| {
                let handles: Vec<_> = (0..n)
                    .map(|_| {
                        let f = Arc::clone(&filter);
                        std::thread::spawn(move || {
                            f.insert(black_box(&"concurrent_item"));
                        })
                    })
                    .collect();
                for h in handles {
                    h.join().unwrap();
                }
            })
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_insert_standard,
    bench_insert_blocked,
    bench_insert_atomic,
    bench_contains_standard,
    bench_contains_blocked,
    bench_contains_atomic,
    bench_concurrent_atomic,
);
criterion_main!(benches);
