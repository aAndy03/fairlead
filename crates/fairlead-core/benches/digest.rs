use criterion::{Criterion, criterion_group, criterion_main};
use fairlead_core::context::effective_digest;
use fairlead_core::hash::ContentHash;

fn bench(c: &mut Criterion) {
    let own = ContentHash([7u8; 32]);
    c.bench_function("merkle_digest_100k", |b| {
        b.iter(|| {
            let mut parent = None;
            for _ in 0..100_000 {
                parent = Some(effective_digest(parent.as_ref(), &own, "0.1.0"));
            }
            parent
        })
    });
}

criterion_group!(benches, bench);
criterion_main!(benches);
