# Platform services

This document defines the platform-dependent APIs owned by
`kokorobox-native`. Callers should use `getNativeCapabilities()` instead of
inferring support from the operating-system name.

## Network context

`getNetworkContext()` returns a best-effort snapshot:

- `defaultInterface`: interface carrying the preferred default route.
- `defaultService`: user-facing network service used for DNS changes. It is
  available on macOS and mirrors the interface on Linux.
- `dnsServers`: DNS servers attached to that active route or service.
- `ssid`: connected Wi-Fi SSID when the platform exposes one.

macOS derives the interface from `route` and maps it to a Network Service.
Linux reads the default route, prefers `resolvectl`, and falls back to
`/etc/resolv.conf`; SSID lookup prefers `iwgetid` and then NetworkManager.
Windows uses network cmdlets for route/DNS state and the Native Wi-Fi API for
the SSID, avoiding localized `netsh` output.

Missing data is omitted or returned as an empty list. Consumers must not treat
an unavailable SSID as an empty SSID match.

## Mihomo core-file privileges

`getCorePrivilegeStatus(paths)` synchronously inspects the set-user-ID bit.
`setCorePrivileges(paths, enabled)` performs the privileged mutation away from
the Node.js main thread. The capability is available only on macOS and Linux.

The native boundary enforces all of the following before elevation:

1. One to eight absolute paths must be supplied.
2. Every path is canonicalized and must resolve to an existing regular,
   executable file.
3. Every canonical filename must be exactly `mihomo` or `mihomo-alpha`.
4. Enabling changes the owner to root and adds only the set-user-ID bit;
   disabling removes only that bit.

Linux launches `pkexec` with direct `chown` and `chmod` argument vectors.
macOS uses the system administrator authorization dialog and securely quoted,
validated paths. The API intentionally cannot execute caller-provided commands
or operate on unrelated files.

Windows does not use core-file set-user-ID permissions. Use
`isRunningAsAdmin()` and the explicitly scoped Windows APIs instead.
