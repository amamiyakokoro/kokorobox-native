export interface FirewallRule {
  name: string;
  applicationPath: string;
}

export interface UwpLoopbackApp {
  sid: string;
  packageName: string;
  displayName: string;
  enabled: boolean;
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
  networkMonitor: boolean;
  macosServiceManagement: boolean;
  macosApplicationRouting: boolean;
  coreFilePrivileges: boolean;
  serviceIdentity: boolean;
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
  /** Whether the operating system currently has a usable default route. */
  online: boolean;
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

export interface ServiceIdentityOptions {
  /** OS credential namespace, for example `com.amamiyakokoro.KokoroBox`. */
  service: string;
  account: string;
  /** Mode-0600 fallback used only when Linux Secret Service is unavailable. */
  linuxFallbackPath?: string;
}

export interface LegacyServiceIdentity {
  keyId?: string;
  publicKey: string;
  privateKey: string;
}

export interface ServiceIdentityInfo {
  keyId: string;
  publicKey: string;
  backend:
    | "windows-credential-manager"
    | "macos-keychain"
    | "linux-secret-service"
    | "linux-protected-file";
}

/** Opaque native signer. Private key material is never exposed by this object. */
export class ServiceIdentity {
  getInfo(): ServiceIdentityInfo;
  sign(data: string): string;
}

export function openServiceIdentity(
  options: ServiceIdentityOptions,
  legacy?: LegacyServiceIdentity,
): Promise<ServiceIdentity>;
export function deleteServiceIdentity(options: ServiceIdentityOptions): Promise<void>;

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
/** Resolve the executable shipped beside the current platform binding. */
export function getTrafficPresenterPath(): string;
export function findExecutables(options: ExecutableSearchOptions): Promise<ExecutableCandidate[]>;
export function getLaunchAtLogin(options: LaunchAtLoginOptions): Promise<LaunchAtLoginStatus>;
export function setLaunchAtLogin(
  options: LaunchAtLoginOptions,
  enabled: boolean,
): Promise<LaunchAtLoginStatus>;
export function getNetworkContext(): Promise<NetworkContext>;
/** Set DNS on the active macOS network service; [] restores automatic DNS. */
export function setActiveNetworkDns(servers: string[]): Promise<void>;
/** Resolve when the native network snapshot changes, or `undefined` on timeout. */
export function waitForNetworkContextChange(
  previous: NetworkContext,
  timeoutMs?: number,
): Promise<NetworkContext | null>;
/** Query a LaunchDaemon plist embedded in the calling macOS application. */
export function getMacosManagedServiceStatus(plistName: string): MacOSManagedServiceStatus;
export function registerMacosManagedService(plistName: string): MacOSManagedServiceStatus;
export function unregisterMacosManagedService(plistName: string): MacOSManagedServiceStatus;
export function reloadMacosManagedService(plistName: string): MacOSManagedServiceStatus;
export function openMacosLoginItemsSettings(): void;
/** Invoke the versioned macOS Network/System Extension control-plane protocol. */
export function invokeMacosApplicationRouting(request: string): Promise<string>;
export type ServiceLifecycleAction =
  | "init"
  | "install"
  | "uninstall"
  | "start"
  | "stop"
  | "restart";
export interface ServiceLifecycleOptions {
  executablePath: string;
  action: ServiceLifecycleAction;
  publicKey?: string;
  authorizedSid?: string;
  authorizedUid?: number;
}
/** Run only a validated KokoroBox Service lifecycle action with elevation. */
export function runServiceLifecycleElevated(options: ServiceLifecycleOptions): Promise<void>;
/** Remove only the fixed pre-SMAppService KokoroBox LaunchDaemon files. */
export function cleanupLegacyMacosService(): Promise<void>;
/** Stop the fixed KokoroBox SMAppService daemon label. */
export function stopMacosManagedService(): Promise<void>;
/** Repair one file below a validated managed root for the requested Unix owner. */
export function repairManagedFilePermissions(
  target: string,
  managedRoot: string,
  uid: number,
  gid: number,
): Promise<void>;
/** Relaunch the current Windows application at the requested privilege level. */
export function relaunchCurrentApplicationWithPrivilege(
  args: string[],
  elevated: boolean,
): void;
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
export function setupFirewallRules(rules: FirewallRule[]): void;
export function listUwpLoopbackApps(): UwpLoopbackApp[];
export function setUwpLoopbackExemption(sid: string, enabled: boolean): void;
