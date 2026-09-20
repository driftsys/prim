#!/usr/bin/env bash
# Tests for the release artifact installation smoke test.
#
# Run with: bash tools/bash_unit spec/install/smoke_test.sh

ROOT="$(git rev-parse --show-toplevel)"
SMOKE="$ROOT/tools/release/smoke-test.sh"

make_archive() {
    local directory="$1"
    mkdir -p "$directory/source"
    cat > "$directory/source/prim" <<'EOF'
#!/usr/bin/env sh
if [ "$#" -ne 1 ] || [ "$1" != "--version" ]; then
    exit 64
fi
printf 'prim 0.9.0-smoke path=%s\n' "$0"
EOF
    chmod 644 "$directory/source/prim"
    printf '.TH PRIM 1\n' > "$directory/source/prim.1"
    tar -czf "$directory/prim-test.tar.gz" -C "$directory/source" prim prim.1
    (cd "$directory" && sha256sum prim-test.tar.gz > prim-test.tar.gz.sha256)
}

test_smoke_test_verifies_extracts_and_runs_from_path() {
    local scratch output
    scratch="$(mktemp -d)"
    make_archive "$scratch"

    output="$(bash "$SMOKE" "$scratch/prim-test.tar.gz" "$scratch/prim-test.tar.gz.sha256")"
    assert_matches 'prim 0\.9\.0-smoke path=.*/install/prim' "$output"

    rm -rf "$scratch"
}

test_smoke_test_rejects_an_archive_without_a_binary() {
    local scratch
    scratch="$(mktemp -d)"
    mkdir -p "$scratch/source"
    printf 'not a binary\n' > "$scratch/source/README"
    tar -czf "$scratch/prim-test.tar.gz" -C "$scratch/source" README
    (cd "$scratch" && sha256sum prim-test.tar.gz > prim-test.tar.gz.sha256)

    assert_fails "bash '$SMOKE' '$scratch/prim-test.tar.gz' '$scratch/prim-test.tar.gz.sha256'"

    rm -rf "$scratch"
}

test_smoke_test_rejects_a_tampered_archive() {
    local scratch
    scratch="$(mktemp -d)"
    make_archive "$scratch"
    printf tampered >> "$scratch/prim-test.tar.gz"

    assert_fails "bash '$SMOKE' '$scratch/prim-test.tar.gz' '$scratch/prim-test.tar.gz.sha256'"

    rm -rf "$scratch"
}

test_smoke_test_rejects_a_checksum_for_another_file() {
    local scratch
    scratch="$(mktemp -d)"
    make_archive "$scratch"
    cp "$scratch/prim-test.tar.gz" "$scratch/other.tar.gz"
    (cd "$scratch" && sha256sum other.tar.gz > prim-test.tar.gz.sha256)

    assert_fails "bash '$SMOKE' '$scratch/prim-test.tar.gz' '$scratch/prim-test.tar.gz.sha256'"

    rm -rf "$scratch"
}

test_smoke_test_rejects_a_checksum_with_extra_entries() {
    local scratch
    scratch="$(mktemp -d)"
    make_archive "$scratch"
    printf '%s  %s\n' "$(awk '{print $1}' "$scratch/prim-test.tar.gz.sha256")" other.tar.gz >> "$scratch/prim-test.tar.gz.sha256"

    assert_fails "bash '$SMOKE' '$scratch/prim-test.tar.gz' '$scratch/prim-test.tar.gz.sha256'"

    rm -rf "$scratch"
}
