# KokoroBox Native

`kokorobox-native` is the native Node.js bridge maintained for KokoroBox Desktop. It provides:

- file and application icon extraction;
- rule-file conversion;
- Windows elevation and administrator checks;
- Windows user SID lookup;
- Windows Firewall rule management.

The JavaScript API is currently compatible with `@uruhalushia/sparkle-native` 0.1.2. New APIs
should be added here behind stable typed exports before KokoroBox Desktop adopts them.

## Packages

The release workflow publishes a small JavaScript loader package plus architecture-specific N-API
packages for Windows, macOS, and GNU/Linux. The loader chooses the package matching
`process.platform` and `process.arch`.

## Development

```sh
cd napi
pnpm install
pnpm build
```

Rust formatting and linting:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
```

## Origin and license

This repository is derived from
[`UruhaLushia/sparkle-native`](https://github.com/UruhaLushia/sparkle-native). It preserves the
upstream Git history and is distributed under GPL-3.0-only. See [LICENSE](LICENSE).
