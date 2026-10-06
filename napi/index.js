// prettier-ignore
/* eslint-disable */
// @ts-nocheck

import { existsSync } from 'node:fs'
import { dirname, join } from "node:path";
import { createRequire } from "node:module";
import {
  nativeErrorMessage,
  wrapNativeOperation,
  drainNativeLogs,
  setNativeLogLevel,
} from "./diagnostics.js";
export { drainNativeLogs, setNativeLogLevel };

const require = createRequire(import.meta.url);
const __dirname = new URL(".", import.meta.url).pathname.replace(
  /^\/([A-Za-z]:)/,
  "$1",
);

const packageName = "kokorobox-native";
const binaryName = "kokorobox-native";
const loadErrors = [];
let loadedTuple = null;

function requireLocal(tuple) {
  const filename = join(__dirname, `${binaryName}.${tuple}.node`);
  if (!existsSync(filename)) return null;
  try {
    return require(filename);
  } catch (err) {
    loadErrors.push(err);
    return null;
  }
}

function requirePackage(tuple) {
  try {
    return require(`${packageName}-${tuple}`);
  } catch (err) {
    loadErrors.push(err);
    return null;
  }
}

function requireBinding(tuple) {
  const binding = requireLocal(tuple) || requirePackage(tuple);
  if (binding) loadedTuple = tuple;
  return binding;
}

export function getTrafficPresenterPath() {
  const filename = `kokorobox-traffic-presenter${process.platform === "win32" ? ".exe" : ""}`;
  const localPath = join(__dirname, filename);
  if (existsSync(localPath)) return localPath;

  if (loadedTuple) {
    const packageEntry = require.resolve(`${packageName}-${loadedTuple}`);
    const packagedPath = join(dirname(packageEntry), filename);
    if (existsSync(packagedPath)) return packagedPath;
  }

  throw new Error(
    `Traffic presenter is missing for ${process.platform} ${process.arch}`,
  );
}

export function getPortableUpdaterPath() {
  if (process.platform !== "win32") {
    throw new Error("Portable updater is only available on Windows");
  }
  const filename = "kokorobox-portable-updater.exe";
  const localPath = join(__dirname, filename);
  if (existsSync(localPath)) return localPath;
  if (loadedTuple) {
    const packageEntry = require.resolve(`${packageName}-${loadedTuple}`);
    const packagedPath = join(dirname(packageEntry), filename);
    if (existsSync(packagedPath)) return packagedPath;
  }
  throw new Error(
    `Portable updater is missing for ${process.platform} ${process.arch}`,
  );
}

function requireNative() {
  if (process.env.NAPI_RS_NATIVE_LIBRARY_PATH) {
    try {
      return require(process.env.NAPI_RS_NATIVE_LIBRARY_PATH);
    } catch (err) {
      loadErrors.push(err);
    }
  }

  if (process.platform === "win32") {
    if (process.arch === "x64") return requireBinding("win32-x64-msvc");
    if (process.arch === "arm64") return requireBinding("win32-arm64-msvc");
  } else if (process.platform === "darwin") {
    if (process.arch === "x64") return requireBinding("darwin-x64");
    if (process.arch === "arm64") return requireBinding("darwin-arm64");
  } else if (process.platform === "linux") {
    if (process.arch === "x64") return requireBinding("linux-x64-gnu");
    if (process.arch === "arm64") return requireBinding("linux-arm64-gnu");
  }

  loadErrors.push(
    new Error(
      `Unsupported OS or architecture: ${process.platform} ${process.arch}`,
    ),
  );
  return null;
}

const nativeBinding = requireNative();

if (!nativeBinding) {
  const detail = nativeErrorMessage({
    cause: loadErrors,
    toString: () => "Failed to load kokorobox-native binding",
  });
  const error = new Error(detail);
  error.cause = loadErrors;
  throw error;
}

// Preserve native classes and unsupported exports; wrap callable operations
// for both named imports and the default binding without changing arguments.
const diagnosticBinding = {};
for (const [name, descriptor] of Object.entries(
  Object.getOwnPropertyDescriptors(nativeBinding),
)) {
  if (/^[a-z]/.test(name) && typeof descriptor.value === "function") {
    descriptor.value = wrapNativeOperation(name, descriptor.value);
  }
  Object.defineProperty(diagnosticBinding, name, descriptor);
}
diagnosticBinding.drainNativeLogs = drainNativeLogs;
diagnosticBinding.setNativeLogLevel = setNativeLogLevel;
export default diagnosticBinding;
export const fileToDataUrl = diagnosticBinding.fileToDataUrl;
export const fileToStr = diagnosticBinding.fileToStr;
export const getAppName = diagnosticBinding.getAppName;
export const getNativeCapabilities = diagnosticBinding.getNativeCapabilities;
export const getWindowsServiceStatus =
  diagnosticBinding.getWindowsServiceStatus;
