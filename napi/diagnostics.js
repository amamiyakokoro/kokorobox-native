// Diagnostics belong to the calling application. Do not open a log file or
// retain operation arguments (which may contain keys or signatures).
const maxEntries = 500;
const maxMessageLength = 4096;
const levels = ["debug", "info", "warn", "error"];
const lifecycleOperations = new Set([
  "runServiceLifecycleElevated",
  "registerMacosManagedService",
  "unregisterMacosManagedService",
  "reloadMacosManagedService",
  "cleanupLegacyMacosService",
  "stopMacosManagedService",
  "repairManagedFilePermissions",
  "setCorePrivileges",
  "applyMacosApplicationRouting",
  "stopMacosApplicationRouting",
  "setLaunchAtLogin",
  "setUwpLoopbackExemption",
  "setSystemProxy",
]);

function boundedMessage(value) {
  const text = String(value);
  if (text.length <= maxMessageLength) return text;
  let end = maxMessageLength;
  if (/^[\uDC00-\uDFFF]$/.test(text[end])) end--;
  return text.slice(0, end) + "…";
}

export function nativeErrorMessage(error) {
  const parts = [];
  const seen = new Set();
  function visit(value, depth = 0) {
    if (depth > 4 || seen.has(value)) return;
    seen.add(value);
    const message = value instanceof Error ? value.message : String(value);
    if (message && !parts.includes(message))
      parts.push(boundedMessage(message));
    if (value && typeof value === "object" && value.cause != null) {
      if (Array.isArray(value.cause))
        value.cause.slice(0, 4).forEach((item) => visit(item, depth + 1));
      else visit(value.cause, depth + 1);
    }
  }
  visit(error);
  return boundedMessage(parts.join(": "));
}

export function createNativeDiagnostics() {
  let minimum = "info";
  let entries = [];
  let dropped = 0;
  const record = (level, target, msg) => {
    if (levels.indexOf(level) < levels.indexOf(minimum)) return;
    entries.push({
      ts: new Date().toISOString(),
      level,
      target,
      msg: boundedMessage(msg),
    });
    if (entries.length > maxEntries) {
      entries.shift();
      dropped++;
    }
  };
  return {
    setLevel(level) {
      if (!levels.includes(level))
        throw new TypeError(
          "Invalid Native log level: expected debug, info, warn, or error",
        );
      minimum = level;
    },
    drain() {
      const result = entries;
      entries = [];
      if (dropped) {
        result.unshift({
          ts: new Date().toISOString(),
          level: "warn",
          target: "diagnostics",
          msg: `Dropped ${dropped} older Native log entries`,
        });
        dropped = 0;
      }
      return result;
    },
    wrap(target, operation) {
      if (typeof operation !== "function") return operation;
      const failed = (error) => {
        record("error", target, nativeErrorMessage(error));
        throw error;
      };
      const completed = (value) => {
        if (target === "getSystemProxyDiagnostics") {
          record(value.status === "available" ? "info" : "warn", "SystemProxy", `[SystemProxy] Configuration: ${value.status}; Proxy enabled: ${value.enabled ?? "unknown"}`);
          if (value.pac?.enabled) record("info", "SystemProxy", "[SystemProxy] PAC configuration detected");
          for (const [name, state] of Object.entries({winHttp: value.windows?.winHttp, appContainer: value.windows?.appContainer})) {
            if (state?.status === "unavailable") record("warn", "SystemProxy", `[SystemProxy] ${name}: unavailable`);
          }
          return value;
        }
        if (value && value.state === "error")
          record(
            "error",
            target,
            typeof value.message === "string"
              ? value.message
              : "Native operation returned an error status",
          );
        else if (lifecycleOperations.has(target))
          record("info", target, "Native operation completed");
        else record("debug", target, "Native operation completed");
        return value;
      };
      return function (...args) {
        let result;
        try {
          result = Reflect.apply(operation, this, args);
        } catch (error) {
          return failed(error);
        }
        if (result && typeof result.then === "function")
          return result.then(completed, failed);
        return completed(result);
      };
    },
  };
}

const diagnostics = createNativeDiagnostics();
export const wrapNativeOperation = diagnostics.wrap;
export const drainNativeLogs = diagnostics.drain;
export const setNativeLogLevel = diagnostics.setLevel;
