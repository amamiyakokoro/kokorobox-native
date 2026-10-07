// Exercise the production selector without opening Settings or requesting approval.
#include "../macos_app_routing.mm"
#include <cstdio>

static bool checkURL(NSInteger majorVersion, NSString *expected) {
  NSString *actual = KBSystemSettingsURLForMajorVersion(majorVersion).absoluteString;
  if ([actual isEqualToString:expected]) return true;
  fprintf(stderr, "Unexpected Settings URL for macOS %ld\n", (long)majorVersion);
  return false;
}

int main() {
  @autoreleasepool {
    NSString *legacy = @"x-apple.systempreferences:com.apple.preference.security?General";
    NSString *extensions = @"x-apple.systempreferences:com.apple.ExtensionsPreferences?extensionPointIdentifier=com.apple.system_extension.network_extension.extension-point&bundleIdentifier=com.amamiyakokoro.app";
    for (NSInteger version : {13, 14}) {
      if (!checkURL(version, legacy)) return 1;
    }
    for (NSInteger version : {15, 16, 26}) {
      if (!checkURL(version, extensions)) return 1;
    }
    NSInteger current = [NSProcessInfo processInfo].operatingSystemVersion.majorVersion;
    if (![KBSystemSettingsURL().absoluteString isEqualToString:current >= 15 ? extensions : legacy]) return 1;
    puts("macOS Settings URL: legacy, modern and current OS selectors passed");
  }
  return 0;
}
