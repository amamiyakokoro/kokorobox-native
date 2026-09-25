# JavaScript API

`kokorobox-native` is an ESM package. Use `getNativeCapabilities()` to check
optional features before calling platform-specific APIs. The exact signatures
and return types are in [`napi/index.d.ts`](../napi/index.d.ts); this page
describes what each API does and where it is available.

## Discovery and data

| API | Purpose |
| --- | --- |
| `getNativeCapabilities()` | Report features compiled into the loaded binary. |
| `fileToDataUrl(path)`, `getAppName(path)` | Read an icon as a data URL or get an application's display name. |
| `inspectApplication(path)` | Resolve a validated application to its executable, routing identifier, and optional icon. |
| `scanWindowsApplications(directory, ...)` | Scan a Windows directory, with optional result limit and executable-name exclusions. |
| `findExecutables(options)` | Search additional directories, `PATH`, and standard locations for executable names. |
| `fileToStr(path, options?)` | Convert a rule file and return outputs, metadata, and skipped rules. |

`inspectApplication()` and `scanWindowsApplications()` run off the Node.js main
thread. Application identifiers are executable paths on Windows and Linux, or
code signing identifiers on macOS. Executable discovery uses no shell and
deduplicates results by canonical path; prefix matching is opt-in. Rule
conversion accepts the targets and formats in `RuleConvertOptions`.

## Desktop integration

| API | Purpose |
| --- | --- |
| `getLaunchAtLogin(options)`, `setLaunchAtLogin(options, enabled)` | Inspect or change the current user's login entry. |
| `getNetworkContext()` | Get a best-effort network snapshot. |
| `waitForNetworkContextChange(previous, timeoutMs?)` | Wait for a changed snapshot; return `null` on timeout. |
| `openServiceIdentity(options, legacy?)`, `deleteServiceIdentity(options)` | Manage an Ed25519 identity in platform credential storage. |
| `ServiceIdentity.getInfo()`, `ServiceIdentity.sign(data)` | Inspect the public identity or sign data without exposing the private key. |
| `getTrafficPresenterPath()` | Locate the traffic presenter in the current platform package. |
| `getPortableUpdaterPath()` | Locate the Windows portable updater sidecar. |

Launch-at-login uses the Windows current-user Run key, macOS `SMAppService`, or
Linux XDG Autostart. Network fields can be absent when the operating system
cannot provide them. Service identity uses Windows Credential Manager, macOS
Keychain, or Linux Secret Service; Linux can report a protected file fallback.
See [platform services](platform-services.md) for validation and backend details
and the [traffic presenter contract](traffic-presenter.md) for its input
protocol.

## Services and platform operations

| API | Platform and purpose |
| --- | --- |
| `getWindowsServiceStatus()` | Windows: read KokoroBox Service status. |
| `getLinuxServiceStatus()` | Linux: read KokoroBox Service status through systemd. |
| `getMacosServiceProcessStatus()` | macOS: read the managed daemon's process status. |
| `getMacosManagedServiceStatus(plistName)`, `registerMacosManagedService(plistName)`, `unregisterMacosManagedService(plistName)`, `reloadMacosManagedService(plistName)`, `openMacosLoginItemsSettings()` | macOS: manage an embedded LaunchDaemon and open its approval settings. |
| `applyMacosApplicationRouting(configuration)`, `getMacosApplicationRoutingStatus()`, `stopMacosApplicationRouting()`, `openMacosApplicationRoutingSettings()` | macOS: control application routing through typed policies and status calls. |
| `setTerminalProxyEnvironment(host, port, bypass)`, `clearTerminalProxyEnvironment()` | Linux: manage KokoroBox's user-session proxy configuration. |
| `runServiceLifecycleElevated(options)` | Run a validated KokoroBox Service `init`, `install`, `uninstall`, `start`, `stop`, or `restart` action. |
| `cleanupLegacyMacosService()`, `stopMacosManagedService()` | macOS: remove legacy service files or stop the managed service. |
| `repairManagedFilePermissions(target, managedRoot, uid, gid)` | macOS/Linux: repair one validated managed file. |
| `getCorePrivilegeStatus(paths)`, `setCorePrivileges(paths, enabled)` | macOS/Linux: inspect or change constrained Mihomo executable permissions. |

The macOS managed-service functions accept only the basename of an embedded
LaunchDaemon plist. Application routing runs its control plane off the Node.js
main thread; packet forwarding stays in a separate System Extension. The Linux
proxy calls manage only KokoroBox's user-session file; setting returns whether
the systemd user manager was updated, and clearing returns `null` when no
managed file exists. See [platform services](platform-services.md) for the
specific paths, validation rules, and privilege boundaries.

## Windows operations

| API | Purpose |
| --- | --- |
| `getCurrentUserSid()`, `isRunningAsAdmin()` | Read the current account SID or elevation state. |
| `relaunchCurrentApplicationWithPrivilege(args, elevated)` | Start a new copy of the current application with the requested privilege level. |
| `ensureKokoroBoxCoreFirewall(mihomoPath, mihomoAlphaPath, applicationPath)` | Manage fixed Firewall rules for validated KokoroBox executables. |
| `listUwpLoopbackApps()`, `setUwpLoopbackExemption(id, enabled)` | List app containers and change an exemption using the returned opaque ID. |

The relaunch API accepts no arbitrary executable path and creates no persistent
elevation. An elevated relaunch presents UAC; an unelevated one uses the
interactive desktop shell token. UWP app records include category, package
type, and exemption status. Windows-only APIs report `UNSUPPORTED_PLATFORM:`
on other operating systems.

## Capability checks

Capability flags describe individual operations rather than general operating
system support. In particular:

| Flag | API group |
| --- | --- |
| `serviceLifecycle` | `runServiceLifecycleElevated` |
| `managedFilePermissions` | `repairManagedFilePermissions` |
| `windowsPrivilegeRelaunch` | `relaunchCurrentApplicationWithPrivilege` |
| `windowsUwpLoopback` | `listUwpLoopbackApps`, `setUwpLoopbackExemption` |
| `linuxTerminalProxy` | Terminal proxy environment calls |

Use the full [`NativeCapabilities` interface](../napi/index.d.ts) for the
other feature flags and the precise API types.
