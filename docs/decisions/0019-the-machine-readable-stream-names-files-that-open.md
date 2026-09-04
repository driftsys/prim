# AD-0019 — The machine-readable stream names files that open

## Status

Accepted. Closes #172. Not breaking: a path that is valid UTF-8 renders exactly
as it did before, in every mode and every format.

## Context

On unix a filename is an arbitrary byte string. prim renders every reported path
through `Path::display`, which replaces each byte that is not valid UTF-8 with
U+FFFD. A file named `caf\xe9.md` therefore reaches stdout as the bytes
`63 61 66 EF BF BD 2E 6D 64` — U+FFFD where the name holds `E9`. The name prim
prints does not open.

AGENTS.md designates the `--check` file list on stdout as prim's
machine-readable output. A consumer of that stream — #172 gives the shape

```bash
prim fmt --check --since "$BASE" . | xargs -d '\n' some-tool
```

— receives a path that does not exist and fails with `ENOENT`. A SARIF report
uploaded to code scanning points at a file that is not in the tree, and a JSON
report names one no tool can open.

This became reachable in #168, which is what stopped such paths being silently
skipped. Before it, they were never selected or classified, so the lossy
rendering had nothing to render. Closing the silent skip was the larger
improvement; it converted an invisible problem into a visible wrong answer,
which is this record's subject. `FR-4.2e` named the limit and cited this issue
rather than fixing it.

## Options

1. **Refuse such a path in the reporting layer** with exit `2`. Honest, and it
   undoes #168: a file prim can format becomes a file prim fails on, which is
   the silent skip again with a louder noise.
2. **Keep the lossy rendering and add an exact channel to the JSON report
   only.** The plain-text list is the stream AGENTS.md designates as
   machine-readable, so the mode that most needs the fix would not get it.
3. **Percent-encode every path in every machine-readable stream.** One rule,
   unambiguous for a consumer, and no branch on decodability. It changes the
   output for every path containing a character outside the unreserved set — a
   space, `%`, any non-ASCII letter — so every existing consumer's paths change
   at once. That is a breaking change to output prim promises is stable, for the
   benefit of a case almost no repository has.
4. **Write the bytes where the stream can carry them, and encode only where it
   cannot.**

## Decision

Option 4, split by what each stream can represent.

**The plain-text stdout stream carries the bytes.** The `fmt --check` list and
the `path:line:col` prefix of a `lint` finding are written to a locked stdout as
the bytes the filesystem holds. The bytes are the name, so they are what a
consumer needs; nothing else round-trips.

**JSON and SARIF encode, and only when they must.** Their strings must be valid
UTF-8, so a path that is not cannot be written literally. The JSON report keeps
its lossy `path` — a reader still sees something — and gains `path_encoded`, the
bytes percent-encoded, present only for a path that is not valid UTF-8. A SARIF
`artifactLocation.uri` is percent-encoded in that same case, which is the form a
URI reference calls for anyway.

Encoding only the undecodable case is what makes this additive. Every path on a
platform whose filenames are Unicode, and every decodable path elsewhere,
produces byte-identical output to the previous release, which
`a_decodable_path_renders_exactly_as_before` pins in both formats.

**Human-facing output on stderr stays lossy.** It is prose for a reader, a
terminal cannot render an undecodable byte, and no tool parses it.

## Consequences

- **The path prim writes is the path that opens**, which
  `the_check_list_names_a_file_that_opens` pins by asserting the exact bytes and
  then reading the file back through them. The stream is still
  newline-delimited, so a consumer needs `xargs -d '\n'` rather than bare
  `xargs`, and a filename containing a newline — legal, and the reason prim
  reads git's output as bytes — cannot be expressed in it at all. This record
  does not add a NUL-separated mode.
- **A consumer of the JSON report must know about `path_encoded`.** One that
  does not sees exactly what it saw before, including the lossy `path`, so it is
  no worse off than it is today.
- **Percent-encoding is applied to a whole path or not at all**, which is the
  price of being additive. In JSON the presence of `path_encoded` is the marker,
  so there is no ambiguity. In SARIF there is none: a real file named
  `caf%E9.md` and a file named `caf\xe9.md` both emit `uri: "caf%E9.md"`, and a
  consumer that decodes resolves the first onto the wrong file. Encoding every
  uri unconditionally removes that ambiguity and is option 3, rejected because
  it changes the uri of every path holding a space or a non-ASCII character. The
  uri prim emits for a decodable path is unchanged by this record, including the
  pre-existing fact that it is not escaped.
- **A platform whose filenames are Unicode gets no exact form, and is offered
  none.** There a path that is not valid UTF-8 cannot be represented at all, so
  `path_bytes` returns `None` and no `path_encoded` appears. Encoding
  `Path::display`'s output there would percent-encode the U+FFFD this record
  exists to escape and promise a round-trip it could not keep. The byte-writing
  path is `#[cfg(unix)]`, matching the split `changed_files::decode` already
  draws for the same reason.
- **The test for this is Linux-only end to end.** A filename that is not valid
  UTF-8 cannot be created on APFS or HFS+, so the file-level test carries
  `#[cfg(target_os = "linux")]` and the rendering is unit-tested through
  `OsStr::from_bytes` everywhere.

## A limit this record does not close

Two other stdout writers still render a path through `Path::display`: the
unified diff's `---`/`+++` headers (`diff.rs`) and `prim explain`, which renders
its subject on the first line (`explain.rs`) and a `.editorconfig` path on every
settings line beneath it (`provenance::location_of`) — three lossy renderings,
not two. The diff header is the most serious of them: a patch naming a lossy
path applies to the wrong file, or to none.

Neither writer is in #172, which names the `--check` list, the plain-text lint
findings and the two report writers, so they are left out of this change rather
than widened into it. **No follow-up issue is filed yet**, which means the more
serious half of this defect class is currently recorded only here.
