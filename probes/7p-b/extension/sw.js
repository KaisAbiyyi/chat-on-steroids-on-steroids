// Synthetic same-tab browser debugger roundtrip ONLY. No user or provider tabs.
const SYNTHETIC_ORIGIN = "https://synthetic.cos.test/";
chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
  if (!message || message.kind !== "synthetic-check") return false;
  const tabId = sender.tab && sender.tab.id;
  const senderUrl = sender.url || "";
  if (!senderUrl.startsWith(SYNTHETIC_ORIGIN) || !Number.isInteger(tabId)) {
    sendResponse({ kind: "ack", debuggerApi: "unowned-sender" });
    return false;
  }
  if (!chrome.debugger || !chrome.debugger.getTargets ||
      !chrome.debugger.attach || !chrome.debugger.detach) {
    sendResponse({ kind: "ack", debuggerApi: "unavailable" });
    return false;
  }
  let responded = false;
  const answer = (value) => {
    if (responded) return;
    responded = true;
    sendResponse({ kind: "ack", debuggerApi: value });
  };
  chrome.debugger.getTargets((targets) => {
    if (chrome.runtime.lastError || !Array.isArray(targets)) {
      answer("target-enumeration-error");
      return;
    }
    const allowed = targets.some(t =>
      t.tabId === tabId && typeof t.url === "string" && t.url.startsWith(SYNTHETIC_ORIGIN));
    if (!allowed) {
      answer("synthetic-target-missing");
      return;
    }
    const owned = { tabId };
    const observed = [];
    const onEvent = (source, method) => {
      if (source.tabId === tabId && method === "Runtime.consoleAPICalled")
        observed.push(method);
    };
    chrome.debugger.attach(owned, "1.3", () => {
      if (chrome.runtime.lastError) {
        answer("attach-refused");
        return;
      }
      chrome.debugger.onEvent.addListener(onEvent);
      const finish = (result) => {
        chrome.debugger.onEvent.removeListener(onEvent);
        chrome.debugger.detach(owned, () => {
          const detachError = chrome.runtime.lastError;
          answer(detachError ? "detach-refused" : result);
        });
      };
      chrome.debugger.sendCommand(owned, "Runtime.enable", {}, () => {
        if (chrome.runtime.lastError) {
          finish("runtime-enable-refused");
          return;
        }
        chrome.debugger.sendCommand(owned, "Runtime.evaluate", {
          expression: "console.log('COS7PB_SYNTHETIC_EVENT'); document.title",
          returnByValue: true
        }, (result) => {
          const failed = !!chrome.runtime.lastError;
          const title = result && result.result && result.result.value;
          setTimeout(() => {
            finish(!failed && title === "7P-B Synthetic" && observed.length > 0
              ? "roundtrip-event-detached" : "roundtrip-incomplete");
          }, 125);
        });
      });
    });
  });
  return true;
});
