// 8P host-contract differential vectors. Native callbacks here are deliberately
// host-owned fakes: no page/renderer object is ever an authority. The Windows C#
// production gate is exercised separately by its compiled self-tests.
import test from "node:test";
import assert from "node:assert/strict";
import {createProfileSessionController} from "../../extension/adapter/profile-session.mjs";
import {createNativeBrowserLifecycleController} from "../../extension/adapter/browser-lifecycle.mjs";

const ORIGIN = "https://chatgpt.com";
const proofA = Object.freeze({nativeTarget:"A"});
const proofB = Object.freeze({nativeTarget:"B"});
const consentStart = Object.freeze({nativeConsent:"start"});
const consentHandoff = Object.freeze({nativeConsent:"handoff"});
const identity = (profile, tab, session) => ({
  profileId:profile,tabId:tab,frameId:0,documentId:"doc-"+profile,
  navigationEpoch:1,sessionId:session,sessionEpoch:1,
  workerGeneration:1,hostGeneration:1,channelId:tab+100,origin:ORIGIN
});

function host() {
  const bindings = new Map([
    [proofA,{context:identity("profile-A",1,"session-A"),partition:"partition-A"}],
    [proofB,{context:identity("profile-B",2,"session-B"),partition:"partition-B"}]
  ]);
  const events = new Map();
  let starts=0,handoffs=0,permissionCalls=0;
  const activeConsents = new Set([consentStart,consentHandoff]);
  const native={
    async authorizeNativeTarget(proof) {
      if (!bindings.has(proof)) throw Error("8P:unattested-target");
      return structuredClone(bindings.get(proof).context);
    },
    async resolveNativePartition(proof) {
      if (!bindings.has(proof)) throw Error("8P:unattested-partition");
      const entry=bindings.get(proof);
      return {profileId:entry.context.profileId,partitionId:entry.partition,
        isolationGeneration:1,mode:"persistent"};
    },
    async beginNativeSignIn({nativeProof,consentProof,origin,providerId,
                             expectedContext,expectedPartition}) {
      if (!activeConsents.delete(consentProof) || consentProof!==consentStart ||
          !bindings.has(nativeProof) || providerId!=="chatgpt" || origin!==ORIGIN)
        throw Error("8P:consent-denied");
      const record=bindings.get(nativeProof);
      if (record.partition!==expectedPartition.partitionId ||
          record.context.documentId!==expectedContext.documentId)
        throw Error("8P:stale-native-context");
      // A native navigation retires the old worker and requires a fresh
      // independently attested document/channel. It never completes auth.
      record.context.documentId="doc-new-"+record.context.profileId;
      record.context.navigationEpoch++;
      record.context.workerGeneration++;
      record.context.hostGeneration++;
      record.context.channelId++;
      starts++;
      return {started:true};
    },
    async commitNativeHandoff({sourceProof,targetProof,consentProof,
                               expectedSource,expectedTarget,sourcePartition,
                               targetPartition}) {
      if (!activeConsents.delete(consentProof) || consentProof!==consentHandoff)
        throw Error("8P:consumed-consent");
      const source=bindings.get(sourceProof),target=bindings.get(targetProof);
      if (!source || !target || source===target ||
          source.partition===target.partition ||
          source.partition!==sourcePartition.partitionId ||
          target.partition!==targetPartition.partitionId ||
          source.context.sessionId!==expectedSource.sessionId ||
          target.context.sessionId!==expectedTarget.sessionId)
        throw Error("8P:scope-or-race");
      const incoming=source.context.sessionId;
      source.context.sessionId="new-session-A";
      source.context.sessionEpoch++;
      target.context.sessionId=incoming;
      target.context.sessionEpoch++;
      handoffs++;
      return {committed:true};
    },
    async verifyNativeLifecycleEvent(eventProof) {
      if (!events.has(eventProof)) throw Error("8P:foreign-event");
      return structuredClone(events.get(eventProof));
    },
    async authorizeNativePermission() {
      permissionCalls++;
      return {granted:false}; // Production 8P does NOT implement approval.
    }
  };
  const profiles=createProfileSessionController({...native,
    allowedProviders:[{id:"chatgpt",origin:ORIGIN}]});
  const lifecycle=createNativeBrowserLifecycleController({...native,
    allowedOrigins:[ORIGIN]});
  return {native,profiles,lifecycle,bindings,events,consents:activeConsents,
    stats:()=>({starts,handoffs,permissionCalls})};
}

