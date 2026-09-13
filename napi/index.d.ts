export interface FirewallRule {
  name: string;
  applicationPath: string;
}

export interface NativeCapabilities {
  applicationInspection: boolean;
  executableDiscovery: boolean;
  windowsApplicationScan: boolean;
  windowsAccount: boolean;
  windowsElevation: boolean;
  windowsFirewall: boolean;
  launchAtLogin: boolean;
  networkContext: boolean;
  macosServiceManagement: boolean;
  coreFilePrivileges: boolean;
}

export interface ExecutableSearchOptions {
  /** Safe executable basenames, without directory components. */
  names: string[];
  /** Absolute directories searched before PATH and platform-standard locations. */
  additionalPaths?: string[];
  /** Also match executable names beginning with a supplied name. */
  matchNamePrefixes?: boolean;
}

export interface ExecutableCandidate {
  /** The discovered path, which may be a symlink. */
  path: string;
  /** The canonical executable path used for deduplication. */
  canonicalPath: string;
  name: string;
}

export type MacOSManagedServiceStatus =
  | "not-registered"
  | "enabled"
  | "requires-approval"
  | "not-found"
  | "unknown";

export interface LaunchAtLoginOptions {
  /** Stable, filesystem-safe identifier, such as `com.amamiyakokoro.kokorobox`. */
  identifier: string;
  displayName: string;
  executablePath: string;
  arguments?: string[];
}

export interface LaunchAtLoginStatus {
  enabled: boolean;
  /** The entry is registered but macOS requires approval in System Settings. */
  requiresApproval: boolean;
  backend: "windows-current-user-run" | "macos-sm-app-service" | "linux-xdg-autostart";
}

/** Best-effort active-network state. Unavailable fields are omitted instead of guessed. */
export interface NetworkContext {
  defaultInterface?: string;
  /** Active network-service name. Currently populated on macOS and Linux. */
  defaultService?: string;
  dnsServers: string[];
  ssid?: string;
}

export interface CorePrivilegeStatus {
  /** Canonical path validated by the native library. */
  path: string;
  /** Whether the set-user-ID bit is present. */
  granted: boolean;
}

export interface ApplicationInfo {
  executablePath: string;
  executableName: string;
  identifier: string;
  identifierKind: "windows-executable" | "macos-signing-identifier" | "linux-executable";
  iconDataUrl?: string;
}

export interface ApplicationScanResult {
  applications: ApplicationInfo[];
  truncated: boolean;
  unreadableDirectoryCount: number;
}

export interface RuleConvertOptions {
  inputTarget?: "mihomo" | "general" | "egern" | "sing-box";
  inputFormat?:
    | "yaml"
    | "mrs"
    | "text"
    | "json"
    | "srs"
    | "domainset"
    | "ruleset"
    | "ipset";
  inputBehavior?: "auto" | "domain" | "ip" | "classical";
  outputTarget?: "mihomo" | "general" | "egern" | "sing-box";
  outputFormat?:
    | "mrs"
    | "text"
    | "yaml"
    | "json"
    | "srs"
    | "domainset"
    | "ruleset"
    | "ipset";
  outputBehavior?: "auto" | "domain" | "ip" | "classical";
}

export interface RuleOutputInfo {
  behavior?: string;
  format: string;
  count: number;
}

export interface RuleSkippedItem {
  rule: string;
  reason: string;
}

export interface RuleStringResult {
  kind: "rules";
  outputs: Record<string, string>;
  info: Record<string, RuleOutputInfo>;
  skipped: RuleSkippedItem[];
}

export function fileToDataUrl(path: string): string;
export function fileToStr(
  path: string,
  options?: RuleConvertOptions | null,
): RuleStringResult;
export function getAppName(path: string): string;
export function getNativeCapabilities(): NativeCapabilities;
export function findExecutables(options: ExecutableSearchOptions): Promise<ExecutableCandidate[]>;
export function getLaunchAtLogin(options: LaunchAtLoginOptions): Promise<LaunchAtLoginStatus>;
export function setLaunchAtLogin(
  options: LaunchAtLoginOptions,
  enabled: boolean,
): Promise<LaunchAtLoginStatus>;
export function getNetworkContext(): Promise<NetworkContext>;
/** Query a LaunchDaemon plist embedded in the calling macOS application. */
export function getMacosManagedServiceStatus(plistName: string): MacOSManagedServiceStatus;
export function registerMacosManagedService(plistName: string): MacOSManagedServiceStatus;
export function unregisterMacosManagedService(plistName: string): MacOSManagedServiceStatus;
export function reloadMacosManagedService(plistName: string): MacOSManagedServiceStatus;
export function openMacosLoginItemsSettings(): void;
/** Inspect only validated `mihomo` and `mihomo-alpha` executable paths. */
export function getCorePrivilegeStatus(paths: string[]): CorePrivilegeStatus[];
/** Grant or revoke the constrained Unix core-file privilege. */
export function setCorePrivileges(
  paths: string[],
  enabled: boolean,
): Promise<CorePrivilegeStatus[]>;
export function inspectApplication(path: string): Promise<ApplicationInfo>;
export function scanWindowsApplications(
  directory: string,
  maximumResults?: number | null,
  excludedExecutableNames?: string[] | null,
): Promise<ApplicationScanResult>;
export function getCurrentUserSid(): string;
export function isRunningAsAdmin(): boolean;
export function runElevated(command: string, args?: string[]): number;
export function launchElevated(command: string, args?: string[]): void;
export function launchUnelevated(command: string, args?: string[]): void;
export function setupFirewallRules(rules: FirewallRule[]): void;
