import assert from "node:assert/strict";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { createNativeDiagnostics, nativeErrorMessage } from "../diagnostics.js";

test("sync and async failures retain identity, causes, and have error severity", async () => {
  const logs = createNativeDiagnostics();
  const cause = new Error("access denied");
  const error = new Error("Open service identity failed", { cause });
  const sync = logs.wrap("openServiceIdentity", () => {
    throw error;
  });
  assert.throws(
    () => sync("PRIVATE KEY MUST NOT BE LOGGED"),
    (value) => value === error,
  );
  const asyncOperation = logs.wrap("openServiceIdentity", async () => {
    throw error;
  });
  await assert.rejects(asyncOperation(), (value) => value === error);
  const entries = logs.drain();
  assert.equal(entries.length, 2);
  for (const entry of entries) {
    assert.equal(entry.level, "error");
    assert.equal(entry.target, "openServiceIdentity");
    assert.match(entry.msg, /Open service identity failed: access denied/);
    assert.ok(Number.isFinite(Date.parse(entry.ts)));
    assert.doesNotMatch(JSON.stringify(entry), /PRIVATE KEY/);
  }
  assert.deepEqual(logs.drain(), []);
});

test("successful polling is quiet, lifecycle actions log without arguments, and filtering works", async () => {
  const logs = createNativeDiagnostics();
  const receiver = { expected: true };
  const get = logs.wrap("getNetworkContext", function () {
    return this;
  });
  assert.equal(get.call(receiver), receiver);
  assert.deepEqual(logs.drain(), []);
  const run = logs.wrap("runServiceLifecycleElevated", async () => 42);
  assert.equal(await run("SECRET"), 42);
  assert.equal(logs.drain()[0].level, "info");
  logs.setLevel("error");
  await run();
  assert.deepEqual(logs.drain(), []);
  assert.throws(() => logs.setLevel("invalid"), /Invalid Native log level/);
  assert.equal(logs.wrap("optional", undefined), undefined);
  logs.setLevel("debug");
  get.call(receiver);
  assert.equal(logs.drain()[0].level, "debug");
});

test("the queue and messages are bounded, overflow is visible and Unicode stays intact", () => {
  const logs = createNativeDiagnostics();
  const fail = logs.wrap("fileToStr", () => {
    throw new Error("😀".repeat(5000));
  });
  for (let i = 0; i < 503; i++) assert.throws(fail);
  const entries = logs.drain();
  assert.equal(entries.length, 501);
  assert.equal(entries[0].level, "warn");
  assert.match(entries[0].msg, /Dropped 3 older/);
  assert.ok(entries[1].msg.length <= 4097);
  assert.equal(entries[1].msg, Buffer.from(entries[1].msg).toString("utf8"));
  assert.deepEqual(logs.drain(), []);
});

test("loader errors include nested causes without cycles or unbounded messages", () => {
  const error = new Error("Load failed");
  error.cause = [new Error("wlanapi.dll is missing"), error];
  assert.equal(
    nativeErrorMessage(error),
    "Load failed: wlanapi.dll is missing",
  );
});

test("the package loader wraps named and default operations while preserving classes", async () => {
  const previous = process.env.NAPI_RS_NATIVE_LIBRARY_PATH;
  process.env.NAPI_RS_NATIVE_LIBRARY_PATH = fileURLToPath(
    new URL("./fixtures/native.cjs", import.meta.url),
  );
  try {
    const binding = await import("../index.js?diagnostics-test");
    assert.deepEqual(binding.getNetworkContext(), { online: true });
    assert.equal(binding.isRunningAsAdmin(), false);
    assert.throws(binding.fileToDataUrl, /file not found/);
    assert.throws(binding.default.fileToDataUrl, /file not found/);
    assert.equal(
      await binding.runServiceLifecycleElevated({ publicKey: "SECRET" }),
      "completed",
    );
    assert.ok(
      new binding.ServiceIdentity() instanceof binding.default.ServiceIdentity,
    );
    assert.equal(binding.default.drainNativeLogs, binding.drainNativeLogs);
    assert.deepEqual(
      binding.drainNativeLogs().map((row) => row.level),
      ["error", "error", "info"],
    );
  } finally {
    if (previous === undefined) delete process.env.NAPI_RS_NATIVE_LIBRARY_PATH;
    else process.env.NAPI_RS_NATIVE_LIBRARY_PATH = previous;
  }
});

test("an error status is logged without turning it into an exception", () => {
  const logs = createNativeDiagnostics();
  const status = { state: "error", message: "Extension refused the policy" };
  assert.equal(
    logs.wrap("applyMacosApplicationRouting", () => status)(),
    status,
  );
  const [entry] = logs.drain();
  assert.equal(entry.level, "error");
  assert.equal(entry.msg, status.message);
});


test("system proxy diagnostics log stable state only, including partial failures", async () => {
  const logs = createNativeDiagnostics();
  const diagnostic = logs.wrap("getSystemProxyDiagnostics", async () => ({
    status: "available", enabled: true,
    pac: { enabled: true, url: "https://USER:SECRET@private.example/token" },
    windows: { proxyServer: "USER:SECRET@private.example", winHttp: { status: "unavailable" }, appContainer: { status: "available" } }
  }));
  await diagnostic();
  const entries = logs.drain();
  assert.equal(entries.length, 3);
  assert.match(entries[0].msg, /Proxy enabled: true/);
  assert.match(entries[1].msg, /PAC configuration detected/);
  assert.equal(entries[2].level, "warn");
  assert.doesNotMatch(JSON.stringify(entries), /USER|SECRET|private|token/);
});
