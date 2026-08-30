# AD-0017 — The exit-code contract survives a panic and an undecodable argv

## Status

Accepted. Closes #125 and #173. Not breaking: both changes replace exit `101`,
which was never a value prim promised, with codes it already documents.

## Context

FR-5.6 states prim's exit codes as a contract: `0` nothing to do, `1`
actionable, `2` prim could not do its job. AGENTS.md repeats it, and every
consumer prim has — a CI gate, a pre-commit hook, an editor — reads that number
and nothing else.

Two routes left the contract entirely, and both landed on `101`, the code the
Rust runtime uses for an unwinding panic.

**An argument that is not valid UTF-8 (#173).** `main` collected the command
line with `std::env::args()`, which panics on exactly that. Every entry point
carrying a path reproduced it:

```console
$ prim fmt --check $'\xe9bad.txt'
thread 'main' panicked at .../env.rs:876
$ echo $?
101
```

`prim explain`, `--stdin-filepath`, `--exclude` and `--since` reproduced it too.
A hook of the shape `prim fmt "$@"` — the shape of both recipes prim ships —
panicked rather than formatting or skipping.

This also left FR-2.5 half true. That requirement says a file whose name is not
valid UTF-8 is owned and formatted exactly as a decodable name would be. prim
delivered that through recursive discovery and through `--since`/`--staged`, and
not through the most direct route a caller has: naming the file.

**A panic inside a dependency (#125).** prim formats through a rayon pool and
`grep -rn catch_unwind crates/` returned nothing, so one panicking worker took
the whole process to `101` — and the files beside it produced no output either.
Two dependency `debug_assert!` panics had already been found and silenced one at
a time with `[profile.dev.package.*] debug-assertions = false` entries
(AD-0006), which is per-package maintenance with no general protection. The next
round is already visible: `dprint-plugin-json` 0.22.0 carries three live
`#[cfg(debug_assertions)]` position assertions.

## Decision

1. prim reads its command line with `std::env::args_os()`. The argv preprocessor
   takes `OsString`.
2. A token the preprocessor cannot decode ends the scan for a verb, exactly as
   an unrecognised flag does. Every verb and every flag prim scans for is ASCII,
   so a token that will not decode is none of them — it is a path, and the first
   path is where the scan stops anyway.
3. A panic inside a third-party formatter **or linter** is caught per file,
   reported with the path named, and the file is left byte-for-byte unchanged.
   The run continues to the next file, and the process exits `2`. The linter is
   not an afterthought: `rumdl` is a larger body of code than the dprint
   formatters, and containing only the formatters left `prim lint` exiting `101`
   while `prim fmt` was safe.
4. A panic is always an **error**, never a warning, whether the file was named
   or reached by a walk. This is where it departs from an unparseable file,
   which is a warning when walked: an unparseable file is the caller's input,
   while a panic is prim's own bug, and a walk passing over prim's bugs quietly
   is how the next one goes unnoticed.
5. The routes that hold a buffer prim does not own return it unchanged.
   `--stdin-filepath` echoes the editor's buffer back to stdout and exits `2`;
   the LSP returns no edits, no diagnostics, and the session survives. Emptying
   an editor buffer because a formatter panicked would be a worse outcome than
   the panic. A `--format` run still emits its document, because `--format`
   changes stdout alone: a pipeline should read a well-formed empty report with
   the failure carried by the exit code, not an empty stream that parses as a
   failure of its own.
6. Containment is written once, generic over the operation, and used at every
   call prim makes into a third-party formatter or linter: the per-file walk,
   the idempotence second pass, the two `--stdin-filepath` routes, the LSP
   formatting handler, the hygiene and Markdown lint calls on the path route,
   the same two on the stdin route, and the same two in the LSP's diagnostics.

Point 3 is what #125 asked to have settled before any code: whether a panicking
file is reported and skipped while the walk continues. The fail-safe rule
already answers it. FR-6.3 says an unparseable file is left byte-for-byte
unchanged and reported as an error, and a file prim panicked on is a file prim
could not process — the cause differs, the disposition does not.

Point 6 is what makes the contract hold rather than the one call site #125
named. A guarantee about exit codes that covers most entry points is not a
guarantee, and the one left out is always the one a user finds — as the first
round of this change showed. It contained `prim_fmt::format` and left
`prim_fmt::lint_markdown` bare, so `prim lint`, `prim lint --format json`,
`prim lint --stdin-filepath` and an LSP `didOpen` on a Markdown document all
still exited `101`, the last of them killing the editor's server outright.

The default panic hook is left in place, so the panic's own message and any
backtrace still reach stderr. That output is alarming, and it is also the only
diagnostic anyone gets: prim's own line names the file and asks for a report,
which is what makes the pair actionable. Silencing the hook would trade the
single thing a bug report needs for a tidier terminal.

## Alternatives considered

- **`[profile.dev.package."*"] debug-assertions = false`.** Ends the per-package
  additions in one line. Rejected in AD-0006 and again here: it silences
  assertions in dependencies that have never misfired, and it does nothing for a
  panic that is not a debug assertion. It treats the symptom prim has met rather
  than the contract prim promises.
- **Catch the panic at the top of `main` instead of per file.** One call site
  and a correct exit code. Rejected: it cannot name the file, and it abandons
  every other file in the run — which is the second half of what #125 reports,
  not just the exit code.
- **Let `--stdin-filepath` print nothing on a panic.** Simpler. Rejected under
  point 5: an editor replaces its buffer with prim's stdout, so printing nothing
  empties the document.
- **Decode argv lossily** (`to_string_lossy`) instead of carrying `OsString`.
  Rejected: prim would then act on a path that is not the one it was given, and
  `\u{FFFD}` in a filename is a path that almost certainly does not exist. #172
  covers prim _reporting_ such a path lossily, which is a display question and a
  separate one.

## Consequences

- `prim fmt "$@"` in a hook survives a filename that is not valid UTF-8, on
  every entry point rather than only through discovery.
- FR-2.5's guarantee is now delivered through naming a path as well, so its
  "#173 does not yet work" caveat goes. The caveat about lossy _reporting_
  (#172) stays: that is untouched here.
- A panicking file no longer takes its neighbours with it. A run over a thousand
  files where one panics now formats the other 999 and exits `2`.
- Exit `101` remains reachable in principle — a panic outside the contained
  region, an abort, a stack overflow. What changed is that the two routes anyone
  has actually reached it through no longer lead there.
- Inputs that panic **are** known: AD-0006 records two, pinned by
  `crates/prim-fmt/src/markdown.rs`, and they stay quiet only because of the
  `[profile.dev.package.*] debug-assertions = false` overrides that record
  describes. Reaching them from a test would mean building without those
  overrides, which is a build prim does not otherwise make, so the containment
  carries a fault injector instead: `PRIM_PANIC_INJECT` panics inside the
  contained region for any path containing its value, and it is compiled out of
  every release binary (`#[cfg(debug_assertions)]`). Matching a substring rather
  than a flag is what lets a test say "this file panics, its neighbours do not",
  which is the half of #125 about the run continuing.
- `AssertUnwindSafe` asserts only that nothing **prim** observes afterwards is
  inconsistent. Inside the dependencies it is not true: `dprint-core`'s `format`
  increments a thread-local count and decrements it only on the success path,
  with no drop guard, so an unwind leaves that thread's count above zero and its
  bump allocator never reset again. That is a leak in the worker, not wrong
  output and not undefined behaviour — the count can only be left too high, and
  too high suppresses the reset. It is bounded by the process for `prim fmt`,
  and by the session for `prim lsp`, which point 5 deliberately keeps alive.
  Accepted: a leaked arena in a worker that has already hit a dependency bug
  costs less than the exit code did.
- Building the workspace with `-C panic=abort` turns the containment into a
  no-op and produces a SIGABRT, again outside the contract. prim sets no such
  profile and ships none; a downstream packager who does gives up this
  guarantee.

---

Satisfies: #125 and #173; removes FR-2.5's #173 caveat and adds FR-5.6a and
FR-5.6b. Related: AD-0006 (the per-package debug-assertion overrides this
generalises), FR-5.6, FR-6.3, `crates/prim-cli/src/formatting.rs`,
`crates/prim-cli/src/argv.rs`, `crates/prim-cli/src/main.rs`.
