# KokoroBox Native

`kokorobox-native` is the Rust implementation and N-API bridge that give
KokoroBox Desktop access to operating-system features that are not practical to
provide in JavaScript alone. It is published as a small JavaScript loader plus
prebuilt, platform-specific native packages.

Those platform packages also contain a per-user traffic presenter. Desktop
owns its lifetime and sends versioned numeric traffic snapshots over standard
input; the presenter never connects to Mihomo or handles controller secrets.

It provides file and application icon helpers, application inspection,
shell-free executable discovery, rule-set conversion, launch-at-login, macOS
managed-service and observable network-context helpers, secure service identity,
constrained Unix core permissions, macOS application-routing control-plane management,
and Windows account, elevation, Firewall, and application-scanning integrations.

## Repository structure

```
.
├── Cargo.toml              # Workspace and core Rust crate manifest
├── docs/                   # Public platform and presenter contracts
│   ├── platform-services.md # Platform APIs, validation, and security rules
│   └── traffic-presenter.md # Presenter JSON-lines protocol and trust boundary
├── src/                    # Platform-neutral native library
│   ├── lib.rs              # Public Rust API and capability declaration
│   ├── application.rs      # Application inspection and Windows scanning
│   ├── executables.rs      # Shell-free cross-platform executable discovery
│   ├── icons.rs            # Icon data URLs and display-name lookup
│   ├── macos_service.rs    # Embedded LaunchDaemon lifecycle management
│   ├── platform.rs         # Login-item and network-context implementations
│   ├── privileges.rs       # Constrained Mihomo core-file privileges
│   ├── rules.rs            # Rule-file conversion facade
│   ├── windows/            # Windows-only token, elevation, and Firewall code
│   └── non_windows.rs      # Explicit unsupported-platform Windows stubs
├── traffic-presenter/      # Cross-platform per-user traffic status process
│   ├── src/lib.rs          # Versioned protocol and presentation state
│   ├── src/runtime.rs      # Standard-input command loop
│   └── src/platform/       # Native Windows, macOS, and Linux presenters
└── napi/                   # Published Node.js package and binding crate
    ├── Cargo.toml          # `cdylib` crate that depends on the core crate
    ├── src/                # Rust-to-JavaScript N-API exports and type mapping
    ├── index.js            # ESM loader for a local or platform package binary
    ├── index.d.ts          # Public TypeScript API
    ├── package.json        # npm metadata, targets, and build scripts
    ├── package-presenters.mjs # Copies presenter sidecars into npm packages
    └── README.md           # Package-consumer documentation
```

The root crate owns the native behavior and has no Node.js-specific types. The
`napi` crate is deliberately thin: it converts Rust values and errors to the
public JavaScript API. Keep feature logic in `src/` and add the corresponding
binding and declaration in `napi/src/` and `napi/index.d.ts`.

`traffic-presenter` is a separate executable, not an N-API export. Desktop
locates its platform-matched sidecar with `getTrafficPresenterPath()` and owns
the process over standard input. Its wire protocol and the deliberate boundary
between Desktop and the Mihomo controller live in
[`docs/traffic-presenter.md`](docs/traffic-presenter.md).

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

`waitForNetworkContextChange()` provides a cancellable-by-timeout observation
primitive so clients can maintain one event loop instead of separate interface
and SSID timers. `openServiceIdentity()` returns an opaque Ed25519 signer: new
private keys remain in Windows Credential Manager, macOS Keychain, or Linux
Secret Service. Linux explicitly reports and uses a mode-0600 file fallback
when no Secret Service session is available.

`invokeMacosApplicationRouting()` owns the macOS application-routing control
plane inside the same native module. It accepts the bounded, versioned JSON
protocol used to activate and inspect the System Extension and to configure the
Network Extension. The packet provider remains a separate System Extension.

Windows privilege relaunches are explicit and non-persistent.
`relaunchCurrentApplicationWithPrivilege` starts a fresh copy of the current
application through the UAC `runas` verb or with the interactive desktop shell
as its logical parent. This avoids privileged token duplication. The API does
not accept an arbitrary executable path, create a task or startup entry, or
wait for the child process to exit.

Service lifecycle, macOS legacy-service cleanup, managed-service stopping, and
managed-file permission repair use separate validated APIs. The Node binding
does not expose an arbitrary elevated-command primitive.

Unix core elevation is deliberately narrow. `setCorePrivileges()` accepts only
canonical, existing, executable files named `mihomo` or `mihomo-alpha`, rejects
more than eight paths, and only changes ownership and the set-user-ID bit. It
does not expose arbitrary privileged command execution.

## JavaScript API

The published package is documented in [`napi/README.md`](napi/README.md), its
exact TypeScript contract is in [`napi/index.d.ts`](napi/index.d.ts), and the
platform/security contract is in [`docs/platform-services.md`](docs/platform-services.md).
The presenter transport and trust boundary are documented in
[`docs/traffic-presenter.md`](docs/traffic-presenter.md).

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

Build the N-API module from the `napi` package:

```sh
cd napi
pnpm install
pnpm build
```

Build the traffic presenter from the repository root:

```sh
cargo build --release -p kokorobox-traffic-presenter
```

Release packaging builds both artifacts for each target. Once N-API has
generated its platform package directories, `pnpm package-presenters` copies
the matching presenter executable into each package and records it in that
package's npm manifest.

Run Rust formatting, lint, and workspace tests from the repository root:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

The loader resolves a matching local `.node` file first, then an optional
platform package. Supported release targets are x64 and arm64 Windows (MSVC),
macOS, and GNU/Linux.

## Origin and license

This repository is derived from
[`UruhaLushia/sparkle-native`](https://github.com/UruhaLushia/sparkle-native).
It preserves the upstream Git history and is distributed under GPL-3.0-only.
See [LICENSE](LICENSE).