export const getLinuxServiceStatus = diagnosticBinding.getLinuxServiceStatus;
export const setTerminalProxyEnvironment =
  diagnosticBinding.setTerminalProxyEnvironment;
export const clearTerminalProxyEnvironment =
  diagnosticBinding.clearTerminalProxyEnvironment;
export const findExecutables = diagnosticBinding.findExecutables;
export const getLaunchAtLogin = diagnosticBinding.getLaunchAtLogin;
export const setLaunchAtLogin = diagnosticBinding.setLaunchAtLogin;
export const getNetworkContext = diagnosticBinding.getNetworkContext;
export const waitForNetworkContextChange =
  diagnosticBinding.waitForNetworkContextChange;
export const getMacosManagedServiceStatus =
  diagnosticBinding.getMacosManagedServiceStatus;
export const getMacosServiceProcessStatus =
  diagnosticBinding.getMacosServiceProcessStatus;
export const registerMacosManagedService =
  diagnosticBinding.registerMacosManagedService;
export const unregisterMacosManagedService =
  diagnosticBinding.unregisterMacosManagedService;
export const reloadMacosManagedService =
  diagnosticBinding.reloadMacosManagedService;
export const openMacosLoginItemsSettings =
  diagnosticBinding.openMacosLoginItemsSettings;
export const applyMacosApplicationRouting =
  diagnosticBinding.applyMacosApplicationRouting;
export const getMacosApplicationRoutingStatus =
  diagnosticBinding.getMacosApplicationRoutingStatus;
export const stopMacosApplicationRouting =
  diagnosticBinding.stopMacosApplicationRouting;
export const openMacosApplicationRoutingSettings =
  diagnosticBinding.openMacosApplicationRoutingSettings;
export const runServiceLifecycleElevated =
  diagnosticBinding.runServiceLifecycleElevated;
export const cleanupLegacyMacosService =
  diagnosticBinding.cleanupLegacyMacosService;
export const stopMacosManagedService =
  diagnosticBinding.stopMacosManagedService;
export const repairManagedFilePermissions =
  diagnosticBinding.repairManagedFilePermissions;
export const relaunchCurrentApplicationWithPrivilege =
  diagnosticBinding.relaunchCurrentApplicationWithPrivilege;
export const getCorePrivilegeStatus = diagnosticBinding.getCorePrivilegeStatus;
export const setCorePrivileges = diagnosticBinding.setCorePrivileges;
export const inspectApplication = diagnosticBinding.inspectApplication;
export const scanWindowsApplications =
  diagnosticBinding.scanWindowsApplications;
export const getCurrentUserSid = diagnosticBinding.getCurrentUserSid;
export const isRunningAsAdmin = diagnosticBinding.isRunningAsAdmin;
export const ensureKokoroBoxCoreFirewall =
  diagnosticBinding.ensureKokoroBoxCoreFirewall;
export const listUwpLoopbackApps = diagnosticBinding.listUwpLoopbackApps;
export const setUwpLoopbackExemption =
  diagnosticBinding.setUwpLoopbackExemption;
export const ServiceIdentity = diagnosticBinding.ServiceIdentity;
export const openServiceIdentity = diagnosticBinding.openServiceIdentity;
export const deleteServiceIdentity = diagnosticBinding.deleteServiceIdentity;

export const getSystemProxyDiagnostics = diagnosticBinding.getSystemProxyDiagnostics;
export const setSystemProxy = diagnosticBinding.setSystemProxy;
export const getSystemDnsDiagnostics = diagnosticBinding.getSystemDnsDiagnostics;

export const inspectCoreProcess = diagnosticBinding.inspectCoreProcess;
export const stopCoreProcess = diagnosticBinding.stopCoreProcess;

export const reconcileMacosApplicationRouting = diagnosticBinding.reconcileMacosApplicationRouting;

export const validateCoreProfile = diagnosticBinding.validateCoreProfile;

export const getServiceProcessStatus = diagnosticBinding.getServiceProcessStatus;
