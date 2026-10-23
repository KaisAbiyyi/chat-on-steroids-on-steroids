// 8B: isolated browser profile admission, opt-in sign-in navigation and
// explicit *logical session attribution* handoff. Native, not page script,
// authenticates identity, proves OS/WebView2 storage separation and consumes
// one-use user approval. This code NEVER copies cookies, tokens or disk data.
const CONTEXT_KEYS = [
  "profileId", "tabId", "frameId", "documentId", "navigationEpoch",
  "sessionId", "sessionEpoch", "workerGeneration", "hostGeneration",
  "channelId", "origin"
];
const PARTITION_KEYS = ["profileId", "partitionId", "isolationGeneration", "mode"];
const UNCHANGED = CONTEXT_KEYS.filter(k => k !== "sessionId" && k !== "sessionEpoch");
const MAX_PROFILES = 64;
const id = value => typeof value === "string" && /^[A-Za-z0-9._:-]{1,128}$/.test(value);
const integer = value => Number.isSafeInteger(value) && value >= 0;
const sameKeys = (obj, keys) => Object.keys(obj).sort().join("|") === [...keys].sort().join("|");
const fail = code => { throw Error("8B:" + code); };
const record = value => value !== null && typeof value === "object" && !Array.isArray(value);

function origin(value) {
  try {
    const url = new URL(value);
    return url.protocol === "https:" && !url.username && !url.password &&
      url.origin === value && !value.includes("\\") ? url.origin : null;
  } catch { return null; }
}
function context(value, allowedOrigins) {
  if (!record(value) || !sameKeys(value, CONTEXT_KEYS) ||
    !["profileId", "documentId", "sessionId"].every(k => id(value[k])) ||
    !["tabId", "frameId", "navigationEpoch", "sessionEpoch",
      "workerGeneration", "hostGeneration", "channelId"].every(k => integer(value[k])) ||
    value.workerGeneration === 0 || value.hostGeneration === 0 ||
    value.channelId === 0 || !allowedOrigins.has(value.origin)) fail("native-context");
  return Object.freeze(Object.fromEntries(CONTEXT_KEYS.map(k => [k, value[k]])));
}
function partition(value, profileId) {
  if (!record(value) || !sameKeys(value, PARTITION_KEYS) ||
    value.profileId !== profileId || !id(value.partitionId) ||
    !integer(value.isolationGeneration) || value.isolationGeneration === 0 ||
    !["ephemeral", "persistent"].includes(value.mode))
    fail("partition-proof");
  return Object.freeze({...value});
}
function equal(a, b, keys = CONTEXT_KEYS) {
  return keys.every(k => a[k] === b[k]);
}

