import test from "node:test";
import assert from "node:assert/strict";
import {createNativeBrowserLifecycleController} from "../browser-lifecycle.mjs";

const ORIGIN = "https://example.com";
const context = (overrides = {}) => ({
  profileId:"p1", tabId:1, frameId:0, documentId:"doc1",
  navigationEpoch:1, sessionId:"session1", sessionEpoch:1,
  workerGeneration:1, hostGeneration:1, channelId:1, origin:ORIGIN,
  ...overrides
});
const part = (profileId = "p1", partitionId = "partition1") => ({
  profileId, partitionId, isolationGeneration:1, mode:"ephemeral"
});
function harness() {
  const bindings = new Map([
    ["first", {context:context(), partition:part()}],
    ["second", {context:context({
      profileId:"p2", tabId:2, documentId:"doc2", sessionId:"session2"
    }),partition:part("p2","partition2")}]
  ]);
  const events = new Map();
  const calls = [];
  let permission = async () => ({granted:true});
  const native = {
    authorizeNativeTarget: async proof => {
      const binding = bindings.get(proof);
      if (!binding) throw Error("native:no-proof");
      return structuredClone(binding.context);
    },
    resolveNativePartition: async proof => structuredClone(bindings.get(proof).partition),
    verifyNativeLifecycleEvent: async proof => {
      if (!events.has(proof)) throw Error("native:invalid-event-proof");
      return structuredClone(events.get(proof));
    },
    authorizeNativePermission: async req => {
      calls.push(req);
      return permission(req);
    },
    allowedOrigins:[ORIGIN]
  };
  return {
    bindings, events, calls,
    setPermission: f => {permission = f;},
    controller: createNativeBrowserLifecycleController(native),
    event: (proof, kind, sequence, before) => {
      events.set(proof, {kind,sequence,before:structuredClone(before)});
      return {eventProof:proof};
    }
  };
}

test("fail-closed constructor, valid HTTPS origin and privileged callbacks", () => {
  const h = harness();
  assert.equal(Object.isFrozen(h.controller), true);
  assert.throws(() => createNativeBrowserLifecycleController({
    allowedOrigins:[ORIGIN]
  }), /privileged-host-required/);
  const native = {
    authorizeNativeTarget:async () => context(),
    resolveNativePartition:async () => part(),
    verifyNativeLifecycleEvent:async () => null,
    authorizeNativePermission:async () => ({granted:false})
  };
  for (const url of ["http://example.com","https://example.com/path",
    "https://user:secret@example.com", "https://example.com/",
    "https://example.com\\evil"]) {
    assert.throws(() => createNativeBrowserLifecycleController({
      ...native,allowedOrigins:[url]
    }), /origin-policy/);
  }
});

test("independent tabs/profiles tracked; reused partitions denied", async () => {
  const h = harness();
  assert.equal((await h.controller.track({nativeProof:"first"})).phase,"active");
  assert.equal((await h.controller.track({nativeProof:"second"})).tabId,2);
  assert.equal((await h.controller.track({nativeProof:"first"})).phase,"active");
  h.bindings.get("second").partition = part("p2","partition1");
  await assert.rejects(h.controller.track({nativeProof:"second"}), /shared-partition|partition-rebound/);
  await assert.rejects(h.controller.track({nativeProof:"missing"}), /native:no-proof/);
});

test("native proof only: malformed identity, attestation drift, and cross-origin denied", async () => {
  const h = harness();
  await h.controller.track({nativeProof:"first"});
  h.bindings.get("first").context = context({origin:"https://evil.example"});
  await assert.rejects(h.controller.track({nativeProof:"first"}), /context/);
  h.bindings.get("first").context = context({tabId:"1"});
  await assert.rejects(h.controller.track({nativeProof:"first"}), /context/);
  h.bindings.get("first").context = context({documentId:"doc2",navigationEpoch:2});
  await assert.rejects(h.controller.track({nativeProof:"first"}), /stale-scope/);
});

test("permissions deny unknown/dangerous capabilities and missing consent", async () => {
  const h = harness();
  await h.controller.track({nativeProof:"first"});
  for (const capability of ["cookies","debugger","arbitrary-script",
    "external-protocol","screencapture",undefined]) {
    await assert.rejects(h.controller.requestPermission({
      nativeProof:"first",capability,consentProof:"owner-one-use"
    }), /capability-denied/);
  }
  await assert.rejects(h.controller.requestPermission({
    nativeProof:"first",capability:"camera"
  }), /consent-required/);
  assert.equal(h.calls.length,0);
});

test("explicit native one-use permission check and strict grant receipt", async () => {
  const h = harness();
  await h.controller.track({nativeProof:"first"});
  h.setPermission(async () => ({granted:false}));
  await assert.rejects(h.controller.requestPermission({
    nativeProof:"first",capability:"camera",consentProof:"consentA"
  }), /permission-not-granted/);
  h.setPermission(async () => ({granted:true,source:"page"}));
  await assert.rejects(h.controller.requestPermission({
    nativeProof:"first",capability:"microphone",consentProof:"consentB"
  }), /permission-not-granted/);
  h.setPermission(async () => ({granted:true}));
  const result = await h.controller.requestPermission({
    nativeProof:"first",capability:"notifications",consentProof:"consentC"
  });
  assert.deepEqual(result,{
    status:"granted",capability:"notifications",profileId:"p1",tabId:1,documentId:"doc1"
  });
  assert.equal(h.calls.at(-1).expectedPartition.partitionId,"partition1");
  assert.equal(h.calls.at(-1).expectedContext.documentId,"doc1");
  assert.equal(Object.isFrozen(h.calls.at(-1).expectedContext),true);
});

