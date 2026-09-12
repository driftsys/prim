#!/usr/bin/env bash
# Contract tests for deterministic release packaging.

ROOT="$(git rev-parse --show-toplevel)"
PACKAGE="$ROOT/tools/release/package.sh"

make_inputs() {
    local directory="$1"
    mkdir -p "$directory"
    printf '#!/usr/bin/env sh\nprintf prim\n' > "$directory/input-binary"
    chmod +x "$directory/input-binary"
    printf '.TH PRIM 1\n' > "$directory/prim.1"
}

verify_checksum() {
    local checksum="$1"
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum -c "$checksum"
    else
        shasum -a 256 -c "$checksum"
    fi
}

file_mode() {
    if [ "$(uname -s)" = "Darwin" ]; then
        stat -f '%Lp' "$1"
    else
        stat -c '%a' "$1"
    fi
}

test_packages_every_supported_target_with_the_canonical_asset_names() {
    local scratch target binary archive listing extracted
    scratch="$(mktemp -d)"
    make_inputs "$scratch/input"

    for target in \
        x86_64-unknown-linux-musl \
        aarch64-unknown-linux-musl \
        x86_64-apple-darwin \
        aarch64-apple-darwin \
        x86_64-pc-windows-msvc
    do
        mkdir -p "$scratch/$target"
        bash "$PACKAGE" "$target" "$scratch/input/input-binary" \
            "$scratch/input/prim.1" "$scratch/$target"
        archive="prim-$target.tar.gz"
        assert "[ -f '$scratch/$target/$archive' ]"
        assert "[ -f '$scratch/$target/$archive.sha256' ]"
        assert_equals "2" "$(find "$scratch/$target" -maxdepth 1 -type f | wc -l | tr -d ' ')"

        binary="prim"
        if [ "$target" = "x86_64-pc-windows-msvc" ]; then
            binary="prim.exe"
        fi
        listing="$(tar -tzf "$scratch/$target/$archive")"
        assert_equals "$binary
prim.1" "$listing"
        assert_equals "$archive" "$(awk '{print $2}' "$scratch/$target/$archive.sha256")"
        assert "cd '$scratch/$target' && verify_checksum '$archive.sha256'" \
            "checksum must verify when invoked from outside the repository"

        extracted="$scratch/$target/extracted"
        mkdir -p "$extracted"
        tar -xzf "$scratch/$target/$archive" -C "$extracted"
        assert "cmp '$scratch/input/input-binary' '$extracted/$binary'"
        assert "cmp '$scratch/input/prim.1' '$extracted/prim.1'"
        assert_equals "755" "$(file_mode "$extracted/$binary")"
        assert_equals "644" "$(file_mode "$extracted/prim.1")"
        rm -rf "$extracted"
    done

    rm -rf "$scratch"
}

test_packaging_normalizes_source_metadata_reproducibly() {
    local scratch target first second
    scratch="$(mktemp -d)"
    make_inputs "$scratch/input-one"
    make_inputs "$scratch/input-two"
    chmod 700 "$scratch/input-one/input-binary"
    chmod 777 "$scratch/input-two/input-binary"
    chmod 600 "$scratch/input-one/prim.1"
    chmod 666 "$scratch/input-two/prim.1"
    touch -t 202001020304 "$scratch/input-one/input-binary" "$scratch/input-one/prim.1"
    touch -t 202512302122 "$scratch/input-two/input-binary" "$scratch/input-two/prim.1"
    if command -v xattr >/dev/null 2>&1; then
        xattr -w com.driftsys.prim.release-test one "$scratch/input-one/input-binary"
        xattr -w com.driftsys.prim.release-test two "$scratch/input-two/input-binary"
    fi
    mkdir -p "$scratch/one" "$scratch/two"
    target="x86_64-unknown-linux-musl"

    bash "$PACKAGE" "$target" "$scratch/input-one/input-binary" \
        "$scratch/input-one/prim.1" "$scratch/one"
    bash "$PACKAGE" "$target" "$scratch/input-two/input-binary" \
        "$scratch/input-two/prim.1" "$scratch/two"
    first="$(awk '{print $1}' "$scratch/one/prim-$target.tar.gz.sha256")"
    second="$(awk '{print $1}' "$scratch/two/prim-$target.tar.gz.sha256")"
    assert_equals "$first" "$second"
    assert "cmp '$scratch/one/prim-$target.tar.gz' '$scratch/two/prim-$target.tar.gz'"

    rm -rf "$scratch"
}

