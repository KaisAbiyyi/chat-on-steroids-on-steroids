// 7E shared source-wire corpus and deterministic negative/reconnect qualifications.
import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createCompanion, CompanionError, STORAGE_KEY } from "../ledger.mjs";

const { schema, cases } = JSON.parse(readFileSync(
  new URL("../../../fixtures/phase7/7e-wire-corpus.json", import.meta.url), "utf8"
));
const digest = "a".repeat(64);
const event = (kind, sequence, extras={}) => ({ protocol: 14, kind, sequence, ...extras });
const claim = (kind, sequence, extra={}) => event(kind, sequence, {intentId:"intent-1", digest, ...extra});
const context = (overrides={}) => ({
  profileId:"profile-A", tabId:3, frameId:0, documentId:"doc-A",
  navigationEpoch:1, sessionId:"local-A", sessionEpoch:1, workerGeneration:1,
  hostGeneration:1, channelId:5, ...overrides
});
function sessionStore() {
  let state = {};
  return {
    async get(){return structuredClone(state);},
    async set(patch){state = {...state, ...structuredClone(patch)};},
    snapshot(){return structuredClone(state);}
  };
}
function fixture(store=sessionStore(), host=context()) {
  const nativeProof = Object.freeze({source:"synthetic-privileged"});
  const companion = createCompanion({
    sessionStore:store,
    verifyHostEvent:async proof => {
      if (proof !== nativeProof) throw new CompanionError("unauthenticated");
      return host;
    }
  });
  return { store, nativeProof, host,
    dispatch:e=>companion.dispatch(e,nativeProof),
    unauthenticated:e=>companion.dispatch(e, {source:"page"})
  };
}
async function prepare(f, steps) {
  for (const step of steps) {
    if (step === "attach") await f.dispatch(event("attach",1));
    else if (step === "claim") await f.dispatch(claim("claim",2));
    else if (step === "repair") await f.dispatch(claim("repair",3));
    else throw new Error(`unknown preparation: ${step}`);
  }
}
test("7E source-wire corpus: same allow/deny as Rust signed receipt gate", async()=>{
  assert.equal(schema,1);
  assert.ok(cases.length>=30);
  for (const entry of cases) {
    const f=fixture();
    await prepare(f,entry.prep);
    const before=f.store.snapshot();
    let accepted=false;
    try { await f.dispatch(entry.event); accepted=true; }
    catch(e){ assert.ok(e instanceof CompanionError, `${entry.id}: unexpected error ${e}`); }
    assert.equal(accepted,entry.accepted,`native/JS disagreement for ${entry.id}`);
    if(!accepted) assert.deepEqual(f.store.snapshot(),before,`${entry.id} mutated ledger`);
  }
});

test("7E bounded event fuzzer denies malformed input without ledger mutation",async()=>{
  for(let i=0;i<224;i++){
    const f=fixture();await f.dispatch(event("attach",1));
    const baseline=f.store.snapshot();
    const hostile=claim("claim",2);
    switch(i%7){
      case 0: hostile.protocol=13;break;
      case 1: hostile.sequence=0;break;
      case 2: hostile.sequence=3;break;
      case 3: hostile.digest="b".repeat(63);break;
      case 4: hostile.intentId="bad/id";break;
      case 5: hostile.receiptId=null;break;
      default: hostile.op="send";break;
    }
    await assert.rejects(f.dispatch(hostile), CompanionError,`mutation ${i}`);
    assert.deepEqual(f.store.snapshot(),baseline,`mutation ${i} persisted`);
    assert.equal((await f.dispatch(claim("claim",2))).status,"claimed");
  }
});

test("7E ambiguous storage error cannot acknowledge success or trigger resend",async()=>{
  const backing=sessionStore();
  let errorAfterCommit=true;
  const unreliable={
    get:()=>backing.get(),
    set:async patch=>{
      await backing.set(patch);
      if(errorAfterCommit){errorAfterCommit=false;throw new Error("uncertain commit");}
    }
  };
  const f=fixture(unreliable);
  await assert.rejects(f.dispatch(event("attach",1)),/uncertain commit/);
  // The disk DID commit; replay of attach at sequence 1 is forbidden.
  await assert.rejects(f.dispatch(event("attach",1)),e=>e.code==="sequence");
  // Client cannot conclude success from the failed write; repair requires a
  // recovered authenticated session and must remain read-only/no auto Send.
  assert.equal((await f.dispatch(event("attach",2))).kind,"attached");
  await f.dispatch(claim("claim",3));
  const outcome=await f.dispatch(claim("repair",4));
  assert.equal(outcome.resendAllowed,false);
  assert.equal(outcome.status,"claimed");
});

test("7E MV3 restart denies old ACK, retains ambiguous claim and first-proof scope",async()=>{
  const shared=sessionStore();
  const before=fixture(shared);
  await before.dispatch(event("attach",1));
  await before.dispatch(claim("claim",2));
  const restarted=fixture(shared,context({
    workerGeneration:2,hostGeneration:2,channelId:6,documentId:"doc-B",navigationEpoch:2
  }));
  await restarted.dispatch(event("attach",1));
  await assert.rejects(before.dispatch(claim("ack",3,{receiptId:"receipt-A"})),
    e=>e.code==="stale-channel");
  const pending=await restarted.dispatch(claim("repair",2));
  assert.equal(pending.status,"claimed");
  assert.equal(pending.resendAllowed,false);
  await assert.rejects(restarted.dispatch(claim("ack",3,{receiptId:"receipt-A"})),
    e=>e.code==="stale-claim-generation");
  assert.equal((await restarted.dispatch(claim("repair",3))).status,"claimed");
  const foreign=fixture(shared,context({tabId:99,hostGeneration:3,channelId:7}));
  await assert.rejects(foreign.dispatch(event("attach",1)),
    e=>e.code==="session-already-owned");
  assert.equal(shared.snapshot()[STORAGE_KEY].intents["intent-1"].status,"claimed");
});
