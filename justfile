# Compile
assemble:
    cargo build

# Run tests
test:
    cargo test

# Lint and format check
lint:
    cargo clippy --workspace --all-targets -- -D warnings
    cargo fmt -- --check
    dprint check
    npx markdownlint-cli '**/*.md' --ignore node_modules

# Audit dependencies
audit:
    cargo audit

# Test install.sh helpers (detect_target, sha256_check, URL patterns)
test-install:
    bash tools/bash_unit spec/install/install_test.sh

# Run all checks (test + install tests + lint)
check: test test-install lint

# Assemble + check
build: assemble check

# Validate commits on branch and build — run before PR
verify:
    git std lint --range main..HEAD
    just build

# Format Rust and Markdown
fmt:
    cargo fmt
    dprint fmt
    npx markdownlint-cli '**/*.md' --ignore node_modules --fix

# Generate the man page to target/man/
man:
    cargo build -p prim-cli
    mkdir -p target/man
    find target/ -path '*/build/prim-cli-*/out/man/*.1' -exec cp {} target/man/ \;
    @echo "man page written to target/man/"

# Generate and open rustdoc
doc:
    cargo doc --open

# Build and serve mdbook documentation
book:
    mdbook serve

# Bump version, update changelog, commit, and tag
release:
    git std bump

# Publish crates to crates.io (library before binary)
publish: check
    @echo "==> 1/2 prim-fmt"
    cargo publish -p prim-fmt
    @echo "==> 2/2 prim-cli"
    cargo publish -p prim-cli
    @echo ""
    @echo "==> All crates published."

# Build release binary, man page, and shell completions, then install to ~/.local/bin
install:
    #!/usr/bin/env bash
    set -euo pipefail
    cargo build --release
    cp target/release/prim ~/.local/bin/

    just man
    mkdir -p ~/.local/share/man/man1
    cp target/man/*.1 ~/.local/share/man/man1/
    printf "hint: if 'man prim' doesn't work, add to your shell profile:\n"
    printf "      export MANPATH=\"\$HOME/.local/share/man:\${MANPATH:-}\"\n"

    shell="$(basename "${SHELL:-}")"
    case "$shell" in
      bash) rc="$HOME/.bashrc"; s="eval \"\$(prim --completions bash)\"" ;;
      zsh)  rc="$HOME/.zshrc";  s="eval \"\$(prim --completions zsh)\""  ;;
      fish)
        mkdir -p "$HOME/.config/fish/conf.d"
        rc="$HOME/.config/fish/conf.d/prim.fish"
        s='prim --completions fish | source'
        ;;
      *)
        printf 'note: add completions manually for %s\n' "${SHELL:-}" >&2
        exit 0
        ;;
    esac
    if [ "$shell" != "fish" ] && [ ! -f "$rc" ]; then
      printf 'note: %s not found — add completions manually\n' "$rc" >&2
      exit 0
    fi
    if grep -q 'prim --completions' "$rc" 2>/dev/null; then
      printf 'completions already configured in %s\n' "$rc"
    else
      printf '\n# prim completions\n%s\n' "$s" >> "$rc"
      printf 'completions installed to %s\n' "$rc"
    fi

# Remove build artifacts
clean:
    cargo clean
