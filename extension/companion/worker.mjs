// 7C MV3 worker adapter (host-gated). No runtime.onMessage/onConnect
// listener is installed: content scripts/MAIN world MUST NOT be native trust.
import { createCompanion } from "./ledger.mjs";

/**
 * The eventual 7L1 native adapter calls subscribeVerifiedNativeEvent only
 * after authenticating its transport using 7B. It supplies original event
 * source evidence and an isolated reply endpoint unavailable to page scripts.
 * This file intentionally does not declare a native host ID or request login.
 */
export function installVerifiedCompanion({ chromeApi, subscribeVerifiedNativeEvent, verifyHostEvent }) {
  if (!chromeApi?.storage?.session || typeof subscribeVerifiedNativeEvent !== "function")
    throw new Error("7C requires a privileged native-host adapter");
  const companion = createCompanion({
    sessionStore: chromeApi.storage.session, verifyHostEvent
  });
  // Install synchronously for MV3. The native host may return an unsubscriber
  // for explicit retirement. Do not discard in-flight ledger writes on shutdown:
  // claims and ACKs that entered dispatch must durably complete or fail.
  let disposed = false;
  const dispatch = (event, proof) => disposed
    ? Promise.reject(new Error("7C:worker-disposed"))
    : companion.dispatch(event, proof);
  const unsubscribe = subscribeVerifiedNativeEvent(dispatch);
  function dispose() {
    if (disposed) return;
    disposed = true;
    if (typeof unsubscribe === "function") unsubscribe();
  }
  // MV3 onSuspend cannot be awaited. Every claim/ACK is persisted BEFORE reply,
  // never batched at suspend time. An interrupted operation yields no ACK;
  // a restarted worker MUST query repair and never execute the intent again.
  chromeApi.runtime?.onSuspend?.addListener(() => {});
  return Object.freeze({dispatch, dispose});
}
