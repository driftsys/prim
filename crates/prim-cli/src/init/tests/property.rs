//! Property test for `merge` (story G4, issue #122).
//!
//! Three fixed regressions each pin one route to the defect `merge` and
//! `outcome` exist to rule out: after any write `prim init` makes, a
//! canonical glob must resolve to the value prim intended, or to what it
//! resolved to before, and never a third value. A fixed regression only
//! proves that route stays closed; it says nothing about routes nobody has
//! thought of yet. This generates thousands of candidate `.editorconfig`
//! files and checks the same property directly against `merge`'s real
//! oracle, `outcome::resolves_strict`.
//!
//! The three properties here are not redundant, and each was checked against
//! a deliberate defect. Disabling `safe_writes`'s outcome guard, so every
//! planned write is made whether or not it resolves as intended, fails the
//! resolution property and neither of the others. Inserting a missing section
//! at end-of-file instead of at its canonical bound fails idempotence and
//! neither of the others — the resolution property survives that one because
//! the outcome guard catches the bad placement and refuses the write, which
//! keeps resolution correct while leaving prim with work it will try again on
//! the next run.

use proptest::prelude::*;
use proptest::sample::select;

use super::super::map;
use super::super::merge;
use super::super::outcome::resolves_strict;
use super::super::sections;

/// The strict globs `merge` takes different code paths for: the default, the
/// whole-repository glob (which drops the floor section entirely), a
/// non-default mdBook `src`, and the two working-memory directories, which
/// each collapse their own exemption when the strict glob is that directory.
fn strict_glob() -> impl Strategy<Value = &'static str> {
    select(vec![
        "docs/**.md",
        "**.md",
        "guide/**.md",
        "docs/wip/**.md",
        "docs/archive/**.md",
    ])
}

/// A `prim_mdlint_strict` value: the two prim reads as a tier, common
/// near-miss casings that still read as one of them, one value prim reads as
/// neither (`banana`, which resolves to the floor tier), and an empty value —
/// which `ec4rs` rejects as an invalid line, so a section carrying it makes
/// the whole file unparseable.
fn key_value() -> impl Strategy<Value = String> {
    select(vec![
        "true".to_string(),
        "false".to_string(),
        "TRUE".to_string(),
        "False".to_string(),
        "banana".to_string(),
        String::new(),
    ])
}

/// One section header, in the shapes issue #117 exposed alongside the plain
/// form: interior whitespace inside the brackets, and a trailing comment.
fn header(glob: &'static str) -> impl Strategy<Value = String> {
    prop_oneof![
        Just(format!("[{glob}]")),
        Just(format!("[ {glob} ]")),
        Just(format!("[{glob}] # trailing comment")),
    ]
}

/// One section: a header, with or without the key prim cares about — a
/// keyless section is one prim would write the key into in place.
fn section(glob: &'static str) -> impl Strategy<Value = String> {
    (header(glob), proptest::option::of(key_value())).prop_map(|(header, value)| match value {
        Some(value) => format!("{header}\nprim_mdlint_strict = {value}\n"),
        None => format!("{header}\n"),
    })
}

