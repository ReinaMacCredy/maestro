#!/bin/sh
set -eu

usage() {
  echo "usage: scripts/install-local.sh [source-binary] [destination]" >&2
}

if [ "${1:-}" = "--help" ] || [ "${1:-}" = "-h" ]; then
  usage
  exit 0
fi

if [ "$#" -gt 2 ]; then
  usage
  exit 2
fi

source_binary="${1:-target/release/maestro}"
destination="${2:-$HOME/.local/bin/maestro}"

if [ ! -f "$source_binary" ]; then
  echo "source binary not found: $source_binary" >&2
  exit 1
fi

destination_dir=$(dirname "$destination")
destination_name=$(basename "$destination")
mkdir -p "$destination_dir"

temp_path="$destination_dir/.$destination_name.install.$$"
cleanup() {
  rm -f "$temp_path"
}
trap cleanup EXIT HUP INT TERM

install -m 755 "$source_binary" "$temp_path"
mv -f "$temp_path" "$destination"
trap - EXIT HUP INT TERM

echo "installed $source_binary -> $destination"
