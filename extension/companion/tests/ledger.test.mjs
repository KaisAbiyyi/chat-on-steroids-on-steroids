import test from "node:test";
import assert from "node:assert/strict";
import { createCompanion, CompanionError, STORAGE_KEY } from "../ledger.mjs";
import { installVerifiedCompanion } from "../worker.mjs";

const context = (overrides={}) => ({
  profileId: "profile-A", tabId: 3, frameId: 0, documentId: "doc-A",
  navigationEpoch: 1, sessionId: "local-A", sessionEpoch: 1,
  workerGeneration: 1, hostGeneration: 1, channelId: 5,
  ...overrides
});
const digest = "a".repeat(64);
const msg = (kind, sequence, extra={}) => ({ protocol: 14, kind, sequence, ...extra });
const intent = (kind, sequence, extra={}) =>
  msg(kind, sequence, {intentId:"intent-1", digest, ...extra});
const ack = (sequence, receiptId="receipt-1") =>
  intent("ack", sequence, { receiptId });
const storage = (initial={}) => {
  let data = structuredClone(initial);
  return {
    async get() { return structuredClone(data); },
    async set(patch) { data = { ...data, ...structuredClone(patch) }; },
    snapshot() { return structuredClone(data); }
  };
};
const fixture = (host=context(), shared=storage()) => ({
  host, store:shared,
  companion:createCompanion({
    sessionStore:shared,
    verifyHostEvent:async (proof) => {
      if (proof !== "privileged") throw new CompanionError("unauthenticated");
      return host;
    }
  })
});
const send = (f, event, proof="privileged") => f.companion.dispatch(event, proof);
const expectCode = async (promise, code) =>
  assert.rejects(promise, e=>e instanceof CompanionError && e.code === code);
async function attach(f) {
  assert.deepEqual(await send(f, msg("attach", 1)), {kind:"attached", replay:false});
}

test("claim / exact ACK / read-only repair persist on success", async () => {
  const f=fixture(); await attach(f);
  assert.deepEqual(await send(f, intent("claim",2)), {kind:"claim",status:"claimed",duplicate:false});
  assert.deepEqual(await send(f, ack(3)), {kind:"ack",status:"acked",duplicate:false});
  assert.deepEqual(await send(f, intent("repair",4)), {
    kind:"repair",status:"acked",receiptId:"receipt-1",resendAllowed:false
  });
  assert.deepEqual(await send(f, ack(5)), {kind:"ack",status:"acked",duplicate:true});
  assert.deepEqual(await send(f, intent("claim",6)), {kind:"claim",status:"acked",duplicate:true});
});

test("claim never sends an operation; pending repair refuses auto-retry", async () => {
  const f=fixture();await attach(f);await send(f,intent("claim",2));
  assert.deepEqual(await send(f,intent("repair",3)),
    {kind:"repair",status:"claimed",receiptId:null,resendAllowed:false});
  assert.equal(f.store.snapshot()[STORAGE_KEY].intents["intent-1"].status,"claimed");
});

test("failing authentication cannot read or write ledger", async () => {
  const f=fixture();
  await expectCode(send(f,msg("attach",1),"page-json"),"unauthenticated");
  assert.deepEqual(f.store.snapshot(), {});
  await attach(f);
});

test("unsupported versions, unknown fields and arbitrary commands rejected", async () => {
  const f=fixture();
  for(const e of [
    msg("attach",1,{ token:"injected" }),msg("attach",1,{protocol:13}),
    msg("send",1),msg("attach",0),msg("attach",1.2),msg("attach",2**54),
    msg("claim",1,{intentId:"not enough"}),intent("claim",1,{digest:"foo"}),
    intent("claim",1,{intentId:"../../escape"}),ack(1,"\n"),
  ]) await assert.rejects(send(f,e), CompanionError);
  assert.deepEqual(f.store.snapshot(),{});
  await attach(f);
});

test("nonsequential, duplicate and old worker envelopes reject without advancement", async () => {
  const f=fixture(); await attach(f);
  await expectCode(send(f,intent("claim",4)),"sequence");
  await send(f,intent("claim",2));
  await expectCode(send(f,intent("claim",2)),"sequence");
  await expectCode(send(f,ack(5)),"sequence");
  assert.equal((await send(f,intent("repair",3))).status,"claimed");
});

test("conflicting intent digest and ACK receipt cannot win",async()=>{
  const f=fixture();await attach(f);await send(f,intent("claim",2));
  await expectCode(send(f,intent("claim",3,{digest:"b".repeat(64)})),"conflicting-claim");
  await expectCode(send(f,ack(3,"invalid receipt")),"invalid-receipt");
  await send(f,ack(3));
  await expectCode(send(f,ack(4,"receipt-other")),"conflicting-receipt");
  assert.equal((await send(f,intent("repair",4))).receiptId,"receipt-1");
});