test("8P/8B production navigation is owner-consented, reattested, not authentication",async()=>{
  const h=host();
  const before=(await h.profiles.probe(proofA)).context;
  await assert.rejects(h.profiles.startSignIn({
    nativeProof:{nativeTarget:"A"},providerId:"chatgpt",consentProof:consentStart
  }),/unattested-target/);
  await assert.rejects(h.profiles.startSignIn({
    nativeProof:proofA,providerId:"unlisted",consentProof:consentStart
  }),/consent-or-provider/);
  const res=await h.profiles.startSignIn({
    nativeProof:proofA,providerId:"chatgpt",consentProof:consentStart
  });
  const after=(await h.profiles.probe(proofA)).context;
  assert.deepEqual(res,{status:"navigation-started",profileId:"profile-A",providerId:"chatgpt"});
  assert.notEqual(after.documentId,before.documentId);
  assert.notEqual(after.channelId,before.channelId);
  assert.equal(after.sessionId,before.sessionId);
  assert.equal((await h.profiles.probe(proofB)).partition.partitionId,"partition-B");
  assert.equal(h.stats().starts,1);
  await assert.rejects(h.profiles.startSignIn({
    nativeProof:proofA,providerId:"chatgpt",consentProof:consentStart
  }),/consent-denied/);
});

test("8P/8B handoff atomically moves only logical session; replay/partition theft denied",async()=>{
  const h=host();
  await h.profiles.probe(proofA);
  await h.profiles.probe(proofB);
  const success=await h.profiles.handoff({
    sourceProof:proofA,targetProof:proofB,consentProof:consentHandoff
  });
  assert.equal(success.status,"transferred");
  assert.equal(h.bindings.get(proofB).context.sessionId,"session-A");
  assert.equal(h.bindings.get(proofA).context.sessionId,"new-session-A");
  assert.equal(h.stats().handoffs,1);
  await assert.rejects(h.profiles.handoff({
    sourceProof:proofA,targetProof:proofB,consentProof:consentHandoff
  }),/consumed-consent/);
  h.bindings.get(proofB).partition="partition-A";
  await assert.rejects(h.profiles.probe(proofB),/partition-rebound|shared-partition/);
});

test("8P/8C redirect retires stale identity and requires fresh host/worker rebind",async()=>{
  const h=host();
  await h.lifecycle.track({nativeProof:proofA});
  await h.lifecycle.track({nativeProof:proofB});
  const prior=structuredClone(h.bindings.get(proofA).context);
  const eventToken=Object.freeze({nativeEvent:"redirect"});
  h.events.set(eventToken,{kind:"navigated",sequence:1,before:prior});
  const a=h.bindings.get(proofA).context;
  a.documentId="redirected";a.navigationEpoch++;a.workerGeneration++;a.hostGeneration++;a.channelId++;
  assert.equal((await h.lifecycle.transition({
    eventProof:eventToken,nativeProof:proofA
  })).phase,"active");
  await assert.rejects(h.lifecycle.transition({
    eventProof:eventToken,nativeProof:proofA
  }),/stale-event/);
  await assert.rejects(h.lifecycle.transition({
    eventProof:{nativeEvent:"redirect"},nativeProof:proofA
  }),/foreign-event/);
  assert.equal((await h.lifecycle.track({nativeProof:proofB})).phase,"active");
  h.bindings.get(proofB).partition="partition-A";
  await assert.rejects(h.lifecycle.track({nativeProof:proofB}),/shared-partition|partition-rebound/);
});

test("8P/8C denies production permissions; stale and unsupported requests never authorize",async()=>{
  const h=host();
  await h.lifecycle.track({nativeProof:proofA});
  await assert.rejects(h.lifecycle.requestPermission({
    nativeProof:proofA,capability:"cookies",consentProof:consentStart
  }),/capability-denied/);
  await assert.rejects(h.lifecycle.requestPermission({
    nativeProof:proofA,capability:"geolocation",consentProof:consentStart
  }),/permission-not-granted/);
  assert.equal(h.stats().permissionCalls,1);
  const before=structuredClone(h.bindings.get(proofA).context);
  const crash=Object.freeze({nativeEvent:"crash"});
  h.events.set(crash,{kind:"renderer-crashed",sequence:1,before});
  assert.equal((await h.lifecycle.transition({eventProof:crash})).phase,"suspended");
  await assert.rejects(h.lifecycle.requestPermission({
    nativeProof:proofA,capability:"geolocation",consentProof:consentStart
  }),/inactive-scope/);
  assert.equal(h.stats().permissionCalls,1);
});
