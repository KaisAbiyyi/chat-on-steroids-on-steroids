import test from "node:test";
import assert from "node:assert/strict";
import {createProfileSessionController} from "../profile-session.mjs";

const hostOrigin = "https://synthetic.cos.test";
const secondOrigin = "https://other.cos.test";
const ctx = (profileId, tabId, sessionId, origin=hostOrigin) => ({
  profileId, tabId, frameId:0, documentId:"doc-"+profileId,
  navigationEpoch:1, sessionId, sessionEpoch:1, workerGeneration:1,
  hostGeneration:1, channelId:tabId+100, origin
});
const fail = code => error => error.message.includes("8B:"+code);

function harness(opts={}) {
  const proofs=[Object.freeze({native:"A"}),Object.freeze({native:"B"}),
    Object.freeze({native:"C"})];
  const approvals=[Object.freeze({consent:"sign-in"}), Object.freeze({consent:"handoff"})];
  const profiles=new Map([
    [proofs[0],{identity:ctx("profile-A",1,"session-A"), partition:"partition-A"}],
    [proofs[1],{identity:ctx("profile-B",2,"session-B"), partition:"partition-B"}],
    [proofs[2],{identity:ctx("profile-C",3,"session-C",secondOrigin),partition:"partition-C"}]
  ]);
  let transfers=0, starts=0, cookieReads=0, cookieWrites=0;
  let approved=true, onTransfer=opts.onTransfer;
  const native={
    async authorizeNativeTarget(proof) {
      if (!profiles.has(proof)) throw Error("native proof refused");
      return structuredClone(profiles.get(proof).identity);
    },
    async resolveNativePartition(proof) {
      const p=profiles.get(proof);
      if (!p) throw Error("unknown native partition");
      return {profileId:p.identity.profileId,partitionId:p.partition,
        isolationGeneration:1,mode:"persistent"};
    },
    async beginNativeSignIn({nativeProof, consentProof, providerId,origin,expectedContext,expectedPartition}) {
      if (!approved || consentProof!==approvals[0] || providerId!=="synthetic" ||
          origin!==hostOrigin || expectedPartition.profileId!==expectedContext.profileId)
        throw Error("native sign-in consent denied");
      starts++;
      const p=profiles.get(nativeProof);
      p.identity.navigationEpoch++;
      p.identity.hostGeneration++;
      return {started:true};
    },
    async commitNativeHandoff(args) {
      if (!approved || args.consentProof!==approvals[1] || transfers ||
          args.expectedSource.profileId===args.expectedTarget.profileId ||
          args.sourcePartition.partitionId===args.targetPartition.partitionId)
        throw Error("native consent denied or consumed");
      if(onTransfer) await onTransfer(args);
      const from=profiles.get(args.sourceProof),to=profiles.get(args.targetProof);
      if (from.identity.sessionId!==args.expectedSource.sessionId ||
          to.identity.sessionId!==args.expectedTarget.sessionId)
        throw Error("native host stale before commit");
      // Atomic synthetic host transaction: only session attribution moves.
      from.identity.sessionId="fresh-empty-session";
      from.identity.sessionEpoch=2;
      to.identity.sessionId=args.expectedSource.sessionId;
      to.identity.sessionEpoch=2;
      transfers++;
      return {committed:true};
    }
  };
  const controller=createProfileSessionController({...native,
    allowedProviders:[{id:"synthetic",origin:hostOrigin},{id:"other",origin:secondOrigin}]});
  return {
    controller,proofs,approvals,profiles,
    native, setApproved:v=>{approved=v;},
    stats:()=>({starts,transfers,cookieReads,cookieWrites})
  };
}

test("only native-authorized profiles are admitted and identifiers are non-filesystem handles",async()=>{
  const h=harness();
  const p=await h.controller.probe(h.proofs[0]);
  assert.equal(p.partition.partitionId,"partition-A");
  assert.equal(p.context.profileId,"profile-A");
  assert.equal(p.partition.mode,"persistent");
  assert.equal(Object.isFrozen(p.context),true);
  await assert.rejects(h.controller.probe({native:"A"}),/native proof refused/);
  assert.deepEqual(h.stats(),{starts:0,transfers:0,cookieReads:0,cookieWrites:0});
});

