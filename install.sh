#!/bin/sh
# Installer for momo — https://github.com/Jarvvski/momo
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/Jarvvski/momo/main/install.sh | sh
#   sh install.sh --version 0.6.0
#   sh install.sh --to /usr/local/bin
#
# Downloads the release archive for this machine, verifies it against the
# published SHA256SUMS, and installs the `momo` binary. POSIX sh only.

set -eu

REPO="Jarvvski/momo"
BIN="momo"

usage() {
  cat <<EOF
Install the momo binary from a GitHub release.

Usage: sh install.sh [options]

Options:
  --version <x.y.z>  Install a specific version (default: the latest release).
  --to <dir>         Install into <dir> (default: \$HOME/.local/bin).
  -h, --help         Show this help and exit.

Examples:
  sh install.sh
  sh install.sh --version 0.6.0
  sh install.sh --to /usr/local/bin
EOF
}

fail() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

# fetch URL DEST — download to a file.
fetch() {
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$1" -o "$2"
  elif command -v wget >/dev/null 2>&1; then
    wget -qO "$2" "$1"
  else
    fail "curl or wget is required to download momo"
  fi
}

# fetch_stdout URL — download to stdout.
fetch_stdout() {
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$1"
  elif command -v wget >/dev/null 2>&1; then
    wget -qO- "$1"
  else
    fail "curl or wget is required to download momo"
  fi
}

# detect_target — set $target to one of the four release triples, or fail.
detect_target() {
  os="$(uname -s)"
  arch="$(uname -m)"

  case "$os" in
    Linux) os_part="unknown-linux-gnu" ;;
    Darwin) os_part="apple-darwin" ;;
    CYGWIN* | MINGW* | MSYS* | Windows*)
      fail "Windows is not supported: momo is Unix-only (Linux and macOS)"
      ;;
    *)
      fail "unsupported operating system '$os': momo ships Linux and macOS builds only"
      ;;
  esac

  case "$arch" in
    x86_64 | amd64) arch_part="x86_64" ;;
    arm64 | aarch64) arch_part="aarch64" ;;
    *)
      fail "unsupported architecture '$arch': momo ships x86_64 and aarch64 builds only"
      ;;
  esac

  target="${arch_part}-${os_part}"
}

# resolve_version — set $version from --version, or the latest release tag.
resolve_version() {
  if [ -n "$requested_version" ]; then
    version="$requested_version"
    return
  fi

  tag="$(fetch_stdout "https://api.github.com/repos/${REPO}/releases/latest" |
    sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"v\{0,1\}\([^"]*\)".*/\1/p' |
    head -n 1)"

  if [ -z "$tag" ]; then
    fail "could not determine the latest release; pass --version <x.y.z> to pin one"
  fi
  version="$tag"
}

# verify_checksum ARCHIVE — compare the archive against the SHA256SUMS entry.
verify_checksum() {
  expected="$(awk -v file="$1" '$2 == file { print $1 }' "${tmp}/SHA256SUMS")"
  if [ -z "$expected" ]; then
    fail "no checksum for $1 in SHA256SUMS; the release may be incomplete"
  fi

  if command -v sha256sum >/dev/null 2>&1; then
    actual="$(sha256sum "${tmp}/$1" | awk '{ print $1 }')"
  elif command -v shasum >/dev/null 2>&1; then
    actual="$(shasum -a 256 "${tmp}/$1" | awk '{ print $1 }')"
  else
    fail "sha256sum or shasum is required to verify the download"
  fi

  if [ "$expected" != "$actual" ]; then
    fail "checksum mismatch for $1
  expected: $expected
  actual:   $actual
Refusing to install a corrupt or tampered download."
  fi
}

requested_version=""
install_dir=""

while [ $# -gt 0 ]; do
  case "$1" in
    --version)
      [ $# -ge 2 ] || fail "--version requires a value, e.g. --version 0.6.0"
      requested_version="$2"
      shift 2
      ;;
    --version=*)
      requested_version="${1#*=}"
      shift
      ;;
    --to)
      [ $# -ge 2 ] || fail "--to requires a directory, e.g. --to /usr/local/bin"
      [ -n "$2" ] || fail "--to requires a non-empty directory"
      install_dir="$2"
      shift 2
      ;;
    --to=*)
      [ -n "${1#*=}" ] || fail "--to requires a non-empty directory"
      install_dir="${1#*=}"
      shift
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    *)
      fail "unknown option '$1' (try --help)"
      ;;
  esac
done

# Default to ~/.local/bin (no sudo); --to overrides.
if [ -z "$install_dir" ]; then
  [ -n "${HOME:-}" ] || fail "HOME is not set; pass --to <dir>"
  install_dir="${HOME}/.local/bin"
fi

# Accept both `0.6.0` and `v0.6.0`.
case "$requested_version" in
  v*) requested_version="${requested_version#v}" ;;
esac

detect_target
resolve_version

archive="momo-${version}-${target}.tar.gz"
base_url="https://github.com/${REPO}/releases/download/v${version}"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

printf 'Installing momo %s (%s)\n' "$version" "$target"
fetch "${base_url}/${archive}" "${tmp}/${archive}"
fetch "${base_url}/SHA256SUMS" "${tmp}/SHA256SUMS"
verify_checksum "$archive"

# The archive holds a `momo-<version>-<target>/` directory containing `momo`.
tar -xzf "${tmp}/${archive}" -C "$tmp"
binary="${tmp}/momo-${version}-${target}/${BIN}"
[ -f "$binary" ] || fail "expected $BIN in ${archive}, but it was not there"

mkdir -p "$install_dir"
cp "$binary" "${install_dir}/${BIN}"
chmod 0755 "${install_dir}/${BIN}"

printf 'Installed %s to %s\n' "$BIN" "${install_dir}/${BIN}"

case ":${PATH}:" in
  *":${install_dir}:"*) ;;
  *)
    printf 'note: %s is not on your PATH; add it, e.g.\n' "$install_dir" >&2
    printf "      export PATH=\"%s:\$PATH\"\n" "$install_dir" >&2
    ;;
esac

printf 'Run "%s --version" to confirm the install.\n' "$BIN"
