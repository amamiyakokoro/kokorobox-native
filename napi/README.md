# kokorobox-native

Native Node.js APIs used by KokoroBox Desktop. This package is an ESM loader:
it selects the native binary for the current operating system and architecture.

## Installation

```sh
pnpm add kokorobox-native
```

Prebuilt optional packages are available for x64 and arm64 Windows (MSVC),
macOS, and GNU/Linux. The loader also accepts a development binary through
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

The full TypeScript declarations are shipped in
[`index.d.ts`](index.d.ts).

## API overview

| Area | Exports |
| --- | --- |
| Capabilities | `getNativeCapabilities` |
| Icons | `fileToDataUrl`, `getAppName` |
| Applications | `inspectApplication`, `scanWindowsApplications` |
| Rules | `fileToStr` |
| Platform | `getLaunchAtLogin`, `setLaunchAtLogin`, `getNetworkContext` |
| Core permissions | `getCorePrivilegeStatus`, `setCorePrivileges` |
| Windows | `getCurrentUserSid`, `isRunningAsAdmin`, `runElevated`, `setupFirewallRules` |

`inspectApplication` validates the input for the current platform and resolves
it to a routing identifier: an executable path on Windows and Linux, or a code
signing identifier on macOS. `scanWindowsApplications` is asynchronous and
Windows-only; it accepts an optional result limit and executable-name exclusion
list.

`fileToStr` converts a rule file and returns each generated output keyed by its
behavior, metadata for those outputs, and rules that were skipped. Its optional
`RuleConvertOptions` accepts Mihomo, General, Egern, and sing-box targets and
the formats listed in the TypeScript declaration.

Launch-at-login requires a stable filesystem-safe `identifier`, a display name,
and an absolute executable path. It returns the selected backend: Windows Task
Scheduler, macOS Login Items, or Linux XDG Autostart. Network context is
best-effort, so optional fields such as the default interface and SSID may be
absent.

Core-file privilege APIs are supported on macOS and Linux. They accept only
canonical, existing executables named `mihomo` or `mihomo-alpha`; callers cannot
use them as a general privileged-command interface. See the repository's
[platform-services contract](../docs/platform-services.md) for validation and
platform behavior.

## Platform behavior

Use `getNativeCapabilities()` before enabling an optional system feature.
Windows account, elevation, Firewall, and application-scan APIs report an
`UNSUPPORTED_PLATFORM:` error on other operating systems instead of behaving as
no-ops.

## Contributing

The source is a two-crate Rust workspace. The root crate implements native
behavior; `napi/` maps it into this package’s JavaScript API. See the root
[README](../README.md#repository-structure) for the complete layout and build
commands.

This project is derived from
[`UruhaLushia/sparkle-native`](https://github.com/UruhaLushia/sparkle-native)
and is licensed under GPL-3.0-only.
