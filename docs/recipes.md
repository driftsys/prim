# Recipes

## CI formatting gate

Fail the build when any tracked file is not formatted:

```yaml
- name: Check formatting
  run: prim --check .
```

`prim --check` writes nothing, exits `0` when everything is already formatted,
and exits `1` (listing the offending files) otherwise.

## Editor format-on-save

Point your editor's "format with external command" hook at:

```bash
prim --stdin-filepath "$FILE"
```

prim reads the buffer on stdin and writes the formatted result to stdout. The
path is used only to select the right formatter.

## Excluding files

prim respects `.gitignore` and `.ignore` automatically. To exclude a **tracked**
file from formatting (for example a deliberately malformed test fixture, or a
generated `CHANGELOG.md`), add it to a committed `.primignore` using gitignore
syntax:

```gitignore
# .primignore
CHANGELOG.md
fixtures/malformed.json
```

## Using prim with git-std

`git-std` generates `CHANGELOG.md`, which prim would otherwise hard-wrap as
Markdown. In repositories using both tools, add `CHANGELOG.md` to `.primignore`
(prim ships this entry by default).
