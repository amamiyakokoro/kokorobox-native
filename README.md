# KokoroBox Native

`kokorobox-native` is the Rust implementation and N-API bridge that give
KokoroBox Desktop access to operating-system features that are not practical to
provide in JavaScript alone. It is published as a small JavaScript loader plus
prebuilt, platform-specific native packages.

It provides file and application icon helpers, application inspection, rule-set
conversion, launch-at-login, macOS managed-service and network-context helpers,
constrained Unix core permissions, and Windows account, elevation, Firewall, and
application-scanning integrations.

## Repository structure

```
.
├── Cargo.toml              # Workspace and core Rust crate manifest
├── src/                    # Platform-neutral native library
│   ├── lib.rs              # Public Rust API and capability declaration
│   ├── application.rs      # Application inspection and Windows scanning
│   ├── icons.rs            # Icon data URLs and display-name lookup
│   ├── macos_service.rs    # Embedded LaunchDaemon lifecycle management
│   ├── platform.rs         # Login-item and network-context implementations
│   ├── privileges.rs       # Constrained Mihomo core-file privileges
│   ├── rules.rs            # Rule-file conversion facade
│   ├── windows/            # Windows-only token, elevation, and Firewall code
│   └── non_windows.rs      # Explicit unsupported-platform Windows stubs
└── napi/                   # Published Node.js package and binding crate
    ├── Cargo.toml          # `cdylib` crate that depends on the core crate
    ├── src/                # Rust-to-JavaScript N-API exports and type mapping
    ├── index.js            # ESM loader for a local or platform package binary
    ├── index.d.ts          # Public TypeScript API
    ├── package.json        # npm metadata, targets, and build scripts
    └── README.md           # Package-consumer documentation
```

The root crate owns the native behavior and has no Node.js-specific types. The
`napi` crate is deliberately thin: it converts Rust values and errors to the
public JavaScript API. Keep feature logic in `src/` and add the corresponding
binding and declaration in `napi/src/` and `napi/index.d.ts`.

## Platform contract

Call `getNativeCapabilities()` before showing optional integrations. It reports
which features were compiled into the loaded binary. Windows-only APIs—SID
lookup, elevation, Firewall management, and directory scanning—fail with an
`UNSUPPORTED_PLATFORM:` error outside Windows; they do not silently succeed.

`inspectApplication(path)` and `scanWindowsApplications(directory)` run off the
Node.js main thread. Application inspection returns a stable routing identifier,
a display name, and an optional icon data URL. The Windows scanner skips links,
limits its result count, and counts nested directories it cannot read.

Launch-at-login uses the current user's Run registry key on Windows,
`SMAppService.mainApp` on macOS, and XDG Autostart on Linux. Managing the
Windows entry does not elevate the caller, and the launched application keeps
the interactive user's normal token. `getNetworkContext()` is best-effort:
unavailable interface, network-service, DNS, or Wi-Fi fields are omitted rather
than inferred. macOS network discovery uses SystemConfiguration without
spawning command-line tools.
Windows network discovery likewise uses IP Helper and Native Wi-Fi instead of
PowerShell.

Windows privilege relaunches are explicit and non-persistent. `launchElevated`
starts a fresh process through the UAC `runas` verb, while `launchUnelevated`
starts it with the interactive desktop shell as its logical parent. This avoids
privileged token duplication. Neither API creates a task, service, startup
entry, or waits for the child process to exit.

Unix core elevation is deliberately narrow. `setCorePrivileges()` accepts only
canonical, existing, executable files named `mihomo` or `mihomo-alpha`, rejects
more than eight paths, and only changes ownership and the set-user-ID bit. It
does not expose arbitrary privileged command execution.

## JavaScript API

The published package is documented in [`napi/README.md`](napi/README.md), its
exact TypeScript contract is in [`napi/index.d.ts`](napi/index.d.ts), and the
platform/security contract is in [`docs/platform-services.md`](docs/platform-services.md).

```ts
import {
  getNativeCapabilities,
  inspectApplication,
  setLaunchAtLogin,
} from "kokorobox-native";

const capabilities = getNativeCapabilities();
const application = await inspectApplication("/Applications/KokoroBox.app");

if (capabilities.launchAtLogin) {
  await setLaunchAtLogin(
    {
      identifier: "com.amamiyakokoro.kokorobox",
      displayName: "KokoroBox",
      executablePath: "/Applications/KokoroBox.app",
    },
    true,
  );
}
```

## Development

Prerequisites: a current Rust toolchain, Node.js 16 or later, and pnpm 11.

Build the native module from the `napi` package:

```sh
cd napi
pnpm install
pnpm build
```

Run Rust formatting and lint checks from the repository root:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
```

The loader resolves a matching local `.node` file first, then an optional
platform package. Supported release targets are x64 and arm64 Windows (MSVC),
macOS, and GNU/Linux.

## Origin and license

This repository is derived from
[`UruhaLushia/sparkle-native`](https://github.com/UruhaLushia/sparkle-native).
It preserves the upstream Git history and is distributed under GPL-3.0-only.
See [LICENSE](LICENSE).