test("unknown intent ACK and repair fail closed", async()=>{
  const f=fixture();await attach(f);
  await expectCode(send(f,ack(2)),"unknown-or-foreign-intent");
  await expectCode(send(f,intent("repair",2)),"unknown-or-foreign-intent");
  assert.equal((await send(f,intent("claim",2))).duplicate,false);
});

test("worker restart restores ledger but needs new native generation; stale ACK denied",async()=>{
  const store=storage(), f=fixture(context(),store);await attach(f);
  await send(f,intent("claim",2));
  const restored=fixture(context({workerGeneration:2,hostGeneration:2,channelId:6}),store);
  await expectCode(send(restored,msg("attach",2)),"sequence");
  await attach(restored);
  assert.equal((await send(restored,intent("repair",2))).resendAllowed,false);
  await expectCode(send(restored,ack(3)),"stale-claim-generation");
  assert.equal((await send(restored,intent("repair",3))).status,"claimed");
  await expectCode(send(f,ack(3)),"stale-channel");
});

test("new navigation may not switch document at same epoch",async()=>{
  const store=storage(), f=fixture(context(),store);await attach(f);
  const other=fixture(context({documentId:"new",hostGeneration:2,workerGeneration:2,channelId:6}),store);
  await expectCode(send(other,msg("attach",1)),"document-rebind");
  await send(f,intent("claim",2));
});

test("generation cannot rollback or replace channel without advance",async()=>{
  const store=storage(),f=fixture(context(),store);await attach(f);
  await expectCode(send(f,msg("attach",1)),"sequence");
  const equal=fixture(context({channelId:6}),store);
  await expectCode(send(equal,msg("attach",1)),"generation-not-advanced");
  const stale=fixture(context({hostGeneration:0}),store);
  await expectCode(send(stale,msg("attach",1)),"host-generation");
});

test("first session attribution wins across browser scopes",async()=>{
  const store=storage(),f=fixture(context(),store);await attach(f);
  const thief=fixture(context({tabId:4, channelId:6}),store);
  await expectCode(send(thief,msg("attach",1)),"session-already-owned");
  await send(f,intent("claim",2));
  assert.equal((await send(f,intent("repair",3))).status,"claimed");
});

test("retire cannot free first-proof session for a different scope",async()=>{
  const store=storage(),f=fixture(context(),store);await attach(f);
  assert.deepEqual(await send(f,msg("retire",2)),{kind:"retired"});
  await expectCode(send(f,intent("claim",3)),"stale-channel");
  await expectCode(send(f,msg("attach",1)),"generation-not-advanced");
  const restarted=fixture(context({hostGeneration:2,workerGeneration:2,channelId:6}),store);
  assert.equal((await send(restarted,msg("attach",1))).kind,"attached");
  const thief=fixture(context({tabId:9}),store);
  await expectCode(send(thief,msg("attach",1)),"session-already-owned");
});

test("failed storage commit produces no success response; repair can recover",async()=>{
  const state=storage();let broken=true;
  const unreliable={
    get:()=>state.get(),
    set:async data=>{if(broken){broken=false;throw new Error("disk write refused")}await state.set(data)}
  };
  const f=fixture(context(),unreliable);
  await assert.rejects(send(f,msg("attach",1)),/disk write refused/);
  assert.deepEqual(state.snapshot(),{});
  assert.equal((await send(f,msg("attach",1))).kind,"attached");
});

test("simultaneous requests serialize, granting one unambiguous claim",async()=>{
  const f=fixture();await attach(f);
  const [a,b]=await Promise.allSettled([
    send(f,intent("claim",2)),send(f,intent("claim",2,{digest:"b".repeat(64)}))
  ]);
  assert.equal(a.status,"fulfilled");
  assert.equal(b.status,"rejected");
  assert.equal((await send(f,intent("repair",3))).status,"claimed");
});

test("corrupt or unknown persistent schema fails without overwriting",async()=>{
  for(const raw of [{format:3,channels:{},intents:{},sessions:{}},
                    {format:1,channels:[],intents:{},sessions:{}}]){
    const store=storage({[STORAGE_KEY]:raw}),f=fixture(context(),store);
    await expectCode(send(f,msg("attach",1)),"corrupt-ledger");
    assert.deepEqual(store.snapshot()[STORAGE_KEY],raw);
  }
});

test("no privileged adapter, no worker listener, and suspend does not flush",async()=>{
  assert.throws(()=>createCompanion({sessionStore:storage()}),CompanionError);
  let handler; let suspend;
  const chromeApi={
    storage:{session:storage()},
    runtime:{onSuspend:{addListener(fn){suspend=fn}}}
  };
  const worker=installVerifiedCompanion({
    chromeApi, verifyHostEvent:async () => context(),
    subscribeVerifiedNativeEvent(fn){handler=fn}
  });
  assert.equal(typeof handler,"function");
  assert.equal(typeof suspend,"function");
  await handler(msg("attach",1),{});
  assert.equal(typeof worker.dispatch,"function");
  assert.equal(suspend(),undefined);
});
