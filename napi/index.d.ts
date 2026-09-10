export interface FirewallRule {
  name: string;
  applicationPath: string;
}

export interface NativeCapabilities {
  applicationInspection: boolean;
  windowsApplicationScan: boolean;
  windowsAccount: boolean;
  windowsElevation: boolean;
  windowsFirewall: boolean;
  launchAtLogin: boolean;
  networkContext: boolean;
  coreFilePrivileges: boolean;
}

export interface LaunchAtLoginOptions {
  /** Stable, filesystem-safe identifier, such as `com.amamiyakokoro.kokorobox`. */
  identifier: string;
  displayName: string;
  executablePath: string;
  arguments?: string[];
}

export interface LaunchAtLoginStatus {
  enabled: boolean;
  backend: "windows-task-scheduler" | "macos-login-item" | "linux-xdg-autostart";
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
export function getLaunchAtLogin(options: LaunchAtLoginOptions): Promise<LaunchAtLoginStatus>;
export function setLaunchAtLogin(
  options: LaunchAtLoginOptions,
  enabled: boolean,
): Promise<LaunchAtLoginStatus>;
export function getNetworkContext(): Promise<NetworkContext>;
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
export function setupFirewallRules(rules: FirewallRule[]): void;