test_packaging_rejects_an_unknown_target() {
    local scratch
    scratch="$(mktemp -d)"
    make_inputs "$scratch/input"
    assert_fails "bash '$PACKAGE' unknown-target '$scratch/input/input-binary' '$scratch/input/prim.1' '$scratch/output'"
    rm -rf "$scratch"
}

test_packaging_rejects_a_missing_binary_or_man_page() {
    local scratch target
    scratch="$(mktemp -d)"
    make_inputs "$scratch/input"
    target="x86_64-unknown-linux-musl"
    assert_fails "bash '$PACKAGE' '$target' '$scratch/missing' '$scratch/input/prim.1' '$scratch/output'"
    assert "[ ! -e '$scratch/output' ]"
    assert_fails "bash '$PACKAGE' '$target' '$scratch/input/input-binary' '$scratch/missing.1' '$scratch/output'"
    assert "[ ! -e '$scratch/output' ]"
    rm -rf "$scratch"
}

test_release_workflows_match_the_security_contract() {
    assert "ruby '$ROOT/spec/install/release_workflow_contract.rb'"
}

test_packager_has_no_network_or_publication_command() {
    local source
    source="$(< "$PACKAGE")"
    assert_not_matches '(^|[[:space:]])(curl|wget|gh[[:space:]]+release|cargo[[:space:]]+publish)([[:space:]]|$)' "$source"
}

test_packager_uses_sha256sum_with_exact_arguments_when_available() {
    local scratch hook target archive digest
    scratch="$(mktemp -d)"
    make_inputs "$scratch/input"
    mkdir -p "$scratch/output"
    hook="$scratch/bash-env"
    cat > "$hook" <<'EOF'
sha256sum() {
    [ "$#" -eq 1 ] || return 97
    digest="$(shasum -a 256 "$1" | awk '{print $1}')"
    printf '%s *%s\n' "$digest" "$1"
}
export -f sha256sum
EOF
    target="x86_64-unknown-linux-musl"
    BASH_ENV="$hook" bash "$PACKAGE" "$target" "$scratch/input/input-binary" \
        "$scratch/input/prim.1" "$scratch/output"
    archive="prim-$target.tar.gz"
    digest="$(shasum -a 256 "$scratch/output/$archive" | awk '{print $1}')"
    assert_equals "$digest  $archive" "$(< "$scratch/output/$archive.sha256")"
    rm -rf "$scratch"
}

test_packager_falls_back_to_sha256_shasum_with_exact_arguments() {
    local scratch hook target archive digest
    scratch="$(mktemp -d)"
    make_inputs "$scratch/input"
    mkdir -p "$scratch/output"
    hook="$scratch/bash-env"
    cat > "$hook" <<'EOF'
command() {
    if [ "$1" = "-v" ] && [ "${2:-}" = "sha256sum" ]; then
        return 1
    fi
    builtin command "$@"
}
shasum() {
    [ "$1" = "-a" ] && [ "$2" = "256" ] && [ "$#" -eq 3 ] || return 97
    digest="$(/usr/bin/shasum -a 256 "$3" | awk '{print $1}')"
    printf '%s *%s\n' "$digest" "$3"
}
export -f command shasum
EOF
    target="x86_64-unknown-linux-musl"
    BASH_ENV="$hook" bash "$PACKAGE" "$target" "$scratch/input/input-binary" \
        "$scratch/input/prim.1" "$scratch/output"
    archive="prim-$target.tar.gz"
    digest="$(shasum -a 256 "$scratch/output/$archive" | awk '{print $1}')"
    assert_equals "$digest  $archive" "$(< "$scratch/output/$archive.sha256")"
    rm -rf "$scratch"
}
