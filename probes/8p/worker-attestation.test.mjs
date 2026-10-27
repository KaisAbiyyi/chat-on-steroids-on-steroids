import test from "node:test";
import assert from "node:assert/strict";
import {readFile} from "node:fs/promises";
import {createHmac,webcrypto} from "node:crypto";

const folder=new URL("./extension/",import.meta.url);
const manifest=JSON.parse(await readFile(new URL("manifest.json",folder),"utf8"));
test("8P production MV3 manifest contains no provider/page/history permissions",()=>{
  assert.deepEqual(manifest.permissions,[]);
  assert.deepEqual(manifest.host_permissions,[]);
  assert.deepEqual(Object.keys(manifest.background).sort(),["service_worker","type"]);
  assert.equal(manifest.background.service_worker,"sw.mjs");
  assert.equal(manifest.content_scripts,undefined);
  assert.equal(manifest.externally_connectable,undefined);
});
test("8P handshake-only worker fences generation and never exposes observations",async()=>{
  globalThis.crypto??=webcrypto;
  globalThis.chrome={runtime:{onInstalled:{addListener(){}}}};
  await import("./extension/sw.mjs");
  const worker=globalThis.__cos7l1Native;
  assert.deepEqual(Object.keys(worker).sort(),["attestSession","bootstrap","workerInstance"]);
  assert.equal(Object.isFrozen(worker),true);
  const key=Buffer.alloc(32,7), challenge=Buffer.alloc(32,11);
  const u64=v=>{const b=Buffer.alloc(8);b.writeBigUInt64BE(BigInt(v));return b;};
  const domain=Buffer.from("CoSoS/7B/handshake/v1");
  const context={
    profileId:"8p-chatgpt",tabId:1,frameId:0,documentId:"doc-1",
    navigationEpoch:1,sessionId:"8p-unattributed",sessionEpoch:1,
    workerGeneration:1,hostGeneration:1,channelId:1
  };
  const bootstrap=(generation,channelId,ctx)=>({
    hostGeneration:generation,channelId,
    key:key.toString("hex"),challenge:challenge.toString("hex"),
    context:ctx
  });
  const result=await worker.bootstrap(bootstrap(1,1,context));
  const expected=createHmac("sha256",key).update(Buffer.concat([
    u64(domain.length),domain,u64(1),u64(1),u64(0),challenge,u64(0)
  ])).digest("hex");
  assert.equal(result.proof,expected);
  assert.match(result.workerInstance,/^[0-9a-f-]{36}$/);
  const signed=await worker.attestSession();
  const bytes=Buffer.from(signed.payload,"hex");
  const frame=JSON.parse(bytes.toString("utf8"));
  assert.deepEqual(frame.identity,{
    profileId:"8p-chatgpt",tabId:1,frameId:0,documentId:"doc-1",
    navigationEpoch:1,sessionId:"8p-unattributed",sessionEpoch:1,
    workerGeneration:1
  });
  const observationLabel=Buffer.from("CoSoS/7B/observation/v1");
  const proof=createHmac("sha256",key).update(Buffer.concat([
    u64(observationLabel.length),observationLabel,u64(1),u64(1),
    u64(1),challenge,u64(bytes.length),bytes
  ])).digest("hex");
  assert.equal(signed.mac,proof);
  assert.equal(typeof worker.observe,"undefined");
  assert.equal(typeof worker.authorizeTool,"undefined");
  assert.equal(typeof worker.prepare,"undefined");
  assert.equal(typeof worker.accept,"undefined");
  await assert.rejects(worker.bootstrap(bootstrap(1,2,{
    ...context,channelId:2
  })),/stale-or-foreign-bootstrap/);
  await assert.rejects(worker.bootstrap(bootstrap(2,2,{
    ...context,hostGeneration:2,channelId:1
  })),/context-drift/);
  const newer=await worker.bootstrap(bootstrap(2,2,{
    ...context,hostGeneration:2,channelId:2,
    navigationEpoch:2,documentId:"doc-2",workerGeneration:2
  }));
  assert.notEqual(newer.proof,result.proof);
  assert.equal(newer.workerInstance,result.workerInstance);
  await assert.rejects(worker.bootstrap(bootstrap(1,1,context)),/stale-or-foreign-bootstrap/);
});
