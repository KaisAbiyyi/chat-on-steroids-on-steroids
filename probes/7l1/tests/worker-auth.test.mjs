import test from "node:test";
import assert from "node:assert/strict";
import { createHmac } from "node:crypto";
import { cpSync, mkdirSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const nativeDir = resolve(import.meta.dirname, "../extension");
const companionDir = resolve(import.meta.dirname, "../../../extension/companion");
const testRoot = process.platform === "win32"
  ? process.env.COS_TEST_TMP
  : process.env.COS_TEST_TMP || tmpdir();
if (!testRoot) throw Error("Set COS_TEST_TMP to an owned D: folder on Windows");
mkdirSync(testRoot, {recursive:true});
const bundle = mkdtempSync(join(testRoot, "7l1-worker-"));
for (const file of ["sw.mjs", "manifest.json"]) cpSync(join(nativeDir, file), join(bundle, file));
for (const file of ["worker.mjs", "ledger.mjs"]) cpSync(join(companionDir, file), join(bundle, file));

const records = new Map();
let writes = 0;
globalThis.chrome = {
  storage: {session: {
    async get(key) { return {[key]:records.get(key)}; },
    async set(values) { for (const [k,v] of Object.entries(values)) records.set(k, structuredClone(v)); writes++; }
  }},
  runtime: {onSuspend:{addListener(){}}},
  tabs: {onUpdated:{addListener(){}}}
};
await import(pathToFileURL(join(bundle, "sw.mjs")).href);
const adapter = globalThis.__cos7l1Native;
const key = Buffer.alloc(32, 42);
const challenge = Buffer.alloc(32, 31);
const context = {
  profileId:"profile-A", tabId:1, frameId:0, documentId:"document-A",
  navigationEpoch:1, sessionId:"session-A", sessionEpoch:1, workerGeneration:1,
  hostGeneration:1, channelId:17
};
const bytes8 = n => { const b = Buffer.alloc(8); b.writeBigUInt64BE(BigInt(n)); return b; };
function proof(domain, seq, payload) {
  const tag = Buffer.from(domain);
  const body = Buffer.from(payload);
  return createHmac("sha256", key).update(Buffer.concat([
    bytes8(tag.length), tag, bytes8(17), bytes8(1),
    bytes8(seq), challenge, bytes8(body.length), body
  ])).digest("hex");
}
function decodePayload(hex) { return Buffer.from(hex,"hex").toString("utf8"); }
function event(kind, sequence, rest={}) { return {kind,protocol:14,sequence,...rest}; }

test("one-use worker bootstrap and complete Rust-compatible HMAC envelope",async()=>{
  const response = await adapter.bootstrap({
    hostGeneration:1,channelId:17, key:key.toString("hex"),
    challenge:challenge.toString("hex"),context
  });
  assert.equal(response.proof,proof("CoSoS/7B/handshake/v1",0,""));
  await assert.rejects(() => adapter.bootstrap({
    hostGeneration:1,channelId:17, key:key.toString("hex"),
    challenge:challenge.toString("hex"),context
  }),/stale-bootstrap/);
  const observed = await adapter.observe(1);
  assert.equal(observed.mac,proof("CoSoS/7B/observation/v1",1,decodePayload(observed.payload)));
  assert.equal(adapter.confirmObservation(1),true);
});

async function accepted(kind, sequence, rest={}) {
  const e=event(kind,sequence,rest);
  const request=await adapter.prepare(e,sequence+1);
  const payload=decodePayload(request.payload);
  assert.deepEqual(JSON.parse(payload).event,e);
  assert.equal(request.mac,proof("CoSoS/7L1/companion-event/v1",sequence+1,payload));
  const receipt=proof("CoSoS/7L1/companion-receipt/v1",sequence+1,payload);
  return adapter.accept(e,sequence+1,receipt);
}

test("only exact native acceptance receipts persist claim and refuse resend",async()=>{
  assert.equal((await accepted("attach",1)).kind,"attached");
  const digest="a".repeat(64);
  const claim=await accepted("claim",2,{intentId:"intent-A",digest});
  assert.deepEqual(claim,{kind:"claim",status:"claimed",duplicate:false});
  const repaired=await accepted("repair",3,{intentId:"intent-A",digest});
  assert.equal(repaired.status,"claimed");
  assert.equal(repaired.resendAllowed,false);
  assert.equal(writes,3);
});

test("event replacement and forged receipt cannot write storage.session",async()=>{
  const e=event("ack",4,{intentId:"intent-A",digest:"a".repeat(64),receiptId:"receipt-A"});
  const signed=await adapter.prepare(e,5);
  const payload=decodePayload(signed.payload);
  const before=writes;
  await assert.rejects(() => adapter.accept({...e,digest:"b".repeat(64)},5,
    proof("CoSoS/7L1/companion-receipt/v1",5,payload)),/unverified-native-event/);
  await assert.rejects(() => adapter.accept(e,5,"0".repeat(64)),/invalid-native-receipt/);
  assert.equal(writes,before);
  assert.ok(records.size > 0);
});

test.after(()=>rmSync(bundle,{recursive:true,force:true}));
