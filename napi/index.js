// prettier-ignore
/* eslint-disable */
// @ts-nocheck

import { existsSync } from 'node:fs'
import { dirname, join } from "node:path";
import { createRequire } from "node:module";

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
  throw new Error(`Portable updater is missing for ${process.platform} ${process.arch}`);
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
  const error = new Error("Failed to load kokorobox-native binding");
  error.cause = loadErrors;
  throw error;
}

export default nativeBinding;
export const fileToDataUrl = nativeBinding.fileToDataUrl;
export const fileToStr = nativeBinding.fileToStr;
export const getAppName = nativeBinding.getAppName;
export const getNativeCapabilities = nativeBinding.getNativeCapabilities;
export const getWindowsServiceStatus = nativeBinding.getWindowsServiceStatus;
export const getLinuxServiceStatus = nativeBinding.getLinuxServiceStatus;
export const setTerminalProxyEnvironment = nativeBinding.setTerminalProxyEnvironment;
export const clearTerminalProxyEnvironment = nativeBinding.clearTerminalProxyEnvironment;
export const findExecutables = nativeBinding.findExecutables;
export const getLaunchAtLogin = nativeBinding.getLaunchAtLogin;
export const setLaunchAtLogin = nativeBinding.setLaunchAtLogin;
export const getNetworkContext = nativeBinding.getNetworkContext;
export const waitForNetworkContextChange = nativeBinding.waitForNetworkContextChange;
export const getMacosManagedServiceStatus = nativeBinding.getMacosManagedServiceStatus;
export const getMacosServiceProcessStatus = nativeBinding.getMacosServiceProcessStatus;
export const registerMacosManagedService = nativeBinding.registerMacosManagedService;
export const unregisterMacosManagedService = nativeBinding.unregisterMacosManagedService;
export const reloadMacosManagedService = nativeBinding.reloadMacosManagedService;
export const openMacosLoginItemsSettings = nativeBinding.openMacosLoginItemsSettings;
export const applyMacosApplicationRouting = nativeBinding.applyMacosApplicationRouting;
export const getMacosApplicationRoutingStatus = nativeBinding.getMacosApplicationRoutingStatus;
export const stopMacosApplicationRouting = nativeBinding.stopMacosApplicationRouting;
export const openMacosApplicationRoutingSettings = nativeBinding.openMacosApplicationRoutingSettings;
export const runServiceLifecycleElevated = nativeBinding.runServiceLifecycleElevated;
export const cleanupLegacyMacosService = nativeBinding.cleanupLegacyMacosService;
export const stopMacosManagedService = nativeBinding.stopMacosManagedService;
export const repairManagedFilePermissions = nativeBinding.repairManagedFilePermissions;
export const relaunchCurrentApplicationWithPrivilege = nativeBinding.relaunchCurrentApplicationWithPrivilege;
export const getCorePrivilegeStatus = nativeBinding.getCorePrivilegeStatus;
export const setCorePrivileges = nativeBinding.setCorePrivileges;
export const inspectApplication = nativeBinding.inspectApplication;
export const scanWindowsApplications = nativeBinding.scanWindowsApplications;
export const getCurrentUserSid = nativeBinding.getCurrentUserSid;
export const isRunningAsAdmin = nativeBinding.isRunningAsAdmin;
export const ensureKokoroBoxCoreFirewall = nativeBinding.ensureKokoroBoxCoreFirewall;
export const listUwpLoopbackApps = nativeBinding.listUwpLoopbackApps;
export const setUwpLoopbackExemption = nativeBinding.setUwpLoopbackExemption;
export const ServiceIdentity = nativeBinding.ServiceIdentity;
export const openServiceIdentity = nativeBinding.openServiceIdentity;
export const deleteServiceIdentity = nativeBinding.deleteServiceIdentity;
