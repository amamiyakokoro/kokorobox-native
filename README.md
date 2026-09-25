# KokoroBox Native

Native Rust components for KokoroBox Desktop. The `kokorobox-native` npm
package loads prebuilt N-API binaries for Windows, macOS, and GNU/Linux on x64
and arm64.

The library provides application and icon inspection, executable discovery,
rule conversion, network observation, launch-at-login, service identity, and
platform-specific service, routing, proxy, and privilege operations. Optional
features are reported by `getNativeCapabilities()`.

## Repository structure

| Path | Purpose |
| --- | --- |
| `src/` | Core Rust library and platform implementations |
| `napi/` | Node.js bindings, TypeScript declarations, and npm package |
| `traffic-presenter/` | Per-user traffic display process controlled by Desktop over standard input |
| `portable-updater/` | Windows portable update sidecar |
| `native/` | macOS routing bridge |
| `docs/` | Platform and presenter contracts |

Keep native behavior in `src/`; expose it through `napi/src/` and
`napi/index.d.ts`. The sidecars are separate executables, not N-API exports.

## Documentation

- [Package usage and API overview](napi/README.md)
- [TypeScript API](napi/index.d.ts)
- [Platform behavior and security contract](docs/platform-services.md)
- [Traffic presenter protocol](docs/traffic-presenter.md)

## Development

Requires a current Rust toolchain, Node.js 26, and pnpm 11.

```sh
cd napi
pnpm install
pnpm build
cd ..
```

From the repository root, build the sidecars and check the Rust workspace:

```sh
cargo build --release -p kokorobox-traffic-presenter
cargo build --release -p kokorobox-portable-updater # Windows only
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Release packaging copies platform-matched sidecars into the npm packages with
`cd napi && pnpm package-presenters`.

## Origin and license

Derived from [UruhaLushia/sparkle-native](https://github.com/UruhaLushia/sparkle-native)
with upstream Git history preserved. Licensed under GPL-3.0-only; see
[LICENSE](LICENSE).
