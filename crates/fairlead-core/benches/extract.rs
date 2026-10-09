use criterion::{Criterion, criterion_group, criterion_main};
use fairlead_syntax::{CommentTable, ExtractConfig, extract};

fn bench(c: &mut Criterion) {
    let cfg = ExtractConfig {
        marker: "fairlead".into(),
        comments: CommentTable::builtin(),
        header_skip: Vec::new(),
        max_file: 2_000_000,
        max_directives: 500,
    };
    let text = "export const v = 1;\n".repeat(500) + "// fairlead: use tok.color.primary\n";
    c.bench_function("extract_500_lines", |b| {
        b.iter(|| extract(text.as_bytes(), "ts", &cfg))
    });
}

criterion_group!(benches, bench);
criterion_main!(benches);
