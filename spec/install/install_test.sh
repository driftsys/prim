#!/usr/bin/env bash
# Tests for install.sh pure functions.
#
# Run with: bash tools/bash_unit spec/install/install_test.sh
#
# Sourcing install.sh loads the helper functions without executing main()
# because the guard at the bottom only runs main() when BASH_SOURCE[0] is
# unset (piped via stdin) or equals $0 (executed as a script).

# shellcheck source=../../install.sh
. "$(git rev-parse --show-toplevel)/install.sh"

# ── detect_target ─────────────────────────────────────────────────────────────

test_detect_linux_x86_64() {
    uname() { case "$1" in -s) echo "Linux" ;; -m) echo "x86_64" ;; esac }
    export -f uname
    assert_equals "x86_64-unknown-linux-musl" "$(detect_target)"
}

test_detect_linux_aarch64() {
    uname() { case "$1" in -s) echo "Linux" ;; -m) echo "aarch64" ;; esac }
    export -f uname
    assert_equals "aarch64-unknown-linux-musl" "$(detect_target)"
}

test_detect_macos_x86_64() {
    uname() { case "$1" in -s) echo "Darwin" ;; -m) echo "x86_64" ;; esac }
    export -f uname
    assert_equals "x86_64-apple-darwin" "$(detect_target)"
}

test_detect_macos_arm64() {
    uname() { case "$1" in -s) echo "Darwin" ;; -m) echo "arm64" ;; esac }
    export -f uname
    assert_equals "aarch64-apple-darwin" "$(detect_target)"
}

test_detect_unsupported_os_fails() {
    uname() { case "$1" in -s) echo "Windows_NT" ;; -m) echo "x86_64" ;; esac }
    export -f uname
    assert_fails "detect_target"
}

test_detect_unsupported_arch_fails() {
    uname() { case "$1" in -s) echo "Linux" ;; -m) echo "riscv64" ;; esac }
    export -f uname
    assert_fails "detect_target"
}

# ── URL construction ───────────────────────────────────────────────────────────
# Verify the download URL pattern matches actual release asset names.

test_release_asset_url_is_version_addressable_and_exact() {
    assert_equals \
        "https://github.com/driftsys/prim/releases/download/v1.2.3/prim-aarch64-apple-darwin.tar.gz" \
        "$(release_asset_url v1.2.3 aarch64-apple-darwin)"
}

test_release_checksum_url_adds_only_the_checksum_suffix() {
    local archive
    archive="$(release_asset_url v1.2.3 x86_64-unknown-linux-musl)"
    assert_equals \
        "https://github.com/driftsys/prim/releases/download/v1.2.3/prim-x86_64-unknown-linux-musl.tar.gz.sha256" \
        "$archive.sha256"
}

test_url_has_tar_gz_extension() {
    local target="x86_64-unknown-linux-musl"
    local version="v1.0.0"
    local url="https://github.com/driftsys/prim/releases/download/$version/prim-$target.tar.gz"
    assert_matches "\.tar\.gz$" "$url"
}

test_url_checksum_has_sha256_suffix() {
    local target="aarch64-apple-darwin"
    local version="v1.2.3"
    local url="https://github.com/driftsys/prim/releases/download/$version/prim-$target.tar.gz.sha256"
    assert_matches "\.tar\.gz\.sha256$" "$url"
}

test_url_contains_target_triple() {
    local target="x86_64-apple-darwin"
    local version="v0.1.0"
    local url="https://github.com/driftsys/prim/releases/download/$version/prim-$target.tar.gz"
    assert_matches "$target" "$url"
}

test_url_contains_version_tag() {
    local target="x86_64-unknown-linux-musl"
    local version="v0.1.0"
    local url="https://github.com/driftsys/prim/releases/download/$version/prim-$target.tar.gz"
    assert_matches "$version" "$url"
}

