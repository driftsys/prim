use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use prim_fmt::{FileKind, Style, format};

fn synthetic_json(items: usize) -> String {
    let mut s = String::from("{\n");
    for i in 0..items {
        s.push_str(&format!(
            "  \"key_{i}\": {{\"id\": {i}, \"value\": \"item number {i}\", \"active\": {}, \"tags\": [\"a\", \"b\", \"c\"]}},\n",
            i % 2 == 0
        ));
    }
    s.push_str("  \"__end\": true\n}\n");
    s
}

fn synthetic_yaml(items: usize) -> String {
    let mut s = String::new();
    for i in 0..items {
        s.push_str(&format!(
            "item_{i}:\n  id: {i}\n  value: item number {i}\n  active: {}\n  tags:\n    - a\n    - b\n",
            i % 2 == 0
        ));
    }
    s
}

fn synthetic_toml(items: usize) -> String {
    let mut s = String::new();
    for i in 0..items {
        s.push_str(&format!(
            "[item_{i}]\nid = {i}\nvalue = \"item number {i}\"\nactive = {}\ntags = [\"a\", \"b\", \"c\"]\n\n",
            i % 2 == 0
        ));
    }
    s
}

fn synthetic_markdown(paragraphs: usize) -> String {
    let mut s = String::new();
    for i in 0..paragraphs {
        s.push_str(&format!(
            "## Section {i}\n\nThis is paragraph number {i} with enough words in it to exercise the hard-wrap logic across multiple lines when the formatter re-flows prose text for width eighty.\n\n- point one about section {i}\n- point two about section {i}\n\n"
        ));
    }
    s
}

type Generator = fn(usize) -> String;

fn bench_format(c: &mut Criterion) {
    let style = Style::default();
    let cases: &[(&str, FileKind, Generator, &[usize])] = &[
        ("json", FileKind::Json, synthetic_json, &[10, 1_000, 50_000]),
        ("yaml", FileKind::Yaml, synthetic_yaml, &[10, 1_000, 50_000]),
        ("toml", FileKind::Toml, synthetic_toml, &[10, 1_000, 50_000]),
        (
            "markdown",
            FileKind::Markdown,
            synthetic_markdown,
            &[10, 1_000, 20_000],
        ),
    ];

    for (label, kind, generator, sizes) in cases {
        let mut group = c.benchmark_group(*label);
        for &size in *sizes {
            let input = generator(size);
            group.throughput(Throughput::Bytes(input.len() as u64));
            group.bench_with_input(BenchmarkId::from_parameter(size), &input, |b, input| {
                b.iter(|| format(*kind, black_box(input), black_box(&style)).expect("formats"));
            });
        }
        group.finish();
    }
}

criterion_group!(benches, bench_format);
criterion_main!(benches);
