#!/bin/sh
set -eu
if [ "$#" -ne 1 ]; then
  echo "Usage: sh scripts/check-macos-routing.sh /path/to/kokorobox-native.node" >&2
  exit 2
fi
routing_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
routing_test_dir=$(mktemp -d)
trap 'rm -rf "$routing_test_dir"' EXIT
# Check the actual Rust-linked library, not only a clang-linked test executable.
# N-API symbols are intentionally resolved by Node, but these Clang builtins are not.
xcrun nm -u "$1" > "$routing_test_dir/undefined-symbols.txt"
if grep -E '__is(Platform|OS)VersionAtLeast' "$routing_test_dir/undefined-symbols.txt"; then
  echo "Native routing library has an unresolved Clang availability helper" >&2
  exit 1
fi
xcrun clang++ -std=c++17 -fobjc-arc -fblocks -Wno-nullability-completeness -mmacosx-version-min=13.0 \
  "$routing_root/native/tests/macos_settings_url.mm" \
  -framework AppKit -framework Foundation -framework NetworkExtension -framework SystemExtensions \
  -o "$routing_test_dir/settings-url"
"$routing_test_dir/settings-url"
