# kokorobox-native

Native Node.js APIs used by KokoroBox Desktop.

The package exposes file/icon helpers, asynchronous application inspection, Windows executable
directory scanning, rule conversion, Windows elevation and SID queries, and Windows Firewall rule
management. Platform-specific N-API binaries are selected at runtime.

Use `getNativeCapabilities()` before offering optional platform features. Windows-only calls fail
with an `UNSUPPORTED_PLATFORM:` error outside Windows rather than reporting a no-op success.

This project is derived from
[`UruhaLushia/sparkle-native`](https://github.com/UruhaLushia/sparkle-native) and remains licensed
under GPL-3.0-only. KokoroBox maintains its own package and release lifecycle so the bridge can
evolve with product-specific APIs.
