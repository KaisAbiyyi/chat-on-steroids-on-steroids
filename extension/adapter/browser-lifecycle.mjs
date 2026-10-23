// 8C: host-only browser lifecycle and permission boundary. This is a
// synthetic integration contract, NOT a WebView2 native implementation.
// Every callback below must be privileged, native-authenticated and unavailable
// to web pages, extension MAIN/ISOLATED worlds and untrusted IPC.
const CONTEXT_KEYS = [
  "profileId", "tabId", "frameId", "documentId", "navigationEpoch",
  "sessionId", "sessionEpoch", "workerGeneration", "hostGeneration",
  "channelId", "origin"
];
const PARTITION_KEYS = ["profileId", "partitionId", "isolationGeneration", "mode"];
const EVENT_KEYS = ["kind", "sequence", "before"];
const EVENTS = new Set([
  "navigated", "renderer-crashed", "tab-closed", "host-restarted", "rebound"
]);
const CAPABILITIES = new Set([
  "camera", "microphone", "geolocation", "notifications",
  "clipboard-read", "clipboard-write", "downloads"
]);
const MAX_SCOPES = 256;
const MAX_PARTITIONS = 64;
const isRecord = v => v !== null && typeof v === "object" && !Array.isArray(v);
const sameKeys = (v, keys) => Object.keys(v).sort().join("|") === [...keys].sort().join("|");
const id = v => typeof v === "string" && /^[A-Za-z0-9._:-]{1,128}$/.test(v);
const uint = v => Number.isSafeInteger(v) && v >= 0;
const fail = reason => { throw Error("8C:" + reason); };
const equal = (a, b, keys = CONTEXT_KEYS) => keys.every(k => a[k] === b[k]);
const scope = c => JSON.stringify([c.profileId, c.tabId, c.frameId]);

function validOrigin(value) {
  try {
    const u = new URL(value);
    return u.protocol === "https:" && !u.username && !u.password &&
      u.origin === value && !value.includes("\\");
  } catch { return false; }
}
function checkedContext(raw, origins) {
  if (!isRecord(raw) || !sameKeys(raw, CONTEXT_KEYS) ||
      !["profileId", "documentId", "sessionId"].every(k => id(raw[k])) ||
      !["tabId", "frameId", "navigationEpoch", "sessionEpoch",
        "workerGeneration", "hostGeneration", "channelId"].every(k => uint(raw[k])) ||
      !raw.workerGeneration || !raw.hostGeneration || !raw.channelId ||
      !origins.has(raw.origin)) fail("context");
  return Object.freeze(Object.fromEntries(CONTEXT_KEYS.map(k => [k, raw[k]])));
}
function checkedPartition(raw, profile) {
  if (!isRecord(raw) || !sameKeys(raw, PARTITION_KEYS) ||
      raw.profileId !== profile || !id(raw.partitionId) ||
      !uint(raw.isolationGeneration) || !raw.isolationGeneration ||
      !["ephemeral", "persistent"].includes(raw.mode)) fail("partition");
  return Object.freeze({...raw});
}
function samePartition(a, b) {
  return PARTITION_KEYS.every(k => a[k] === b[k]);
}
function view(entry) {
  return Object.freeze({
    profileId: entry.context.profileId, tabId: entry.context.tabId,
    frameId: entry.context.frameId, phase: entry.phase,
    navigationEpoch: entry.context.navigationEpoch,
    hostGeneration: entry.context.hostGeneration,
    lastSequence: entry.sequence
  });
}

