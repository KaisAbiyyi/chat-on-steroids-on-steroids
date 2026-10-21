// ISOLATED world marker should never leak into page JS global.
window.__cosIsolatedSecret = "isolation-only";
document.documentElement.dataset.cosIsolated = "yes";
try {
  chrome.runtime.sendMessage({ kind: "synthetic-check" }, (reply) => {
    if (chrome.runtime.lastError || !reply || reply.kind !== "ack") {
      document.documentElement.dataset.cosWorkerAck = "error";
      document.documentElement.dataset.cosDebugger = "unknown";
      return;
    }
    document.documentElement.dataset.cosWorkerAck = "ok";
    document.documentElement.dataset.cosDebugger = reply.debuggerApi;
  });
} catch (_) {
  document.documentElement.dataset.cosWorkerAck = "error";
  document.documentElement.dataset.cosDebugger = "unknown";
}