/// Every glob a generated `.editorconfig` draws sections from: prim's own
/// canonical globs for this strict glob, so a candidate exercises real
/// placement decisions, plus foreign globs that overlap them (a bare `*.md`
/// or `**.md` a person wrote for reasons of their own, and the strict globs
/// of the other paths this run is not using), so the two interact the way a
/// person's own accumulated `.editorconfig` would make them interact.
fn glob_pool(strict_glob: &'static str) -> Vec<&'static str> {
    vec![
        "*.md",
        strict_glob,
        "docs/wip/**.md",
        "docs/archive/**.md",
        "**/SUMMARY.md",
        "*.txt",
        "*",
        "**.md",
        "docs/**.md",
        "guide/**.md",
    ]
}

/// A candidate `.editorconfig`: an optional top-level `root = true`, then a
/// random number of sections in random order, each glob drawn from
/// `glob_pool` independently — so duplicate sections happen the way a person
/// accumulating entries over time would produce them, and section order is
/// whatever the draw happens to land on rather than anything canonical.
fn editorconfig(strict_glob: &'static str) -> impl Strategy<Value = String> {
    let entry = select(glob_pool(strict_glob)).prop_flat_map(section);
    (proptest::bool::ANY, proptest::collection::vec(entry, 0..12)).prop_map(
        |(has_root, sections)| {
            let mut content = String::new();
            if has_root {
                content.push_str("root = true\n\n");
            }
            for section in sections {
                content.push_str(&section);
            }
            content
        },
    )
}

/// A strict glob paired with a candidate built for it, so the two vary
/// together rather than independently — `editorconfig` needs to know the
/// strict glob to place it in the pool of globs it draws from.
fn case() -> impl Strategy<Value = (&'static str, String)> {
    strict_glob().prop_flat_map(|glob| (Just(glob), editorconfig(glob)))
}

/// Whether `glob` already carries `prim_mdlint_strict` somewhere in `before`,
/// on any of its occurrences — the same condition `merge` itself checks
/// before ever planning a write for a spec. When it holds, prim never
/// attempts anything there: a person's own explicit choice for that glob is
/// legitimate and left exactly as written, silently, by design (see
/// `outcome::defeated_sections`). The one thing merge still checks for such a
/// glob is whether something else defeats the person's own choice for their
/// own probes — a genuinely different question from disagreeing with prim's
/// canonical value, and already covered by `warnings` when it happens.
fn already_explicit(before: &str, glob: &str) -> bool {
    let lines = sections::split_lines(before);
    let headers = sections::header_lines(&lines);
    sections::matching_sections(&lines, &headers, glob)
        .iter()
        .any(sections::SectionOccurrence::has_key)
}

/// Whether every line of `before` appears in `after`, in the same relative
/// order. `merge` only ever inserts whole lines — it never edits, deletes or
/// reorders one already there — so `before`'s lines must survive as a
/// subsequence of `after`'s.
fn is_line_subsequence(before: &str, after: &str) -> bool {
    let mut after_lines = after.lines();
    'before_lines: for line in before.lines() {
        for candidate in after_lines.by_ref() {
            if candidate == line {
                continue 'before_lines;
            }
        }
        return false;
    }
    true
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 512, ..ProptestConfig::default() })]

    /// After `merge`, every canonical glob resolves to the value prim
    /// intended or to what it resolved to before — never a third value —
    /// and prim never leaves a glob at a value it did not intend without
    /// saying so in `warnings`. When `before` does not parse at all, `merge`
    /// must refuse outright rather than guess.
    #[test]
    fn merge_never_leaves_a_canonical_glob_at_a_value_prim_neither_intended_nor_left_alone(
        (strict_glob, before) in case(),
    ) {
        let specs = map::canonical_specs(strict_glob);
        // The same probe `merge` itself checks parseability against: if this
        // does not resolve, none of the file does, and `merge` must refuse.
        let parse_probe = &specs[0].probes[0];

        let result = merge(&before, strict_glob);
        let after = &result.contents;

        if resolves_strict(&before, parse_probe).is_none() {
            prop_assert!(
                result.actions.is_empty(),
                "unparseable .editorconfig but merge planned actions {:?}\n{before}",
                result.actions,
            );
            prop_assert_eq!(
                after, &before,
                "unparseable .editorconfig but merge changed the file"
            );
            prop_assert!(
                !result.warnings.is_empty(),
                "unparseable .editorconfig but merge gave no warning\n{before}"
            );
            return Ok(());
        }

        for spec in &specs {
            let intended = spec.value;
            for probe in &spec.probes {
                let before_value = resolves_strict(&before, probe)
                    .expect("before parses: parse_probe already resolved");
                let after_value = resolves_strict(after, probe)
                    .expect("merge only writes text that still parses");
                let matches_intent = after_value == intended;
                let unchanged = after_value == before_value;

                prop_assert!(
                    matches_intent || unchanged,
                    "strict_glob {strict_glob:?}, glob [{}], probe {probe}: resolved to {after_value} \
                     after merge, but before it was {before_value} and prim intended {intended} — a \
                     value prim neither intended nor left alone\n--- before ---\n{before}\n--- after \
                     ---\n{after}",
                    spec.glob,
                );

                // A glob the person already gave an explicit value to before
                // merge ran is exempt: prim never touches it, so a value
                // that differs from prim's own intent is the person's
                // legitimate choice, not a decline — nothing for prim to
                // warn about (see `already_explicit`).
                if !matches_intent && !already_explicit(&before, spec.glob) {
                    prop_assert!(
                        !result.warnings.is_empty(),
                        "strict_glob {strict_glob:?}, glob [{}], probe {probe}: resolved to \
                         {after_value}, not the intended {intended}, with no warning\n--- before \
                         ---\n{before}\n--- after ---\n{after}",
                        spec.glob,
                    );
                }
            }
        }
    }

    /// `merge` never edits, deletes or reorders a line `before` already had —
    /// every change it makes is an insertion.
    #[test]
    fn merge_only_ever_inserts_lines((strict_glob, before) in case()) {
        let after = merge(&before, strict_glob).contents;
        prop_assert!(
            is_line_subsequence(&before, &after),
            "merge did not preserve before's lines as a subsequence of after's\n--- before \
             ---\n{before}\n--- after ---\n{after}"
        );
    }

    /// Merging prim's own output changes nothing: once a candidate resolves
    /// the way prim intends (or prim has said why it does not), running
    /// merge again must find nothing left to do.
    #[test]
    fn merge_is_idempotent((strict_glob, before) in case()) {
        let once = merge(&before, strict_glob);
        let twice = merge(&once.contents, strict_glob);
        prop_assert_eq!(
            &twice.contents, &once.contents,
            "merging merge's own output changed it further\n--- once ---\n{}\n--- twice \
             ---\n{}",
            once.contents, twice.contents,
        );
        prop_assert!(
            twice.actions.is_empty(),
            "merging merge's own output still had actions to make: {:?}\n{}",
            twice.actions, once.contents,
        );
    }
}
