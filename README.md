<div align="center">

# KokoroBox Native

Rust components and Node.js bindings for [KokoroBox Desktop](https://github.com/amamiyakokoro/KokoroBox-Desktop).

[Documentation](docs/api.md) · [License](LICENSE)

</div>

## Features

- Application and icon inspection, executable discovery, and rule conversion
- Network observation, launch at login, and secure service identity
- Platform-specific service, application routing, proxy, and privilege operations
- Traffic presenter and Windows portable updater sidecars

Use `getNativeCapabilities()` to check which optional features are available.

## Supported platforms

Prebuilt npm packages support Windows (MSVC), macOS, and GNU/Linux on x64 and
arm64. The portable updater is available on Windows only.

## Get started

Install the ESM package:

```sh
pnpm add kokorobox-native
```

See the [package README](napi/README.md) for a usage example.

## Development

Requires a current Rust toolchain, Node.js 26, and pnpm 11.

```sh
cd napi
pnpm install
pnpm build
cd ..
cargo build --release -p kokorobox-traffic-presenter
cargo build --release -p kokorobox-portable-updater # Windows only
```

Run workspace checks from the repository root:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

The root crate implements native behavior, `napi/` exposes it to JavaScript,
and `traffic-presenter/` and `portable-updater/` build the sidecars. Release
packaging copies sidecars into platform packages with
`cd napi && pnpm package-presenters`.

## Documentation

- [JavaScript API guide](docs/api.md) and [TypeScript declarations](napi/index.d.ts)
- [Platform behavior and security contract](docs/platform-services.md)
- [Traffic presenter protocol](docs/traffic-presenter.md)

## License

Derived from [UruhaLushia/sparkle-native](https://github.com/UruhaLushia/sparkle-native)
with upstream Git history preserved. Licensed under [GPL-3.0-only](LICENSE).
