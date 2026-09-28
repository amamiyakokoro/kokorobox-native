class ServiceIdentity {}
module.exports = {
  ServiceIdentity,
  getNetworkContext: () => ({ online: true }),
  fileToDataUrl: () => {
    throw new Error("file not found");
  },
  runServiceLifecycleElevated: async () => "completed",
};
// N-API functions can be non-enumerable and non-configurable.
Object.defineProperty(module.exports, "isRunningAsAdmin", { value: () => false });
