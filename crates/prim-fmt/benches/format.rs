use criterion::{Criterion, criterion_group, criterion_main};

fn bench_noop(c: &mut Criterion) {
    c.bench_function("noop", |b| b.iter(|| 1 + 1));
}

criterion_group!(benches, bench_noop);
criterion_main!(benches);
