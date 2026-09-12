#!/usr/bin/env bash
set -euo pipefail

die() {
  printf 'error: %s\n' "$1" >&2
  exit 2
}

if [ "$#" -ne 4 ]; then
  die "usage: package.sh TARGET BINARY MAN_PAGE OUTPUT_DIRECTORY"
fi

target="$1"
binary="$2"
man_page="$3"
output_directory="$4"

case "$target" in
  x86_64-unknown-linux-musl | aarch64-unknown-linux-musl | x86_64-apple-darwin | aarch64-apple-darwin)
    binary_name="prim"
    ;;
  x86_64-pc-windows-msvc)
    binary_name="prim.exe"
    ;;
  *)
    die "unsupported release target: $target"
    ;;
esac

[ -f "$binary" ] || die "release binary does not exist: $binary"
[ -f "$man_page" ] || die "man page does not exist: $man_page"

mkdir -p "$output_directory"
output_directory="$(cd "$output_directory" && pwd)"
staging_directory="$(mktemp -d "${TMPDIR:-/tmp}/prim-package.XXXXXX")"
trap 'rm -rf "$staging_directory"' EXIT

umask 022
cp "$binary" "$staging_directory/$binary_name"
cp "$man_page" "$staging_directory/prim.1"
chmod 755 "$staging_directory/$binary_name"
chmod 644 "$staging_directory/prim.1"
if command -v xattr >/dev/null 2>&1; then
  xattr -c "$staging_directory/$binary_name" "$staging_directory/prim.1"
fi
TZ=UTC touch -t 197001010000 "$staging_directory/$binary_name" "$staging_directory/prim.1"

archive="prim-$target.tar.gz"
COPYFILE_DISABLE=1 COPY_EXTENDED_ATTRIBUTES_DISABLE=1 LC_ALL=C \
  tar --no-xattrs --no-acls -cf - -C "$staging_directory" "$binary_name" prim.1 \
  | gzip -n > "$output_directory/$archive"

(
  cd "$output_directory"
  if command -v sha256sum >/dev/null 2>&1; then
    digest="$(sha256sum "$archive" | awk '{print $1}')"
  else
    digest="$(shasum -a 256 "$archive" | awk '{print $1}')"
  fi
  printf '%s  %s\n' "$digest" "$archive" > "$archive.sha256"
)
