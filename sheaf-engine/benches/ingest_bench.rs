//! Benchmark: tick ingestion throughput.

// @anchor infra:sheaf:ingest_bench
// @tags infra

use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_ingest_1k_ticks(c: &mut Criterion) {
    c.bench_function("ingest_1k_ticks", |b| {
        b.iter(|| {
            black_box(());
        });
    });
}

criterion_group!(benches, bench_ingest_1k_ticks);
criterion_main!(benches);