test_main_requests_the_exact_versioned_archive_and_checksum_urls() {
    local scratch target calls archive_dir
    scratch="$(mktemp -d)"
    target="x86_64-unknown-linux-musl"
    calls="$scratch/calls"
    archive_dir="$scratch/assets"
    mkdir -p "$archive_dir" "$scratch/input"
    printf '#!/usr/bin/env sh\nprintf prim\n' > "$scratch/input/prim"
    chmod +x "$scratch/input/prim"
    printf '.TH PRIM 1\n' > "$scratch/input/prim.1"
    bash "$(git rev-parse --show-toplevel)/tools/release/package.sh" \
        "$target" "$scratch/input/prim" "$scratch/input/prim.1" "$archive_dir"

    TEST_INSTALL_TARGET="$target"
    detect_target() { printf '%s\n' "$TEST_INSTALL_TARGET"; }
    install_completions() { :; }
    curl() {
        local argument output="" url=""
        while [ "$#" -gt 0 ]; do
            argument="$1"
            shift
            if [ "$argument" = "-o" ]; then
                output="$1"
                shift
            elif [[ "$argument" == https://* ]]; then
                url="$argument"
            fi
        done
        printf '%s\n' "$url" >> "$calls"
        case "$url" in
            */releases/latest)
                printf '{"tag_name":"v1.2.3"}\n'
                ;;
            *.tar.gz.sha256)
                cp "$archive_dir/prim-$target.tar.gz.sha256" "$output"
                ;;
            *.tar.gz)
                cp "$archive_dir/prim-$target.tar.gz" "$output"
                ;;
            *)
                return 1
                ;;
        esac
    }

    INSTALL_DIR="$scratch/install"
    PRIM_MAN_DIR="$scratch/man"
    SHELL="/unknown"
    main >/dev/null

    assert_equals \
        "https://api.github.com/repos/driftsys/prim/releases/latest
https://github.com/driftsys/prim/releases/download/v1.2.3/prim-$target.tar.gz
https://github.com/driftsys/prim/releases/download/v1.2.3/prim-$target.tar.gz.sha256" \
        "$(< "$calls")"
    rm -rf "$scratch"
}

# ── piped invocation (curl | bash) ────────────────────────────────────────────
# Regression guard: when the script is fed via stdin (the documented
# `curl … | bash` install path), BASH_SOURCE[0] is unset. Combined with
# `set -u` this must not fail with "BASH_SOURCE[0]: unbound variable" before
# main() ever runs.

test_piped_invocation_does_not_fail_on_unbound_bash_source() {
    local script
    script="$(git rev-parse --show-toplevel)/install.sh"

    # Replace main() with a no-op so the test does not perform a real install.
    local stderr
    stderr="$(sed 's/^main() {$/main() { return 0; }\n_orig_main() {/' "$script" \
        | bash 2>&1 >/dev/null)"

    assert_equals "" "$stderr" \
        "piped invocation produced bash errors: $stderr"
}

# ── sha256_check ──────────────────────────────────────────────────────────────

test_sha256_check_valid_file_passes() {
    local tmp
    tmp="$(mktemp -d)"
    echo "test archive content" > "$tmp/archive.tar.gz"
    (
        cd "$tmp"
        if command -v sha256sum >/dev/null 2>&1; then
            sha256sum archive.tar.gz > archive.tar.gz.sha256
        else
            shasum -a 256 archive.tar.gz > archive.tar.gz.sha256
        fi
        sha256_check archive.tar.gz.sha256
    )
    rm -rf "$tmp"
}

test_sha256_check_prefers_sha256sum_with_exact_arguments() {
    local calls
    calls="$(mktemp)"
    SHA256_CALLS="$calls"
    sha256sum() { printf '%s\n' "$*" > "$SHA256_CALLS"; }
    shasum() { return 99; }

    sha256_check archive.tar.gz.sha256
    unset -f sha256sum shasum

    assert_equals "-c archive.tar.gz.sha256" "$(< "$calls")"
    rm -f "$calls"
}

test_sha256_check_falls_back_to_sha256_shasum_with_exact_arguments() {
    local calls
    calls="$(mktemp)"
    SHASUM_CALLS="$calls"
    command() {
        if [ "$1" = "-v" ] && [ "${2:-}" = "sha256sum" ]; then
            return 1
        fi
        builtin command "$@"
    }
    shasum() { printf '%s\n' "$*" > "$SHASUM_CALLS"; }

    sha256_check archive.tar.gz.sha256
    unset -f command shasum

    assert_equals "-a 256 -c archive.tar.gz.sha256" "$(< "$calls")"
    rm -f "$calls"
}

test_sha256_check_tampered_file_fails() {
    local tmp
    tmp="$(mktemp -d)"
    echo "original content" > "$tmp/archive.tar.gz"
    (
        cd "$tmp"
        if command -v sha256sum >/dev/null 2>&1; then
            sha256sum archive.tar.gz > archive.tar.gz.sha256
        else
            shasum -a 256 archive.tar.gz > archive.tar.gz.sha256
        fi
    )
    echo "tampered content" > "$tmp/archive.tar.gz"
    assert_fails "(cd '$tmp' && sha256_check archive.tar.gz.sha256)"
    rm -rf "$tmp"
}
