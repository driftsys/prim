//! What the per-directory `.editorconfig` cascade cache buys (AD-0002).
//!
//! The cache was added after measurement and the figure it was justified by
//! lived only in a commit message, so it could be neither re-checked nor
//! defended against regression (#158). This benchmark measures the mechanism:
//! resolving a whole tree through one reused [`Resolver`] against resolving
//! each file from a freshly parsed cascade.
//!
//! It does not reproduce the end-to-end `--check` figure AD-0002 quotes. That
//! number came from a full run, where resolution is one cost among many.

use std::path::PathBuf;

use criterion::{Criterion, Throughput, black_box, criterion_group, criterion_main};
use prim_cli::editorconfig;

/// A tree shaped like a repository: a root `.editorconfig`, a nested override
/// in some directories, and `files_per_dir` owned files in each of `dirs`
/// directories three levels down.
fn repository(dirs: usize, files_per_dir: usize) -> (tempfile::TempDir, Vec<PathBuf>) {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(
        root.path().join(".editorconfig"),
        "root = true\n\n[*]\nindent_style = space\nindent_size = 2\ninsert_final_newline = true\n\n[*.md]\nmax_line_length = 80\n",
    )
    .unwrap();

    let mut paths = Vec::new();
    for d in 0..dirs {
        let dir = root
            .path()
            .join(format!("crate_{d}"))
            .join("src")
            .join("module");
        std::fs::create_dir_all(&dir).unwrap();
        // A nested override in a quarter of the directories, so part of the
        // tree has a cascade more than one config deep.
        if d % 4 == 0 {
            std::fs::write(dir.join(".editorconfig"), "[*.json]\nindent_size = 4\n").unwrap();
        }
        for f in 0..files_per_dir {
            let path = dir.join(format!("file_{f}.json"));
            std::fs::write(&path, "{}\n").unwrap();
            paths.push(path);
        }
    }
    (root, paths)
}

fn resolution(c: &mut Criterion) {
    // 200 directories of 5 files. Enough directories for the per-directory
    // cache to matter, and few enough files that the uncached arm stays
    // inside criterion's default sampling budget.
    let (_root, paths) = repository(200, 5);

    let mut group = c.benchmark_group("editorconfig_resolution");
    group.throughput(Throughput::Elements(paths.len() as u64));

    // What a walk does: one resolver reused across every file it reaches.
    // `app::load` builds one per rayon worker, so this is the per-worker cost.
    group.bench_function("cached_per_directory", |b| {
        b.iter(|| {
            let mut resolver = editorconfig::Resolver::new();
            for path in &paths {
                black_box(resolver.resolve(path));
            }
        })
    });

    // What it cost before the cache, and what `--stdin-filepath` still does
    // for its single file: a cascade parsed afresh for every resolution.
    group.bench_function("uncached_per_file", |b| {
        b.iter(|| {
            for path in &paths {
                black_box(editorconfig::resolve(path));
            }
        })
    });

    group.finish();
}

criterion_group!(benches, resolution);
criterion_main!(benches);
