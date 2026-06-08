# AD-0001 — Pure engine crate boundary: `prim-fmt` stays free of I/O

## Context

prim needs a formatting engine that external tools or future crates can consume
without pulling in a CLI dependency tree. It also needs `.editorconfig`
resolution, file discovery, and atomic writes — all I/O-heavy operations. The
question is where the boundary between the two sits.

A single crate containing both the engine and the CLI is the simplest package
structure, but it forces any library consumer to resolve `clap`, `yansi`, and
`ignore` unless all CLI code is feature-gated. A thin wrapper pattern (one lib
crate, one bin crate depending on it) is established practice and the pattern
used by the `driftsys/git-std` archetype.

## Options

**Single lib+bin package with feature flags.** The engine and CLI live together;
`prim-fmt` functionality is gated behind a default-off `cli` feature. Simpler
`Cargo.toml`; one fewer publish target. Drawback: feature flags are a
maintenance surface, and the feature boundary is easily eroded over time.

**Two crates: `prim-fmt` (lib) + `prim-cli` (bin).** The engine is a separate
package. `prim-cli` depends on `prim-fmt` and adds all CLI dependencies. Library
consumers get a lean dep tree at zero cost. Drawback: one extra `Cargo.toml` and
one extra `cargo publish` step on release.

**Monolith.** Engine and CLI together, no separation. Simplest for a tool that
will never be consumed as a library. Drawback: the maintainer anticipates other
crates consuming the engine; the split pays for itself immediately.

## Decision

The workspace uses two separate Cargo packages: `prim-fmt` (library) and
`prim-cli` (binary). `prim-fmt` must never depend on `clap`, `yansi`, `ignore`,
`ec4rs`, or any I/O or terminal crate. The crate boundary is the enforcement
mechanism; no feature flags are needed.

All I/O — `.editorconfig` reading (`ec4rs`), file discovery (`ignore`), atomic
writes (`tempfile`), terminal output (`yansi`) — lives exclusively in
`prim-cli`. The resolved `Style` struct lives in `prim-fmt` so the engine can
consume it without any I/O dependency; `prim-cli` constructs `Style` values and
passes them into `prim_fmt::format`.

A third crate, `spec` (test-only, never published), holds CLI snapshot tests and
install tests.

## Consequences

Per-format parsers (FR-1) belong in `prim-fmt` or in future `prim-*` sibling
library crates, never in `prim-cli`. If a parser ever needs I/O (unlikely for a
formatting library), that is a design smell to revisit explicitly.

The release pipeline publishes `prim-fmt` first, then `prim-cli`, because
`prim-cli` has a path-and-version dependency on `prim-fmt`.

---

Satisfies: FR-3 (style resolution placed in CLI; engine stays pure), NFR-1
(single static binary remains achievable when the lib is dep-free).\
Related: AD-0002 (editorconfig library choice), `docs/design/system.md`.
