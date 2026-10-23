// 8A: browser-owned Chrome API compatibility + static extension worlds.
// Native 7B/7L1 evidence is mandatory; page messages, sender fields and raw
// tab IDs are NEVER authority. This module does not attach a debugger, Send,
// access cookies, or move credentials between browser profiles.
import { installVerifiedCompanion } from "../companion/worker.mjs";

const WORLDS = Object.freeze({
  ISOLATED: "adapter/world/isolated.js",
  MAIN: "adapter/world/main.js"
});
const HOST_KEYS = [
  "profileId", "tabId", "frameId", "documentId", "navigationEpoch",
  "sessionId", "sessionEpoch", "workerGeneration", "hostGeneration",
  "channelId", "origin"
];
const id = (v) => typeof v === "string" && /^[A-Za-z0-9._:-]{1,128}$/.test(v);
const count = (v) => Number.isSafeInteger(v) && v >= 0;
const fail = (code) => { throw new Error("8A:" + code); };
const sameKeys = (obj, keys) => Object.keys(obj).sort().join("|") === [...keys].sort().join("|");

function originOf(url) {
  try {
    const parsed = new URL(url);
    if (parsed.username || parsed.password || parsed.protocol !== "https:") return null;
    return parsed.origin;
  } catch { return null; }
}

function verifiedContext(value, origins) {
  if (!value || typeof value !== "object" || Array.isArray(value) ||
      !sameKeys(value, HOST_KEYS)) fail("native-context-shape");
  if (!["profileId","documentId","sessionId"].every(k => id(value[k])) ||
      !["tabId","frameId","navigationEpoch","sessionEpoch","workerGeneration",
        "hostGeneration","channelId"].every(k => count(value[k])) ||
      value.workerGeneration === 0 || value.hostGeneration === 0 || value.channelId === 0 ||
      !origins.has(value.origin)) fail("native-target");
  return Object.freeze(Object.fromEntries(HOST_KEYS.map(k => [k, value[k]])));
}

// Chrome 109+ can expose callback or Promise APIs. Consume callback lastError
// synchronously (it disappears after the callback). Never accept lastError as
// a successful, empty browser result.
function chromeCall(receiver, name, argument, runtime) {
  if (typeof receiver?.[name] !== "function") fail("api-missing-" + name);
  return new Promise((resolve, reject) => {
    let finished = false;
    const done = (error, value) => {
      if (finished) return;
      finished = true;
      if (error) reject(Error("8A:chrome-api-failed:" + name));
      else resolve(value);
    };
    try {
      const result = receiver[name](argument, value => {
        const error = runtime?.lastError;
        done(error, value);
      });
      if (result && typeof result.then === "function") {
        result.then(value => done(null, value), error => done(error));
      }
    } catch (error) { done(error); }
  });
}

export function createChromeWorkerAdapter({
  chromeApi, subscribeVerifiedNativeEvent, verifyHostEvent,
  authorizeNativeTarget, allowedOrigins
}) {
  if (typeof subscribeVerifiedNativeEvent !== "function" ||
      typeof verifyHostEvent !== "function" ||
      typeof authorizeNativeTarget !== "function" ||
      !chromeApi?.storage?.session ||
      !chromeApi?.scripting || !chromeApi?.tabs || !chromeApi?.webNavigation)
    fail("missing-privileged-host");
  if (!Array.isArray(allowedOrigins) || allowedOrigins.length === 0 ||
      allowedOrigins.length > 16 || allowedOrigins.some(s =>
        typeof s !== "string" || originOf(s) !== s))
    fail("invalid-origin-policy");
  const origins = new Set(allowedOrigins);
  const companion = installVerifiedCompanion({
    chromeApi, subscribeVerifiedNativeEvent, verifyHostEvent
  });
  const active = new Set();
  const fences = new Map();

  async function reauthorize(proof, expected) {
    const current = verifiedContext(await authorizeNativeTarget(proof), origins);
    if (HOST_KEYS.some(k => current[k] !== expected[k])) fail("native-target-changed");
  }
  async function inspect(current) {
    const [tab, frame] = await Promise.all([
      chromeCall(chromeApi.tabs, "get", current.tabId, chromeApi.runtime),
      chromeCall(chromeApi.webNavigation, "getFrame",
        {tabId:current.tabId,frameId:current.frameId}, chromeApi.runtime)
    ]);
    if (!tab || tab.id !== current.tabId ||
        originOf(tab.url) !== current.origin ||
        !frame || frame.frameId !== current.frameId ||
        frame.documentId !== current.documentId ||
        originOf(frame.url) !== current.origin) fail("navigation-or-origin");
  }

  // Injection only of audited, static extension files, never caller JS,
  // arbitrary CDP, page-derived identity or dynamically named resources.
  // A script can execute before a navigation race is observed post-injection:
  // these fixed scripts are strictly inert and MUST NOT perform user actions.
  async function inject({nativeProof, world}) {
    if (!Object.hasOwn(WORLDS, world)) fail("world-denied");
    const current = verifiedContext(
      await authorizeNativeTarget(nativeProof), origins);
    const scope = JSON.stringify([current.profileId,current.tabId,current.frameId]);
    if (active.has(scope)) fail("injection-busy");
    active.add(scope);
    try {
      const prior = fences.get(scope);
      if (prior && (current.hostGeneration < prior.hostGeneration ||
          current.navigationEpoch < prior.navigationEpoch ||
          (current.documentId !== prior.documentId &&
            (current.navigationEpoch <= prior.navigationEpoch ||
             current.hostGeneration <= prior.hostGeneration))))
        fail("stale-native-generation");
      await reauthorize(nativeProof, current);
      await inspect(current);
      await reauthorize(nativeProof, current);
      // Only documentIds is used (frameIds and documentIds are mutually
      // exclusive in chrome.scripting). Unsupported hosts MUST fail closed.
      const results = await chromeCall(chromeApi.scripting, "executeScript", {
        target:{tabId:current.tabId,documentIds:[current.documentId]},
        files:[WORLDS[world]],world
      }, chromeApi.runtime);
      await reauthorize(nativeProof, current);
      await inspect(current);
      if (!Array.isArray(results) || results.length !== 1 ||
          results[0]?.frameId !== current.frameId ||
          results[0]?.documentId !== current.documentId)
        fail("injection-result-mismatch");
      fences.set(scope, {hostGeneration:current.hostGeneration,
        navigationEpoch:current.navigationEpoch, documentId:current.documentId});
      return Object.freeze({world,profileId:current.profileId,
        tabId:current.tabId,frameId:current.frameId,
        documentId:current.documentId,hostGeneration:current.hostGeneration});
    } finally {
      active.delete(scope);
    }
  }
  return Object.freeze({inject, companion});
}
