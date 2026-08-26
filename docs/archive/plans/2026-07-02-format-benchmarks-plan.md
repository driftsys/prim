# Format Benchmarks Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `criterion`-based benchmark suite to `prim-fmt` that measures
formatting throughput per format (JSON, TOML, YAML, Markdown) across
small/medium/large synthetic inputs, runnable via `just bench`, so performance
regressions are visible over time.

**Architecture:** A single `benches/format.rs` binary (criterion's
`harness = false` convention) generates synthetic inputs of increasing size
programmatically (no vendored/network-fetched corpus — deterministic,
reproducible, licensing-free) and times `prim_fmt::format` per format/size via
`criterion::BenchmarkId` groups with byte-throughput reporting. Not wired into
CI-per-PR (too slow/noisy for a gate); run on demand via `just bench`.

**Tech Stack:** `criterion` (new dev-dependency, HTML reports feature), reusing
`prim-fmt`'s existing `Style`/`FileKind`/`format` public API.

## Global Constraints

- Zero warnings: `cargo bench` compiles clean, `clippy` stays clean (AGENTS.md
  "Conventions").
- `rustfmt` formatting on all new Rust files; run `just fmt` before committing.
- Conventional Commits, imperative mood.
- Single PR ships implementation + docs together.
- `prim-fmt` stays free of clap/CLI/terminal dependencies — `criterion` is a
  dev-dependency only, never a runtime dependency.
- Run `just verify` before considering the branch done (benches are not part of
  `just check`, so `just verify` alone won't catch a broken bench — Task 1's own
  steps are what verify it compiles and runs).

---

### Task 1: Wire `criterion` and a no-op bench target

**Files:**

- Modify: `crates/prim-fmt/Cargo.toml`
- Create: `crates/prim-fmt/benches/format.rs`

**Interfaces:**

- Produces: a `cargo bench -p prim-fmt` entry point named `format`.

- [ ] **Step 1: Add the dev-dependency and bench target**

In `crates/prim-fmt/Cargo.toml`, add to `[dev-dependencies]`:

```toml
criterion = { version = "0.5", features = ["html_reports"] }
```

And add a new section (after `[dev-dependencies]`):

```toml
[[bench]]
name = "format"
harness = false
```

- [ ] **Step 2: Write a trivial bench to prove wiring**

Create `crates/prim-fmt/benches/format.rs`:

```rust
use criterion::{Criterion, criterion_group, criterion_main};

fn bench_noop(c: &mut Criterion) {
    c.bench_function("noop", |b| b.iter(|| 1 + 1));
}

criterion_group!(benches, bench_noop);
criterion_main!(benches);
```

- [ ] **Step 3: Run it**

Run: `cargo bench -p prim-fmt` Expected: compiles, runs, prints a `noop` timing
report (sub-nanosecond), exits 0.

- [ ] **Step 4: Commit**

```bash
git add crates/prim-fmt/Cargo.toml crates/prim-fmt/benches/format.rs
git commit -m "chore(fmt): wire criterion benchmark target"
```

---

### Task 2: Synthetic corpus generators + real benchmarks

**Files:**

- Modify: `crates/prim-fmt/benches/format.rs` (replace the no-op bench)

**Interfaces:**

- Consumes: `prim_fmt::{FileKind, Style, format}`.

- [ ] **Step 1: Replace `format.rs` with the real benchmark**

```rust
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
        ("markdown", FileKind::Markdown, synthetic_markdown, &[10, 1_000, 20_000]),
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
```

- [ ] **Step 2: Run the full suite**

Run: `cargo bench -p prim-fmt` Expected: compiles and runs 12 benchmark groups
(4 formats × 3 sizes each), each printing a timing + throughput report; exits 0.
This will take a couple of minutes — criterion runs multiple sampling iterations
per case.

- [ ] **Step 3: Spot-check the HTML report**

Run: `open target/criterion/report/index.html` (macOS) or note the path for
manual inspection on other platforms. Expected: a report page listing all 12
benchmarks with violin plots. This confirms the `html_reports` feature is wired
correctly.

- [ ] **Step 4: Commit**

```bash
git add crates/prim-fmt/benches/format.rs
git commit -m "test(fmt): add per-format synthetic-corpus benchmarks"
```

---

### Task 3: `just bench` recipe

**Files:**

- Modify: `justfile`

**Interfaces:**

- Produces: `just bench` shell command.

- [ ] **Step 1: Add the recipe**

In `justfile`, add (near the other `test`/`check` recipes):

```just
# Run format benchmarks (not part of `just check` — slow, not CI-gated)
bench:
    cargo bench -p prim-fmt
```

- [ ] **Step 2: Run it**

Run: `just bench` Expected: same output as `cargo bench -p prim-fmt` in Task 2
Step 2.

- [ ] **Step 3: Commit**

```bash
git add justfile
git commit -m "chore: add just bench recipe"
```

---

### Task 4: Document how to run and interpret benchmarks

**Files:**

- Modify: `crates/prim-fmt/README.md`

- [ ] **Step 1: Add a "Benchmarks" section**

Append to `crates/prim-fmt/README.md`:

````markdown
## Benchmarks

`benches/format.rs` times `format()` per file kind (JSON, TOML, YAML,
Markdown) across small/medium/large synthetic inputs generated at bench time
(no vendored corpus — deterministic and reproducible). Run:

```bash
just bench
````

This is not part of `just check` or CI — it's slow and its numbers are
machine-dependent, so it's for local regression-hunting, not a gate. HTML
reports land in `target/criterion/report/index.html`. There is currently no
tracked performance baseline; treat a run before and after your change as the
comparison, not an absolute number.

````
- [ ] **Step 2: Verify the doc formats cleanly**

Run: `cargo run -q -p prim-cli -- --check crates/prim-fmt/README.md`
Expected: exit 0. If not, run `cargo run -q -p prim-cli -- crates/prim-fmt/README.md` to fix, then re-check.

- [ ] **Step 3: Run full verification and commit**

Run: `just verify`
Expected: PASS.

```bash
git add crates/prim-fmt/README.md
git commit -m "docs(fmt): document benchmark usage"
````
