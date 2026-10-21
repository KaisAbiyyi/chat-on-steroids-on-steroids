// Observe API surface only. Deliberately do not attach to any browser target.
chrome.runtime.onMessage.addListener((message, _sender, sendResponse) => {
  if (!message || message.kind !== "synthetic-check") return false;
  try {
    if (!chrome.debugger || typeof chrome.debugger.getTargets !== "function") {
      sendResponse({ kind: "ack", debuggerApi: "unavailable" });
      return false;
    }
    chrome.debugger.getTargets((_targets) => {
      sendResponse({
        kind: "ack",
        debuggerApi: chrome.runtime.lastError ? "error" : "available"
      });
    });
    return true; // keep MV3 message port alive for asynchronous callback
  } catch (_) {
    sendResponse({ kind: "ack", debuggerApi: "error" });
    return false;
  }
});