test("cross-profile reuse of the same native storage partition refuses",async()=>{
  const h=harness();
  await h.controller.probe(h.proofs[0]);
  h.profiles.get(h.proofs[1]).partition="partition-A";
  await assert.rejects(h.controller.probe(h.proofs[1]),fail("shared-partition"));
});

test("profile rebound and rollback to a different storage partition are refused",async()=>{
  const h=harness();
  await h.controller.probe(h.proofs[0]);
  h.profiles.get(h.proofs[0]).partition="partition-moved";
  await assert.rejects(h.controller.probe(h.proofs[0]),fail("partition-rebound"));
});

test("native context shape, origin and partition attestation fail closed",async()=>{
  for(const mutation of [
    p=>{p.identity.extra="page-supplied";},
    p=>{p.identity.origin="https://malicious.example";},
    p=>{p.identity.sessionEpoch=Number.MAX_SAFE_INTEGER+1;},
    p=>{p.identity.profileId="../bad";}
  ]) {
    const h=harness(); mutation(h.profiles.get(h.proofs[0]));
    await assert.rejects(h.controller.probe(h.proofs[0]),fail("native-context"));
  }
  const h=harness();
  h.profiles.get(h.proofs[0]).partition="../../profile";
  await assert.rejects(h.controller.probe(h.proofs[0]),fail("partition-proof"));
});

test("sign-in is explicit consent-gated navigation, not fabricated authentication",async()=>{
  const h=harness();
  await assert.rejects(h.controller.startSignIn({
    nativeProof:h.proofs[0],providerId:"synthetic"}),fail("consent-or-provider"));
  await assert.rejects(h.controller.startSignIn({
    nativeProof:h.proofs[0],providerId:"unlisted",consentProof:h.approvals[0]}),
    fail("consent-or-provider"));
  h.setApproved(false);
  await assert.rejects(h.controller.startSignIn({
    nativeProof:h.proofs[0],providerId:"synthetic",consentProof:h.approvals[0]}),
    /consent denied/);
  h.setApproved(true);
  assert.deepEqual(await h.controller.startSignIn({
    nativeProof:h.proofs[0],providerId:"synthetic",consentProof:h.approvals[0]}),
    {status:"navigation-started",profileId:"profile-A",providerId:"synthetic"});
  assert.deepEqual(h.stats(),{starts:1,transfers:0,cookieReads:0,cookieWrites:0});
});

test("sign-in cannot jump providers or claim a cross-origin context",async()=>{
  const h=harness();
  await assert.rejects(h.controller.startSignIn({
    nativeProof:h.proofs[2],providerId:"synthetic",consentProof:h.approvals[0]}),
    fail("sign-in-origin"));
});

test("sign-in cannot report navigation success if provider or session silently changes",async()=>{
  const h=harness();
  const rogue=createProfileSessionController({...h.native,
    beginNativeSignIn:async args=>{
      h.profiles.get(h.proofs[0]).identity.origin=secondOrigin;
      return {started:true};
    },allowedProviders:[{id:"synthetic",origin:hostOrigin},{id:"other",origin:secondOrigin}]});
  await assert.rejects(rogue.startSignIn({nativeProof:h.proofs[0],
    providerId:"synthetic",consentProof:h.approvals[0]}),fail("sign-in-context-changed"));
  const g=harness();
  const swapped=createProfileSessionController({...g.native,
    beginNativeSignIn:async()=>{
      g.profiles.get(g.proofs[0]).identity.sessionId="unrelated-session";
      return {started:true};
    },allowedProviders:[{id:"synthetic",origin:hostOrigin}]});
  await assert.rejects(swapped.startSignIn({nativeProof:g.proofs[0],
    providerId:"synthetic",consentProof:g.approvals[0]}),fail("sign-in-context-changed"));
});