export function createNativeBrowserLifecycleController({
  authorizeNativeTarget, resolveNativePartition, verifyNativeLifecycleEvent,
  authorizeNativePermission, allowedOrigins
}) {
  if (![authorizeNativeTarget, resolveNativePartition,
    verifyNativeLifecycleEvent, authorizeNativePermission]
    .every(f => typeof f === "function")) fail("privileged-host-required");
  if (!Array.isArray(allowedOrigins) || !allowedOrigins.length ||
      allowedOrigins.length > 16 ||
      allowedOrigins.some(v => !validOrigin(v)) ||
      new Set(allowedOrigins).size !== allowedOrigins.length)
    fail("origin-policy");
  const origins = new Set(allowedOrigins);
  const entries = new Map();
  const partitionOwners = new Map();
  const profilePartitions = new Map();
  const permissionBusy = new Set();
  const transitionBusy = new Set();

  // The caller supplies an opaque NATIVE proof, never page-derived identifiers.
  // Attestation and storage partition are independently rechecked.
  async function attest(nativeProof) {
    const first = checkedContext(await authorizeNativeTarget(nativeProof), origins);
    const p = checkedPartition(await resolveNativePartition(nativeProof), first.profileId);
    const second = checkedContext(await authorizeNativeTarget(nativeProof), origins);
    if (!equal(first, second)) fail("attestation-race");
    const prior = profilePartitions.get(first.profileId);
    if (prior && !samePartition(prior, p)) fail("partition-rebound");
    if (partitionOwners.has(p.partitionId) &&
        partitionOwners.get(p.partitionId) !== first.profileId) fail("shared-partition");
    if (!prior && profilePartitions.size >= MAX_PARTITIONS) fail("partition-capacity");
    profilePartitions.set(first.profileId, p);
    partitionOwners.set(p.partitionId, first.profileId);
    return Object.freeze({context:first, partition:p});
  }

  async function track({nativeProof}) {
    const checked = await attest(nativeProof);
    const key = scope(checked.context);
    const entry = entries.get(key);
    if (entry) {
      if (entry.phase !== "active" || !equal(entry.context, checked.context) ||
          !samePartition(entry.partition, checked.partition)) fail("stale-scope");
      return view(entry);
    }
    if (entries.size >= MAX_SCOPES) fail("scope-capacity");
    const fresh = {
      context:checked.context, partition:checked.partition,
      phase:"active", sequence:0, version:0
    };
    entries.set(key, fresh);
    return view(fresh);
  }

  // These event proofs must be verified by the *native* event authority.
  // Events contain an authenticated snapshot of the previous binding. No
  // renderer-supplied event, tab ID or sequence is accepted as authority.
  async function transition({eventProof, nativeProof}) {
    const event = await verifyNativeLifecycleEvent(eventProof);
    if (!isRecord(event) || !sameKeys(event, EVENT_KEYS) ||
        !EVENTS.has(event.kind) || !uint(event.sequence) ||
        event.sequence === 0) fail("event");
    const before = checkedContext(event.before, origins);
    const key = scope(before);
    const entry = entries.get(key);
    if (!entry || !equal(entry.context, before) ||
        event.sequence <= entry.sequence) fail("stale-event");
    if (transitionBusy.has(key)) fail("transition-busy");
    transitionBusy.add(key);
    const version = entry.version;
    try {
    if (event.kind === "host-restarted") {
      if (nativeProof !== undefined) fail("unexpected-after-proof");
      // Fail closed for every tab, not just the tab that reported restart.
      for (const value of entries.values()) {
        value.phase = "suspended";
        value.version++;
      }
      entry.sequence = event.sequence;
      return view(entry);
    }
    if (event.kind === "renderer-crashed" || event.kind === "tab-closed") {
      if (entry.phase !== "active" || nativeProof !== undefined)
        fail("invalid-terminal-event");
      entry.phase = event.kind === "tab-closed" ? "closed" : "suspended";
      entry.sequence = event.sequence;
      entry.version++;
      return view(entry);
    }
    if (nativeProof === undefined) fail("after-proof-required");
    if ((event.kind === "navigated" && entry.phase !== "active") ||
        (event.kind === "rebound" && entry.phase === "active")) fail("invalid-transition");
    const checked = await attest(nativeProof);
    const after = checked.context;
    if (scope(after) !== key || !samePartition(entry.partition, checked.partition) ||
        after.origin !== before.origin || after.sessionId !== before.sessionId ||
        after.sessionEpoch !== before.sessionEpoch ||
        after.documentId === before.documentId ||
        after.navigationEpoch <= before.navigationEpoch ||
        after.hostGeneration < before.hostGeneration ||
        (event.kind === "rebound" && after.hostGeneration <= before.hostGeneration))
      fail("unsafe-rebind");
    // A further authoritative recheck closes the async event/attest gap.
    const confirmed = await attest(nativeProof);
    if (!equal(after, confirmed.context) ||
        !samePartition(checked.partition, confirmed.partition)) fail("rebind-race");
    if (entry.version !== version || !equal(entry.context, before) ||
        event.sequence <= entry.sequence || entry.phase !==
          (event.kind === "navigated" ? "active" : "suspended") &&
          !(event.kind === "rebound" && entry.phase === "closed"))
      fail("transition-race");
    // Preserve the exact entry object so in-flight permission requests can
    // detect its version increment and never return a stale grant.
    entry.context = after;
    entry.phase = "active";
    entry.sequence = event.sequence;
    entry.version++;
    return view(entry);
    } finally { transitionBusy.delete(key); }
  }

  // The native callback must atomically consume fresh user approval and
  // authorize permission for the *exact* tab/document/generation/partition.
  // It MUST NOT grant permission before those checks complete.
  async function requestPermission({nativeProof, capability, consentProof}) {
    if (!CAPABILITIES.has(capability)) fail("capability-denied");
    if (!consentProof) fail("consent-required");
    const before = await attest(nativeProof);
    const key = scope(before.context);
    const entry = entries.get(key);
    if (!entry || entry.phase !== "active" ||
        !equal(entry.context, before.context) ||
        !samePartition(entry.partition, before.partition)) fail("inactive-scope");
    if (permissionBusy.has(key)) fail("permission-busy");
    permissionBusy.add(key);
    const version = entry.version;
    try {
      const check = await attest(nativeProof);
      if (!equal(check.context, before.context) ||
          !samePartition(check.partition, before.partition) ||
          entry.version !== version || entry.phase !== "active")
        fail("permission-race");
      const decision = await authorizeNativePermission({
        nativeProof, consentProof, capability,
        expectedContext:before.context, expectedPartition:before.partition
      });
      // Only a host-attested explicit native allow can yield a grant receipt.
      // A timeout/error/undefined/renderer decision always denies.
      if (!isRecord(decision) || !sameKeys(decision, ["granted"]) ||
          decision.granted !== true) fail("permission-not-granted");
      const after = await attest(nativeProof);
      if (!equal(after.context, before.context) ||
          !samePartition(after.partition, before.partition) ||
          entry.version !== version || entry.phase !== "active")
        fail("permission-postcondition");
      return Object.freeze({status:"granted", capability,
        profileId:before.context.profileId, tabId:before.context.tabId,
        documentId:before.context.documentId});
    } finally {
      permissionBusy.delete(key);
    }
  }
  return Object.freeze({track, transition, requestPermission});
}
