#!/usr/bin/env bash
set -euo pipefail

REPO="driftsys/prim"
INSTALL_DIR="${PRIM_INSTALL_DIR:-$HOME/.local/bin}"
tmp_dir=""

die() { printf 'error: %s\n' "$1" >&2; exit 1; }

detect_shell() {
  case "$(basename "${SHELL:-}")" in
    bash) echo "bash" ;;
    zsh)  echo "zsh"  ;;
    fish) echo "fish" ;;
    *)    echo ""     ;;
  esac
}

install_completions() {
  local shell rc_file snippet
  shell="$(detect_shell)"
  case "$shell" in
    bash) rc_file="$HOME/.bashrc"; snippet="eval \"\$(prim --completions bash)\"" ;;
    zsh)  rc_file="$HOME/.zshrc";  snippet="eval \"\$(prim --completions zsh)\""  ;;
    fish)
      mkdir -p "$HOME/.config/fish/conf.d"
      rc_file="$HOME/.config/fish/conf.d/prim.fish"
      snippet='prim --completions fish | source'
      ;;
    *)
      printf 'note: unknown shell %s — add completions manually\n' "${SHELL:-}" >&2
      return
      ;;
  esac

  # Don't create a missing RC file (fish conf.d/ dir already created above).
  if [ "$shell" != "fish" ] && [ ! -f "$rc_file" ]; then
    printf 'note: %s not found — add completions manually\n' "$rc_file" >&2
    return
  fi

  if grep -q 'prim --completions' "$rc_file" 2>/dev/null; then
    printf 'completions already configured in %s\n' "$rc_file"
    return
  fi

  printf '\n# prim completions\n%s\n' "$snippet" >> "$rc_file"
  printf 'completions installed to %s\n' "$rc_file"
  printf 'note: restart your shell or run: source %s\n' "$rc_file"
}

sha256_check() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum -c "$1"
  else
    shasum -a 256 -c "$1"
  fi
}

detect_target() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"

  case "$os" in
    Linux)
      case "$arch" in
        x86_64)  echo "x86_64-unknown-linux-musl" ;;
        aarch64) echo "aarch64-unknown-linux-musl" ;;
        *)       die "unsupported architecture: $arch" ;;
      esac
      ;;
    Darwin)
      case "$arch" in
        x86_64)  echo "x86_64-apple-darwin" ;;
        arm64)   echo "aarch64-apple-darwin" ;;
        *)       die "unsupported architecture: $arch" ;;
      esac
      ;;
    *)
      die "unsupported OS: $os (use WSL on Windows)"
      ;;
  esac
}

main() {
  local target version download_url base

  target="$(detect_target)"
  printf 'detected target: %s\n' "$target"

  version="$(curl -sSf "https://api.github.com/repos/$REPO/releases/latest" \
    | grep '"tag_name"' | head -1 | cut -d'"' -f4)"
  [ -n "$version" ] || die "could not determine latest release"
  printf 'latest version: %s\n' "$version"

  base="prim-$target"
  download_url="https://github.com/$REPO/releases/download/$version/$base.tar.gz"
  printf 'downloading %s\n' "$download_url"

  tmp_dir="$(mktemp -d)"
  trap 'rm -rf "${tmp_dir:-}"' EXIT

  curl -sSfL "$download_url" -o "$tmp_dir/$base.tar.gz" \
    || die "download failed — check that the release exists for $target"
  curl -sSfL "$download_url.sha256" -o "$tmp_dir/$base.tar.gz.sha256" \
    || die "checksum download failed"

  (cd "$tmp_dir" && sha256_check "$base.tar.gz.sha256") \
    || die "checksum verification failed"

  tar -xzf "$tmp_dir/$base.tar.gz" -C "$tmp_dir"

  mkdir -p "$INSTALL_DIR"
  mv "$tmp_dir/prim" "$INSTALL_DIR/prim"
  chmod +x "$INSTALL_DIR/prim"

  printf 'installed prim to %s/prim\n' "$INSTALL_DIR"

  # Install man page if present in the tarball.
  local man_dir="${PRIM_MAN_DIR:-$HOME/.local/share/man/man1}"
  if ls "$tmp_dir"/prim*.1 >/dev/null 2>&1; then
    mkdir -p "$man_dir"
    cp "$tmp_dir"/prim*.1 "$man_dir/"
    printf 'installed man page to %s\n' "$man_dir"
    printf "hint: if 'man prim' doesn't work, add to your shell profile:\n"
    printf "      export MANPATH=\"\$HOME/.local/share/man:\${MANPATH:-}\"\n"
  fi

  # Install shell completions.
  install_completions

  if command -v prim >/dev/null 2>&1; then
    printf 'version: %s\n' "$(prim --version)"
  else
    printf 'note: %s is not in your PATH — add it to use "prim"\n' "$INSTALL_DIR"
  fi
}

# Run main when executed as a script (directly or via `curl … | bash`).
# When piped from stdin BASH_SOURCE[0] is unset; under `set -u` a bare
# expansion would abort before main() runs, so default to empty and treat
# the unset case as "executed, not sourced".
if [[ -z "${BASH_SOURCE[0]:-}" || "${BASH_SOURCE[0]}" == "${0}" ]]; then main "$@"; fi
