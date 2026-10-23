// 7C — extension-side intent claim / ACK / repair ledger for a native-attested MV3 adapter.
// This module intentionally performs NO provider actions, Send, debugger, or DOM writes.
// The native source authenticator from 7B must gate every event BEFORE calling this module.
export const VERSION = 14;
export const MAX_CLAIMS = 1024;
export const STORAGE_KEY = "cosos7cLedgerV1";
const ID = /^[A-Za-z0-9._:-]{1,128}$/;
const DIGEST = /^[a-f0-9]{64}$/;
const MAX_SAFE = Number.MAX_SAFE_INTEGER;

export class CompanionError extends Error {
  constructor(code) { super(code); this.name = "CompanionError"; this.code = code; }
}
const deny = (code) => { throw new CompanionError(code); };
const safe = (n) => Number.isSafeInteger(n) && n >= 0;
const bounded = (s) => typeof s === "string" && ID.test(s);
const own = (x, k) => Object.prototype.hasOwnProperty.call(x, k);
const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);

function trustedContext(value) {
  if (!value || typeof value !== "object" || Array.isArray(value)) deny("untrusted-host");
  const keys = [
    "profileId", "tabId", "frameId", "documentId", "navigationEpoch",
    "sessionId", "sessionEpoch", "workerGeneration", "hostGeneration", "channelId"
  ];
  if (Object.keys(value).sort().join("|") !== [...keys].sort().join("|")) deny("host-shape");
  for (const k of ["profileId", "documentId", "sessionId"]) {
    if (!bounded(value[k])) deny("host-identity");
  }
  for (const k of ["tabId", "frameId", "navigationEpoch", "sessionEpoch", "workerGeneration", "hostGeneration", "channelId"]) {
    if (!safe(value[k])) deny("host-identity");
  }
  if (value.workerGeneration === 0 || value.hostGeneration === 0 || value.channelId === 0) deny("host-generation");
  return Object.fromEntries(keys.map(k => [k, value[k]]));
}
const scopeKey = (c) => JSON.stringify([c.profileId, c.tabId, c.frameId]);
const sessionKey = (c) => JSON.stringify([c.sessionId, c.sessionEpoch]);
const channelKey = (c) => JSON.stringify([c.channelId, c.hostGeneration, c.workerGeneration]);
const identityKey = (c) => JSON.stringify([
  c.profileId, c.tabId, c.frameId, c.documentId, c.navigationEpoch,
  c.sessionId, c.sessionEpoch, c.workerGeneration, c.hostGeneration, c.channelId
]);
const emptyState = () => ({ format: 1, channels: {}, tombstones: {}, sessions: {}, intents: {} });

function validateState(state) {
  if (!state || typeof state !== "object" || Array.isArray(state) ||
      state.format !== 1 || !state.channels || !state.tombstones || !state.sessions || !state.intents ||
      [state.channels, state.tombstones, state.sessions, state.intents].some(x =>
        typeof x !== "object" || x === null || Array.isArray(x))) deny("corrupt-ledger");
  if (Object.keys(state.channels).length > MAX_CLAIMS ||
      Object.keys(state.tombstones).length > MAX_CLAIMS ||
      Object.keys(state.sessions).length > MAX_CLAIMS ||
      Object.keys(state.intents).length > MAX_CLAIMS) deny("capacity");
  return state;
}
function parseIntent(event) {
  if (typeof event?.intentId !== "string" || !bounded(event.intentId) ||
      !DIGEST.test(event.digest ?? "")) deny("invalid-intent");
  return { intentId: event.intentId, digest: event.digest };
}
function parseEvent(event) {
  if (!event || typeof event !== "object" || Array.isArray(event) ||
      event.protocol !== VERSION || !Number.isSafeInteger(event.sequence) || event.sequence < 1)
    deny("invalid-envelope");
  if (!["attach", "claim", "ack", "repair", "retire"].includes(event.kind))
    deny("invalid-event");
  // Never permit implicit credentials or unreviewed extension-to-native commands.
  const valid = {
    attach: ["kind", "protocol", "sequence"],
    retire: ["kind", "protocol", "sequence"],
    claim: ["kind", "protocol", "sequence", "intentId", "digest"],
    ack: ["kind", "protocol", "sequence", "intentId", "digest", "receiptId"],
    repair: ["kind", "protocol", "sequence", "intentId", "digest"]
  };
  if (!same(Object.keys(event).sort(), [...valid[event.kind]].sort())) deny("unknown-event-field");
  if (["claim", "ack", "repair"].includes(event.kind)) parseIntent(event);
  if (event.kind === "ack" && !bounded(event.receiptId)) deny("invalid-receipt");
}

function locate(state, trusted) {
  const slot = state.channels[scopeKey(trusted)];
  if (!slot || slot.identity !== identityKey(trusted) || slot.channel !== channelKey(trusted))
    deny("stale-channel");
  return slot;
}