test("navigation rotates binding and refuses stale document/permission", async () => {
  const h = harness();
  await h.controller.track({nativeProof:"first"});
  const original = context();
  h.bindings.get("first").context = context({
    documentId:"doc3",navigationEpoch:2,hostGeneration:1
  });
  const moved = await h.controller.transition({
    ...h.event("navigate","navigated",1,original),nativeProof:"first"
  });
  assert.equal(moved.navigationEpoch,2);
  assert.equal(moved.phase,"active");
  await assert.rejects(h.controller.transition({
    ...h.event("replay","navigated",1,original),nativeProof:"first"
  }), /stale-event/);
  await assert.rejects(h.controller.track({nativeProof:"unknown"}), /native:no-proof/);
  assert.equal((await h.controller.track({nativeProof:"first"})).navigationEpoch,2);
});

test("crash/restart suspend grants across tabs; strict rebound required", async () => {
  const h = harness();
  await h.controller.track({nativeProof:"first"});
  await h.controller.track({nativeProof:"second"});
  await h.controller.transition(h.event("restart","host-restarted",1,context()));
  for (const proof of ["first","second"]) {
    await assert.rejects(h.controller.requestPermission({
      nativeProof:proof,capability:"geolocation",consentProof:"owner"
    }), /inactive-scope/);
  }
  h.bindings.get("first").context = context({
    documentId:"doc3",navigationEpoch:2,hostGeneration:2
  });
  const rebound = await h.controller.transition({
    ...h.event("rebound","rebound",2,context()),nativeProof:"first"
  });
  assert.equal(rebound.phase,"active");
  await assert.rejects(h.controller.requestPermission({
    nativeProof:"second",capability:"notifications",consentProof:"owner"
  }), /inactive-scope/);
});

test("forged native event, stale order and unsafe rebound all fail", async () => {
  const h = harness();
  await h.controller.track({nativeProof:"first"});
  await assert.rejects(h.controller.transition({eventProof:{
    kind:"host-restarted",sequence:1,before:context()
  }}),/native:invalid-event-proof/);
  await assert.rejects(h.controller.transition(h.event("e","navigated",0,context())),/event/);
  await h.controller.transition(h.event("crash","renderer-crashed",1,context()));
  await assert.rejects(h.controller.transition({
    ...h.event("bad","rebound",2,context()), nativeProof:"first"
  }), /unsafe-rebind/);
  await assert.rejects(h.controller.transition(h.event("again","renderer-crashed",2,context())),
    /invalid-terminal-event/);
});

test("racing navigation cancels in-flight permission even if native granted", async () => {
  const h = harness();
  await h.controller.track({nativeProof:"first"});
  let resolvePermission;
  let started;
  const entered = new Promise(r => {started = r;});
  h.setPermission(async () => {
    started();
    return new Promise(r => {resolvePermission = r;});
  });
  const pending = h.controller.requestPermission({
    nativeProof:"first",capability:"camera",consentProof:"owner"
  });
  await entered;
  await assert.rejects(h.controller.requestPermission({
    nativeProof:"first",capability:"camera",consentProof:"owner2"
  }),/permission-busy/);
  h.bindings.get("first").context = context({
    documentId:"doc-next",navigationEpoch:2
  });
  await h.controller.transition({
    ...h.event("nav","navigated",1,context()),nativeProof:"first"
  });
  resolvePermission({granted:true});
  await assert.rejects(pending,/permission-postcondition/);
  h.setPermission(async () => ({granted:true}));
  const good = await h.controller.requestPermission({
    nativeProof:"first",capability:"camera",consentProof:"new-one-use"
  });
  assert.equal(good.status,"granted");
});

test("navigation must keep trusted origin/session and unique document", async () => {
  const h = harness();
  await h.controller.track({nativeProof:"first"});
  for (const changes of [
    {documentId:"doc1",navigationEpoch:2},
    {documentId:"doc-next",navigationEpoch:1},
    {documentId:"doc-next",navigationEpoch:2,sessionId:"other"},
    {documentId:"doc-next",navigationEpoch:2,hostGeneration:0},
    {documentId:"doc-next",navigationEpoch:2,workerGeneration:0}
  ]) {
    h.bindings.get("first").context = context(changes);
    await assert.rejects(h.controller.transition({
      ...h.event("change","navigated",1,context()),nativeProof:"first"
    }), /unsafe-rebind|context/);
  }
});

test("privileged host disposal releases scopes and denies future tracking", async () => {
  const h = harness();
  await h.controller.track({nativeProof:"first"});
  await h.controller.track({nativeProof:"second"});
  h.controller.dispose();
  h.controller.dispose(); // idempotent
  await assert.rejects(
    h.controller.track({nativeProof:"first"}), /controller-disposed/
  );
  await assert.rejects(
    h.controller.requestPermission({
      nativeProof:"first",capability:"camera",consentProof:"previous-consent"
    }), /controller-disposed/
  );
  assert.equal(h.calls.length,0);
});

test("disposing while native permission is pending never returns a stale grant", async () => {
  const h = harness();
  await h.controller.track({nativeProof:"first"});
  let entered;
  const started = new Promise(resolve => { entered = resolve; });
  let grant;
  h.setPermission(() => {
    entered();
    return new Promise(resolve => { grant = resolve; });
  });
  const pending = h.controller.requestPermission({
    nativeProof:"first",capability:"camera",consentProof:"one-use"
  });
  await started;
  h.controller.dispose();
  grant({granted:true});
  await assert.rejects(pending, /controller-disposed|permission-postcondition/);
});