test("atomic explicit logical handoff moves attribution, never browser credentials",async()=>{
  const h=harness();
  const result=await h.controller.handoff({
    sourceProof:h.proofs[0],targetProof:h.proofs[1],consentProof:h.approvals[1]});
  assert.deepEqual(result,{status:"transferred",fromProfile:"profile-A",toProfile:"profile-B"});
  assert.equal(h.profiles.get(h.proofs[0]).identity.sessionId,"fresh-empty-session");
  assert.equal(h.profiles.get(h.proofs[1]).identity.sessionId,"session-A");
  assert.deepEqual(h.stats(),{starts:0,transfers:1,cookieReads:0,cookieWrites:0});
  await assert.rejects(h.controller.handoff({
    sourceProof:h.proofs[0],targetProof:h.proofs[1],consentProof:h.approvals[1]}),
    /consumed/);
});

test("handoff rejects same-profile, cross-provider, missing consent and same session",async()=>{
  const h=harness();
  await assert.rejects(h.controller.handoff({
    sourceProof:h.proofs[0],targetProof:h.proofs[0],consentProof:h.approvals[1]}),
    fail("handoff-scope"));
  await assert.rejects(h.controller.handoff({
    sourceProof:h.proofs[0],targetProof:h.proofs[2],consentProof:h.approvals[1]}),
    fail("handoff-scope"));
  await assert.rejects(h.controller.handoff({
    sourceProof:h.proofs[0],targetProof:h.proofs[1]}),fail("consent-required"));
  h.profiles.get(h.proofs[1]).identity.sessionId="session-A";
  await assert.rejects(h.controller.handoff({
    sourceProof:h.proofs[0],targetProof:h.proofs[1],consentProof:h.approvals[1]}),
    fail("handoff-scope"));
  assert.equal(h.stats().transfers,0);
});

test("handoff serializes both profiles while native commit is pending",async()=>{
  let release;const h=harness({onTransfer:()=>new Promise(resolve=>{release=resolve;})});
  const first=h.controller.handoff({sourceProof:h.proofs[0],
    targetProof:h.proofs[1],consentProof:h.approvals[1]});
  for(let i=0;i<60&&!release;i++) await Promise.resolve();
  assert.equal(typeof release,"function");
  await assert.rejects(h.controller.startSignIn({nativeProof:h.proofs[0],
    providerId:"synthetic",consentProof:h.approvals[0]}),fail("profile-busy"));
  release();
  assert.equal((await first).status,"transferred");
});

test("native postcommit context cannot falsely confirm an uncertain transfer",async()=>{
  const h=harness();
  const native=h.native;
  const failing=createProfileSessionController({...native,
    commitNativeHandoff:async args=>{
      const result=await native.commitNativeHandoff(args);
      h.profiles.get(h.proofs[1]).identity.documentId="other-document";
      return result;
    },allowedProviders:[{id:"synthetic",origin:hostOrigin}]});
  await assert.rejects(failing.handoff({
    sourceProof:h.proofs[0],targetProof:h.proofs[1],consentProof:h.approvals[1]}),
    fail("handoff-postcondition"));
  assert.equal(h.stats().transfers,1); // committed: never retry blindly
});

test("native-reported unconfirmed outcome is never transformed into success",async()=>{
  const h=harness();
  const failing=createProfileSessionController({...h.native,
    commitNativeHandoff:async()=>({committed:false}),
    beginNativeSignIn:async()=>({started:false}),
    allowedProviders:[{id:"synthetic",origin:hostOrigin}]});
  await assert.rejects(failing.handoff({
    sourceProof:h.proofs[0],targetProof:h.proofs[1],consentProof:h.approvals[1]}),
    fail("handoff-unconfirmed"));
  await assert.rejects(failing.startSignIn({
    nativeProof:h.proofs[0],providerId:"synthetic",consentProof:h.approvals[0]}),
    fail("sign-in-unconfirmed"));
});

test("rejects origin and provider policy ambiguity before touching native host",()=>{
  const h=harness();
  const native=h.native;
  for (const policy of [
    [],[{id:"x",origin:"http://synthetic.cos.test"}],
    [{id:"x",origin:hostOrigin},{id:"x",origin:secondOrigin}],
    [{id:"x",origin:hostOrigin+"/path"}],
    [{id:"x",origin:hostOrigin,extra:true}]
  ]) assert.throws(()=>createProfileSessionController({
    ...native,allowedProviders:policy}),fail("provider-policy"));
});
