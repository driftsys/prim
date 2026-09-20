#!/usr/bin/env bash
set -euo pipefail

die() {
  printf 'error: %s\n' "$1" >&2
  exit 2
}

if [ "$#" -ne 2 ]; then
  die "usage: smoke-test.sh ARCHIVE CHECKSUM"
fi

archive="$1"
checksum="$2"
[ -f "$archive" ] || die "archive does not exist: $archive"
[ -f "$checksum" ] || die "checksum does not exist: $checksum"

archive_directory="$(cd "$(dirname "$archive")" && pwd)"
checksum_path="$(cd "$(dirname "$checksum")" && pwd)/$(basename "$checksum")"
if command -v sha256sum >/dev/null 2>&1; then
  (cd "$archive_directory" && sha256sum -c "$checksum_path")
else
  (cd "$archive_directory" && shasum -a 256 -c "$checksum_path")
fi

scratch_directory="$(mktemp -d "${TMPDIR:-/tmp}/prim-smoke.XXXXXX")"
trap 'rm -rf "$scratch_directory"' EXIT

tar -xzf "$archive" -C "$scratch_directory"
if [ -f "$scratch_directory/prim" ]; then
  binary="$scratch_directory/prim"
elif [ -f "$scratch_directory/prim.exe" ]; then
  binary="$scratch_directory/prim.exe"
else
  die "archive does not contain prim or prim.exe"
fi

install_directory="$scratch_directory/install"
mkdir -p "$install_directory"
cp "$binary" "$install_directory/"
chmod +x "$install_directory/$(basename "$binary")"

PATH="$install_directory:$PATH"
export PATH
command -v prim >/dev/null 2>&1 || die "prim is not available from PATH"
prim --version
