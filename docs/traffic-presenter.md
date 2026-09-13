# Traffic presenter contract

`kokorobox-traffic-presenter` is an unprivileged per-user process shipped in
each `kokorobox-native` platform package. The Desktop main process owns its
lifetime and writes one JSON object per line to standard input. Closing the
stream is equivalent to a shutdown command.

The presenter does not open the Mihomo controller, named pipes, sockets, or
configuration files. Desktop remains the only source of traffic samples and
never sends controller credentials to this process.

## Protocol version 1

Every message contains `"version": 1` and exactly one of the following shapes:

```json
{"version":1,"type":"configure","visible":true,"layout":"horizontal","theme":"system"}
{"version":1,"type":"traffic","up":1024,"down":2048}
{"version":1,"type":"unavailable"}
{"version":1,"type":"shutdown"}
```

`up` and `down` are non-negative byte-per-second counters. `layout` is either
`horizontal` or `stacked`; `theme` is `system`, `light`, or `dark`. Receivers
reject unknown fields and unsupported protocol versions instead of guessing.

The current platform presentation is:

- Windows: a transparent, non-activating popup attached adjacent to the notification area. The
  popup is reattached after Explorer recreates the taskbar and falls back to a topmost overlay if
  the taskbar temporarily rejects attachment.
- macOS: an `NSStatusItem`-backed menu-bar title.
- Linux: a StatusNotifierItem using the KSNI D-Bus backend.

The process is single-instance on Windows and exits when Desktop closes its
input stream on every platform.
