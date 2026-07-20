# `.editorconfig` custom keys + glob sections spike (#41)

Proves prim can read namespaced `prim_*` keys from `.editorconfig`, per matching
glob section, with EditorConfig's precedence — the resolver mechanism story C1
(#46) and the G3/G4 strict-glob map (#59/#60) depend on. Output is the resolver
test in `crates/prim-cli/src/editorconfig.rs` (`SPIKE #41` block).

## What was proven

- **`ec4rs` already exposes custom keys.** prim's existing dependency
  (`ec4rs = "1.2"`) resolves _unknown_ keys per section and returns them via
  `Properties::get_raw_for_key(key) -> &RawValue`. No new dependency, no
  hand-rolled parser.
- **Precedence is EditorConfig's own last-match-wins**, and it applies to custom
  keys identically to standard ones. prim's real cascade (`build_cascade` +
  `apply`) carries the custom keys through untouched; `use_fallbacks()` only
  fills standard-key defaults.
- **The G4 placement map resolves as intended.** For

  ```ini
  root = true
  [*.md]
  prim_mdlint_strict = false
  [docs/**.md]
  prim_mdlint_strict = true
  [**/SUMMARY.md]
  prim_mdlint_strict = false
  ```

  | file              | matches                                     | `prim_mdlint_strict`   |
  | ----------------- | ------------------------------------------- | ---------------------- |
  | `README.md`       | `[*.md]`                                    | `false` (floor)        |
  | `docs/guide.md`   | `[*.md]`, `[docs/**.md]`                    | `true` (strict)        |
  | `docs/SUMMARY.md` | `[*.md]`, `[docs/**.md]`, `[**/SUMMARY.md]` | `false` (SUMMARY-safe) |

- **Nearer configs override farther ones** for custom keys (cascade last-wins).
- **Unknown keys are fail-safe**: a `prim_*` key alongside standard keys does
  not disturb `Style` resolution; unset keys resolve to `None`.

## The recipe C1 will use

EditorConfig has **no specificity ranking** — "more specific wins" is achieved
by authoring the narrower section _later_ in the file (which G4's `prim init`
scaffolds). Reading a `prim_*` value for a path:

```rust
let props = apply(&cascade, &path);        // prim's real per-file resolution
props
    .get_raw_for_key("prim_mdlint_strict") // EditorConfig lowercases keys
    .into_option()                         // None when unset
    .map(|v| v.eq_ignore_ascii_case("true"));
```

## Notes for story C1 (#46)

- **Where it landed.** G3 established the right boundary: formatting keys still
  resolve into `prim_fmt::Style`, while lint-only `prim_*` keys resolve
  separately in `crates/prim-cli/src/editorconfig.rs`. C1 keeps that split and
  generalizes it with a shared private helper for documented boolean `prim_*`
  keys; `Style` does **not** grow lint-only fields.
- **Key set is a closed allowlist.** C1 documents the exact `prim_*` keys prim
  reads. Today that set contains only `prim_mdlint_strict`; anything else is
  ignored (proven fail-safe).
- **Scope decision recorded.** The AC's `prim_md_list_marker` text was
  illustrative, not a committed deliverable. C1 deliberately does not add that
  key because prim has no list-marker consumer today, and this story is about
  hardening the resolver pattern rather than inventing new behavior.
- **Keys are case-insensitive (lowercased by EditorConfig); parse values
  case-insensitively.** The spike compares with `eq_ignore_ascii_case`.
- **No AC change.** C1 and G3/G4 acceptance criteria hold as written; ec4rs
  supplies everything needed.
