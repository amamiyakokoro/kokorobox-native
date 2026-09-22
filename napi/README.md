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
  getTrafficPresenterPath,
  inspectApplication,
} from "kokorobox-native";

const capabilities = getNativeCapabilities();
const network = await getNetworkContext();
const app = await inspectApplication("/Applications/KokoroBox.app");
const presenter = getTrafficPresenterPath();
```

The full TypeScript declarations are shipped in
[`index.d.ts`](index.d.ts).

## API overview

| Area | Exports |
| --- | --- |
| Capabilities | `getNativeCapabilities` |
| Traffic presenter | `getTrafficPresenterPath` |
| Icons | `fileToDataUrl`, `getAppName` |
| Applications | `inspectApplication`, `scanWindowsApplications`, `findExecutables` |
| Rules | `fileToStr` |
| Platform | `getLaunchAtLogin`, `setLaunchAtLogin`, `getNetworkContext` |
| macOS service | `getMacosManagedServiceStatus`, `registerMacosManagedService`, `unregisterMacosManagedService`, `reloadMacosManagedService`, `openMacosLoginItemsSettings` |
| macOS application routing | `invokeMacosApplicationRouting` |
| Core permissions | `getCorePrivilegeStatus`, `setCorePrivileges` |
| Privileged operations | `runServiceLifecycleElevated`, `cleanupLegacyMacosService`, `stopMacosManagedService`, `repairManagedFilePermissions` |
| Windows | `getCurrentUserSid`, `isRunningAsAdmin`, `relaunchCurrentApplicationWithPrivilege`, `setupFirewallRules` |

`inspectApplication` validates the input for the current platform and resolves
it to a routing identifier: an executable path on Windows and Linux, or a code
signing identifier on macOS. `scanWindowsApplications` is asynchronous and
Windows-only; it accepts an optional result limit and executable-name exclusion
list.

`findExecutables` asynchronously searches absolute additional directories,
`PATH`, and platform-standard binary locations without spawning command-line
tools. It returns canonical paths for stable deduplication; prefix matching is
available only when explicitly requested.

`fileToStr` converts a rule file and returns each generated output keyed by its
behavior, metadata for those outputs, and rules that were skipped. Its optional
`RuleConvertOptions` accepts Mihomo, General, Egern, and sing-box targets and
the formats listed in the TypeScript declaration.

Launch-at-login requires a stable filesystem-safe `identifier`, a display name,
and an absolute executable path. It returns the selected backend: the Windows
current-user Run key, macOS `SMAppService.mainApp`, or Linux XDG Autostart. On
macOS, `requiresApproval` reports when the entry is registered but still needs
approval in System Settings. Network context is best-effort, so optional fields
such as the default interface and SSID may be absent. macOS obtains this state
directly from SystemConfiguration without parsing command output.
Windows obtains the preferred interface and DNS servers from IP Helper and the
SSID from Native Wi-Fi, without spawning PowerShell.

The macOS managed-service functions accept the basename of an embedded
LaunchDaemon plist and use `SMAppService` directly. They return an explicit
`requires-approval` state when an administrator must approve the daemon in
System Settings.

`invokeMacosApplicationRouting(request)` runs the bounded, versioned macOS
application-routing control-plane protocol off the Node.js main thread. It
controls the bundled System Extension and Network Extension; packet forwarding
continues to run in the separate extension process.

Core-file privilege APIs are supported on macOS and Linux. They accept only
canonical, existing executables named `mihomo` or `mihomo-alpha`; callers cannot
use them as a general privileged-command interface. See the repository's
[platform-services contract](../docs/platform-services.md) for validation and
platform behavior.

`getTrafficPresenterPath()` returns the presenter executable shipped in the
same platform package as the loaded native binding. The presenter accepts the
versioned JSON-lines protocol documented by the `traffic-presenter` crate on
standard input and terminates when its parent closes that stream.

## Platform behavior

Use `getNativeCapabilities()` before enabling an optional system feature.
Windows account, elevation, Firewall, and application-scan APIs report an
`UNSUPPORTED_PLATFORM:` error on other operating systems instead of behaving as
no-ops.

The capability contract includes dedicated flags for APIs that have narrower
support than the package itself:

| Capability | Supported platforms |
| --- | --- |
| `networkDnsMutation` | macOS |
| `serviceLifecycle` | Windows, macOS, Linux |
| `managedFilePermissions` | macOS, Linux |
| `windowsPrivilegeRelaunch` | Windows |
| `windowsUwpLoopback` | Windows |

Use these flags instead of inferring availability from `process.platform` or
from a related but broader capability.

`runServiceLifecycleElevated(options)` accepts only the KokoroBox Service
`init`, `install`, `uninstall`, `start`, `stop`, and `restart` actions. macOS
legacy-service cleanup, managed-service stopping, and managed-file permission
repair are separate purpose-specific APIs.

`relaunchCurrentApplicationWithPrivilege(args, elevated)` starts a new copy of
the current Windows application without accepting an executable path. An
elevated relaunch presents the standard UAC prompt; the unelevated path uses
the interactive desktop shell token. It does not configure persistent
elevation.

## Contributing

The source is a two-crate Rust workspace. The root crate implements native
behavior; `napi/` maps it into this package’s JavaScript API. See the root
[README](../README.md#repository-structure) for the complete layout and build
commands.

This project is derived from
[`UruhaLushia/sparkle-native`](https://github.com/UruhaLushia/sparkle-native)
and is licensed under GPL-3.0-only.