// All native methods below MUST be inaccessible to renderer/page scripts.
// In particular, commitNativeHandoff atomically verifies both host bindings,
// isolation partitions and consumes consentProof exactly once before changing
// session attribution, with rollback on failed precommit checks.
export function createProfileSessionController({
  authorizeNativeTarget, resolveNativePartition, beginNativeSignIn,
  commitNativeHandoff, allowedProviders
}) {
  if (![authorizeNativeTarget, resolveNativePartition, beginNativeSignIn,
    commitNativeHandoff].every(x => typeof x === "function")) fail("native-host-required");
  if (!Array.isArray(allowedProviders) || allowedProviders.length < 1 ||
      allowedProviders.length > 16) fail("provider-policy");
  const providers = new Map();
  for (const item of allowedProviders) {
    if (!record(item) || !sameKeys(item, ["id", "origin"]) ||
        !id(item.id) || origin(item.origin) !== item.origin ||
        providers.has(item.id)) fail("provider-policy");
    providers.set(item.id, item.origin);
  }
  const allowedOrigins = new Set(providers.values());
  const byProfile = new Map();
  const partitionOwners = new Map();
  const busy = new Set();

  // A checked browser profile must have its own host-proven storage partition.
  // IDs here are opaque handles (NOT paths), and never grant filesystem access.
  async function probe(nativeProof) {
    const first = context(await authorizeNativeTarget(nativeProof), allowedOrigins);
    const p = partition(await resolveNativePartition(nativeProof), first.profileId);
    const last = context(await authorizeNativeTarget(nativeProof), allowedOrigins);
    if (!equal(first, last)) fail("profile-race");
    const prior = byProfile.get(first.profileId);
    if (prior && (prior.partitionId !== p.partitionId ||
        prior.isolationGeneration > p.isolationGeneration ||
        prior.mode !== p.mode)) fail("partition-rebound");
    if (partitionOwners.has(p.partitionId) &&
        partitionOwners.get(p.partitionId) !== first.profileId) fail("shared-partition");
    if (!prior && byProfile.size >= MAX_PROFILES) fail("profile-capacity");
    byProfile.set(first.profileId, p);
    partitionOwners.set(p.partitionId, first.profileId);
    return Object.freeze({context:first, partition:p});
  }
  async function confirm(nativeProof, before) {
    const after = await probe(nativeProof);
    if (!equal(before.context, after.context) ||
        before.partition.partitionId !== after.partition.partitionId ||
        before.partition.isolationGeneration !== after.partition.isolationGeneration)
      fail("stale-profile");
    return after;
  }
  function lock(profileIds) {
    if (profileIds.some(key => busy.has(key))) fail("profile-busy");
    for (const key of profileIds) busy.add(key);
    return () => { for (const key of profileIds) busy.delete(key); };
  }

  async function startSignIn({nativeProof, providerId, consentProof}) {
    if (!providers.has(providerId) || !consentProof) fail("consent-or-provider");
    const before = await probe(nativeProof);
    if (before.context.origin !== providers.get(providerId))
      fail("sign-in-origin");
    const release = lock([before.context.profileId]);
    try {
      await confirm(nativeProof, before);
      const outcome = await beginNativeSignIn({
        nativeProof, consentProof, providerId, origin:providers.get(providerId),
        expectedContext:before.context, expectedPartition:before.partition
      });
      // "started" means only the native navigation was accepted; not that the
      // user authenticated or any provider account was inspected.
      if (!record(outcome) || !sameKeys(outcome, ["started"]) ||
          outcome.started !== true) fail("sign-in-unconfirmed");
      const after = await probe(nativeProof);
      if (after.context.profileId !== before.context.profileId ||
          after.context.origin !== providers.get(providerId) ||
          after.context.sessionId !== before.context.sessionId ||
          after.context.sessionEpoch !== before.context.sessionEpoch ||
          after.partition.partitionId !== before.partition.partitionId ||
          after.partition.isolationGeneration !== before.partition.isolationGeneration ||
          after.context.hostGeneration < before.context.hostGeneration)
        fail("sign-in-context-changed");
      return Object.freeze({status:"navigation-started",profileId:before.context.profileId,
        providerId});
    } finally { release(); }
  }

  // Transfers only the application session's attribution. Never transfers
  // provider authentication, browser cookies, localStorage or profile files.
  async function handoff({sourceProof, targetProof, consentProof}) {
    if (!sourceProof || !targetProof || !consentProof) fail("consent-required");
    const source = await probe(sourceProof);
    const target = await probe(targetProof);
    if (source.context.profileId === target.context.profileId ||
        source.partition.partitionId === target.partition.partitionId ||
        source.context.origin !== target.context.origin ||
        source.context.sessionId === target.context.sessionId)
      fail("handoff-scope");
    const release = lock([source.context.profileId, target.context.profileId]);
    try {
      await confirm(sourceProof, source);
      await confirm(targetProof, target);
      const result = await commitNativeHandoff({
        sourceProof, targetProof, consentProof,
        expectedSource:source.context, expectedTarget:target.context,
        sourcePartition:source.partition, targetPartition:target.partition
      });
      if (!record(result) || !sameKeys(result, ["committed"]) ||
          result.committed !== true) fail("handoff-unconfirmed");
      // An uncertain postcommit result is NEVER safe to retry from JS; native
      // must provide a replay-proof one-use consent receipt.
      const [sourceAfter, targetAfter] = await Promise.all([
        probe(sourceProof), probe(targetProof)
      ]);
      if (!equal(source.context, sourceAfter.context, UNCHANGED) ||
          !equal(target.context, targetAfter.context, UNCHANGED) ||
          sourceAfter.partition.partitionId !== source.partition.partitionId ||
          targetAfter.partition.partitionId !== target.partition.partitionId ||
          sourceAfter.partition.isolationGeneration !== source.partition.isolationGeneration ||
          targetAfter.partition.isolationGeneration !== target.partition.isolationGeneration ||
          sourceAfter.context.sessionId === source.context.sessionId ||
          targetAfter.context.sessionId !== source.context.sessionId ||
          sourceAfter.context.sessionEpoch <= source.context.sessionEpoch ||
          targetAfter.context.sessionEpoch <= Math.max(source.context.sessionEpoch,
            target.context.sessionEpoch))
        fail("handoff-postcondition");
      return Object.freeze({status:"transferred",fromProfile:source.context.profileId,
        toProfile:target.context.profileId});
    } finally { release(); }
  }
  return Object.freeze({probe, startSignIn, handoff});
}
