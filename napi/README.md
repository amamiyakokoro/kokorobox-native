# kokorobox-native

Native Node.js APIs used by KokoroBox Desktop.

The package currently exposes file/icon helpers, rule conversion, Windows elevation and SID
queries, and Windows Firewall rule management. Platform-specific N-API binaries are selected at
runtime.

This project is derived from
[`UruhaLushia/sparkle-native`](https://github.com/UruhaLushia/sparkle-native) and remains licensed
under GPL-3.0-only. KokoroBox maintains its own package and release lifecycle so the bridge can
evolve with product-specific APIs.