function attach(state, trusted, event) {
  const key = scopeKey(trusted);
  const owner = state.sessions[sessionKey(trusted)];
  if (owner && owner !== key) deny("session-already-owned");
  const previous = state.channels[key] ?? state.tombstones[key];
  const isRetired = !state.channels[key] && !!previous;
  if (previous) {
    if (trusted.sessionId !== previous.context.sessionId ||
        trusted.sessionEpoch !== previous.context.sessionEpoch) deny("session-rebind");
    if (trusted.navigationEpoch < previous.context.navigationEpoch ||
        trusted.workerGeneration < previous.context.workerGeneration ||
        trusted.hostGeneration < previous.context.hostGeneration) deny("stale-generation");
    if (trusted.navigationEpoch === previous.context.navigationEpoch &&
        trusted.documentId !== previous.context.documentId) deny("document-rebind");
    if (!isRetired && same(trusted, previous.context)) {
      if (event.sequence !== previous.nextSequence) deny("sequence");
      previous.nextSequence++;
      return { kind: "attached", replay: false };
    }
    // A worker/channel recovery MUST advance host or worker generation and
    // never implicitly reassign an outstanding intent or make it executable.
    // Native 7B replacement always requires a STRICT host-generation bump.
    // Worker-only increments cannot impersonate a fresh authenticated channel.
    if (trusted.hostGeneration <= previous.context.hostGeneration)
      deny("generation-not-advanced");
    if (event.sequence !== 1) deny("sequence");
  } else if (event.sequence !== 1) deny("sequence");
  if (!state.channels[key] && Object.keys(state.channels).length >= MAX_CLAIMS) deny("capacity");
  state.sessions[sessionKey(trusted)] = key;
  delete state.tombstones[key];
  state.channels[key] = {
    context: trusted, identity: identityKey(trusted), channel: channelKey(trusted),
    nextSequence: event.sequence + 1
  };
  return { kind: "attached", replay: false };
}

function process(state, trusted, event) {
  if (event.kind === "attach") return attach(state, trusted, event);
  const slot = locate(state, trusted);
  if (event.sequence !== slot.nextSequence) deny("sequence");
  if (event.kind === "retire") {
    state.tombstones[scopeKey(trusted)] = { context: trusted };
    delete state.channels[scopeKey(trusted)];
    // First-proof session attribution is deliberately never released by an
    // extension-generated event; a separate native 7B handoff must approve it.
    return { kind: "retired" };
  }
  const { intentId, digest } = parseIntent(event);
  // Intent IDs are untrusted even after host authentication. Plain objects
  // inherit keys such as "__proto__", "constructor" and "toString"; treating
  // an inherited property as a ledger claim (or assigning to __proto__) can
  // corrupt ownership or silently lose an acknowledged claim.
  const existing = own(state.intents, intentId) ? state.intents[intentId] : undefined;
  if (event.kind === "claim") {
    if (existing) {
      if (existing.owner !== scopeKey(trusted) || existing.digest !== digest) deny("conflicting-claim");
      slot.nextSequence++;
      return { kind: "claim", status: existing.status, duplicate: true };
    }
    if (Object.keys(state.intents).length >= MAX_CLAIMS) deny("capacity");
    Object.defineProperty(state.intents, intentId, {
      value: {
        owner: scopeKey(trusted), session: sessionKey(trusted),
        identity: identityKey(trusted), digest, status: "claimed", receiptId: null
      },
      enumerable: true, writable: true, configurable: true
    });
    slot.nextSequence++;
    return { kind: "claim", status: "claimed", duplicate: false };
  }
  if (!existing || existing.owner !== scopeKey(trusted) ||
      existing.session !== sessionKey(trusted) || existing.digest !== digest)
    deny("unknown-or-foreign-intent");
  // A changed document, worker or channel is not allowed to ACK a claim
  // from the previous incarnation: it must be reconciled by the native host.
  if (event.kind === "ack") {
    if (existing.identity !== identityKey(trusted)) deny("stale-claim-generation");
    if (existing.status === "acked") {
      if (existing.receiptId !== event.receiptId) deny("conflicting-receipt");
      slot.nextSequence++;
      return { kind: "ack", status: "acked", duplicate: true };
    }
    if (existing.status !== "claimed") deny("ambiguous-intent");
    existing.status = "acked";
    existing.receiptId = event.receiptId;
    slot.nextSequence++;
    return { kind: "ack", status: "acked", duplicate: false };
  }
  // Repair only returns ledger status. NEVER replays Send, tool calls,
  // input events or resurrects an outstanding claim automatically.
  slot.nextSequence++;
  return { kind: "repair", status: existing.status, receiptId: existing.receiptId,
    resendAllowed: false };
}

/**
 * verifyHostEvent MUST authenticate native 7B channel AND original event source,
 * including scope and generation, before returning the complete trusted context.
 * Never supply it from chrome.runtime.onMessage sender data or page JSON.
 * sessionStore: e.g. chrome.storage.session, no cookies / profile access.
 */
export function createCompanion({ sessionStore, verifyHostEvent }) {
  if (!sessionStore || typeof sessionStore.get !== "function" ||
      typeof sessionStore.set !== "function" ||
      typeof verifyHostEvent !== "function") deny("missing-trusted-adapter");
  let queue = Promise.resolve();
  const dispatch = (event, nativeProof) => {
    const task = queue.then(async () => {
      parseEvent(event);
      // Do not mutate or touch the ledger until native authentication passes.
      const trusted = trustedContext(await verifyHostEvent(nativeProof, event));
      const raw = await sessionStore.get(STORAGE_KEY);
      const state = validateState(structuredClone(raw[STORAGE_KEY] ?? emptyState()));
      const outcome = process(state, trusted, event);
      // ACK/claim reply is emitted ONLY after persistent session write resolves.
      await sessionStore.set({ [STORAGE_KEY]: state });
      return outcome;
    });
    queue = task.catch(() => {});
    return task;
  };
  return Object.freeze({ dispatch });
}
