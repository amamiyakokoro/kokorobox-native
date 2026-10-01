# kokorobox-native

Native Node.js APIs used by KokoroBox Desktop. This ESM package selects the
native binary for the current operating system and architecture.

## Installation

```sh
pnpm add kokorobox-native
```

Prebuilt optional packages support x64 and arm64 Windows (MSVC), macOS, and
GNU/Linux. For local development, the loader also accepts
`NAPI_RS_NATIVE_LIBRARY_PATH`.

## Usage

```ts
import {
  getNativeCapabilities,
  getNetworkContext,
  inspectApplication,
} from "kokorobox-native";

const capabilities = getNativeCapabilities();
const network = await getNetworkContext();
const app = await inspectApplication("/Applications/KokoroBox.app");
```

Check `getNativeCapabilities()` before using optional platform features.

## Diagnostics

`drainNativeLogs()` returns English JSON-compatible records with `ts`, `level`,
`target`, and `msg`. Use one consumer to periodically drain the queue into the
application's log. It retains at most 500 records; overflow produces a warning.
`setNativeLogLevel("debug" | "info" | "warn" | "error")` changes recording
severity (default: `info`). Healthy polling is only recorded at `debug`. Operation failures retain
their original exceptions and causes; lifecycle successes record the operation
name without arguments, keys, or signatures.

The traffic presenter emits JSON Lines to stderr. The Windows portable updater
also saves its last run in `%TEMP%/kokorobox-portable-updater.log` because Desktop
exits before extraction. These sidecars include Unix milliseconds in `timestamp`.

## System Proxy diagnostics

`getSystemProxyDiagnostics()` returns actual OS configuration with independent
availability information. Windows, macOS and Linux share the same entry point;
platform-specific details are nested under `windows`, `macos` or `linux`.
macOS uses SystemConfiguration for effective HTTP/HTTPS/SOCKS, PAC/discovery,
primary IPv4/IPv6 services and the current Network Location. Reading never asks
for administrator authorization. Runtime health and connectivity belong to
KokoroBox Service, not this API.

`setSystemProxy(settings)` supports explicit Windows/macOS manual, PAC and
disabled actions. macOS honors `onlyActiveDevice`, preserves PAC/discovery
when restoring manual proxies, and reuses Native's administrator dialog if
required for a write. Its structured result includes
`automaticSettingsPreserved`; callers must not resume a legacy watchdog or
cleanup that would erase those settings. Never invoke this setter merely to
inspect or refresh diagnostics.

## Documentation

- [JavaScript API guide](../docs/api.md)
- [TypeScript declarations](index.d.ts)
- [Platform behavior and security contract](../docs/platform-services.md)
- [Traffic presenter protocol](../docs/traffic-presenter.md)
- [Repository layout and build commands](../README.md)

Derived from [UruhaLushia/sparkle-native](https://github.com/UruhaLushia/sparkle-native)
and licensed under GPL-3.0-only.
### System DNS diagnostics

`getSystemDnsDiagnostics()` asynchronously probes the OS resolver for fixed
public test domains (`www.gstatic.com`, `example.com`) and returns per-domain
outcomes, the primary interface/service and current DNS IP addresses. It is
read-only on Windows, macOS and Linux. No input domains, arbitrary commands,
credentials or SSIDs are accepted or returned. Linux prefers per-link DNS from
NetworkManager/systemd-resolved and falls back to resolv.conf for inspection.

Bootstrap repair is Desktop configuration policy: Desktop copies the observed
system DNS addresses into `dns.default-nameserver` and restarts the core. Native
does not expose an OS DNS mutation API for diagnostics.
