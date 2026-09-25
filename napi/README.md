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

## Documentation

- [JavaScript API guide](../docs/api.md)
- [TypeScript declarations](index.d.ts)
- [Platform behavior and security contract](../docs/platform-services.md)
- [Traffic presenter protocol](../docs/traffic-presenter.md)
- [Repository layout and build commands](../README.md)

Derived from [UruhaLushia/sparkle-native](https://github.com/UruhaLushia/sparkle-native)
and licensed under GPL-3.0-only.
