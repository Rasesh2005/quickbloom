use criterion::{black_box, criterion_group, criterion_main, Criterion};
use quickbloom::BloomFilter;

fn bench_insert(c: &mut Criterion) {
    let mut filter = BloomFilter::new(100_000, 7);
    c.bench_function("insert_item", |b| {
        b.iter(|| {
            filter.insert(black_box(&"benchmark_string"));
        })
    });
}

fn bench_contains(c: &mut Criterion) {
    let mut filter = BloomFilter::new(100_000, 7);
    filter.insert(&"benchmark_string");
    c.bench_function("contains_item", |b| {
        b.iter(|| {
            filter.contains(black_box(&"benchmark_string"));
        })
    });
}

criterion_group!(benches, bench_insert, bench_contains);
criterion_main!(benches);
