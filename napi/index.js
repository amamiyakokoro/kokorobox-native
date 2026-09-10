// prettier-ignore
/* eslint-disable */
// @ts-nocheck

import { existsSync } from 'node:fs'
import { join } from "node:path";
import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
const __dirname = new URL(".", import.meta.url).pathname.replace(
  /^\/([A-Za-z]:)/,
  "$1",
);

const packageName = "kokorobox-native";
const binaryName = "kokorobox-native";
const loadErrors = [];

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
  return requireLocal(tuple) || requirePackage(tuple);
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
    if (process.arch === "ia32") return requireBinding("win32-ia32-msvc");
  } else if (process.platform === "darwin") {
    if (process.arch === "x64") return requireBinding("darwin-x64");
    if (process.arch === "arm64") return requireBinding("darwin-arm64");
  } else if (process.platform === "linux") {
    if (process.arch === "x64") return requireBinding("linux-x64-gnu");
    if (process.arch === "arm64") return requireBinding("linux-arm64-gnu");
    if (process.arch === "loong64") return requireBinding("linux-loong64-gnu");
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
export const inspectApplication = nativeBinding.inspectApplication;
export const scanWindowsApplications = nativeBinding.scanWindowsApplications;
export const getCurrentUserSid = nativeBinding.getCurrentUserSid;
export const isRunningAsAdmin = nativeBinding.isRunningAsAdmin;
export const runElevated = nativeBinding.runElevated;
export const setupFirewallRules = nativeBinding.setupFirewallRules;
