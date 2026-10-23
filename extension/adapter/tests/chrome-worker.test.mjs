import test from "node:test";
import assert from "node:assert/strict";
import { createChromeWorkerAdapter } from "../chrome-worker.mjs";
import { STORAGE_KEY } from "../../companion/ledger.mjs";

const initial = () => ({
  profileId:"profile-A",tabId:7,frameId:0,documentId:"doc-A",
  navigationEpoch:1,sessionId:"session-A",sessionEpoch:1,
  workerGeneration:1,hostGeneration:1,channelId:9,
  origin:"https://synthetic.cos.test"
});
const fail = (code) => e => e.message.includes("8A:" + code);
const event = (kind,sequence,extra={}) => ({kind,protocol:14,sequence,...extra});

function harness(options={}) {
  let context = initial(), page = {tabId:7,frameId:0,documentId:"doc-A",
    tabUrl:"https://synthetic.cos.test/home",
    frameUrl:"https://synthetic.cos.test/frame",documentLifecycle:"active"};
  let authorized = true, onExecute = null, getNativeEvents;
  let reads = 0, writes = 0, calls = [], storage = {}, runtimeMessages = 0;
  const proof = Object.freeze({secret:Symbol("native-handle")});
  const chrome = {
    runtime:{
      onSuspend:{addListener(){}},
      onMessage:{addListener(){ runtimeMessages++; }},
      lastError:undefined
    },
    storage:{session:{
      async get(key){reads++;return {[key]:structuredClone(storage[key])};},
      async set(v){writes++;storage={...storage,...structuredClone(v)};}
    }},
    tabs:{get(tabId, cb) {
      const value={id:page.tabId,url:page.tabUrl};
      if(options.promises) return Promise.resolve(value);
      cb(value);
    }},
    webNavigation:{getFrame(args, cb) {
      assert.equal(args.tabId,7);
      assert.equal(args.frameId,0);
      assert.equal(typeof args.documentId,"string");
      const value=(args.frameId !== page.frameId ||
        args.documentId !== page.documentId) ? null : {
          documentId:page.documentId,url:page.frameUrl,
          documentLifecycle:page.documentLifecycle
        };
      if(options.promises) return Promise.resolve(value);
      cb(value);
    }},
    scripting:{executeScript(spec, cb) {
      calls.push(structuredClone(spec));
      if (options.apiError) {
        chrome.runtime.lastError={message:"extension permission denied"};
        cb(undefined);
        chrome.runtime.lastError=undefined;
        return;
      }
      const result=[{frameId:page.frameId,documentId:page.documentId}];
      if(onExecute) return onExecute(spec,cb,result);
      if(options.promises) return Promise.resolve(result);
      cb(result);
    }}
  };
  const configure = overrides => {context={...context,...overrides};};
  const browser = overrides => {page={...page,...overrides};};
  const host = {
    chromeApi:chrome,
    allowedOrigins:["https://synthetic.cos.test"],
    subscribeVerifiedNativeEvent(cb){getNativeEvents=cb;},
    async verifyHostEvent(nativeProof) {
      if(nativeProof!=="native") throw Error("7B:unverified");
      const {origin,...identity}=context;
      return identity;
    },
    async authorizeNativeTarget(token) {
      if(token!==proof || !authorized) throw Error("7B:unauthorized-target");
      return structuredClone(context);
    }
  };
  const adapter=createChromeWorkerAdapter(host);
  return {
    adapter, chrome, proof, configure, browser,
    revoke:()=>{authorized=false;},
    hook:cb=>{onExecute=cb;},
    calls:()=>calls, stats:()=>({reads,writes,runtimeMessages}),
    journal:()=>structuredClone(storage),
    nativeEvent:(e,p)=>getNativeEvents(e,p)
  };
}

test("fails closed without native authority, Chrome API or bounded HTTPS origins",async()=>{
  const f=harness();
  await assert.rejects(f.adapter.inject({nativeProof:{},world:"ISOLATED"}),/7B:unauthorized-target/);
  await assert.rejects(f.adapter.inject({nativeProof:f.proof,world:"EVAL"}),fail("world-denied"));
  assert.deepEqual(f.stats(),{reads:0,writes:0,runtimeMessages:0});
  const base={
    chromeApi:f.chrome,subscribeVerifiedNativeEvent(){},
    verifyHostEvent:async()=>initial(),authorizeNativeTarget:async()=>initial()
  };
  for(const origins of [[],["http://synthetic.cos.test"],
    ["https://synthetic.cos.test/route"],Array(17).fill("https://synthetic.cos.test")])
    assert.throws(()=>createChromeWorkerAdapter({...base,allowedOrigins:origins}),
      fail("invalid-origin-policy"));
  assert.throws(()=>createChromeWorkerAdapter({...base,allowedOrigins:["https://synthetic.cos.test"],
    authorizeNativeTarget:null}),fail("missing-privileged-host"));
});

test("MAIN and ISOLATED inject only fixed audited files to exact document",async()=>{
  const f=harness();
  for(const world of ["ISOLATED","MAIN"]) {
    const result=await f.adapter.inject({nativeProof:f.proof,world});
    assert.deepEqual(result,{world,profileId:"profile-A",tabId:7,
      frameId:0,documentId:"doc-A",hostGeneration:1});
  }
  assert.deepEqual(f.calls(),[
    {target:{tabId:7,documentIds:["doc-A"]},
      files:["adapter/world/isolated.js"],world:"ISOLATED"},
    {target:{tabId:7,documentIds:["doc-A"]},
      files:["adapter/world/main.js"],world:"MAIN"}
  ]);
  assert.equal(f.stats().runtimeMessages,0);
  assert.equal(f.stats().writes,0);
  assert.equal("debugger" in f.chrome,false);
});

test("promise Chrome APIs supported but fail closed on absent document ID",async()=>{
  const f=harness({promises:true});
  assert.equal((await f.adapter.inject({nativeProof:f.proof,world:"MAIN"})).world,"MAIN");
  f.hook(()=>Promise.resolve([{frameId:0}]));
  await assert.rejects(f.adapter.inject({nativeProof:f.proof,world:"ISOLATED"}),
    fail("injection-result-mismatch"));
});

test("URL, frame and tab identity MUST match native authority",async()=>{
  for(const change of [
    {tabUrl:"https://attacker.example/"},{frameUrl:"https://evil.invalid/"},
    {frameUrl:"data:text/html,hi"},{documentId:"doc-B"},{frameId:42},
    {tabId:8},{documentLifecycle:"prerender"}
  ]) {
    const f=harness();f.browser(change);
    await assert.rejects(f.adapter.inject({nativeProof:f.proof,world:"ISOLATED"}),
      fail("navigation-or-origin"));
    assert.equal(f.calls().length,0);
  }
});

test("getFrame response does not supply frameId (real Chromium schema)",async()=>{
  const f=harness();
  assert.equal((await f.adapter.inject({nativeProof:f.proof,world:"ISOLATED"})).frameId,0);
});

test("malformed or extra native fields never reach chrome.scripting",async()=>{
  for(const change of [
    {hostGeneration:0},{workerGeneration:0},{profileId:"../../x"},
    {tabId:-1},{channelId:Number.MAX_SAFE_INTEGER+1},{origin:"https://evil.invalid"},
    {extra:"page"}
  ]) {
    const f=harness();f.configure(change);
    await assert.rejects(f.adapter.inject({nativeProof:f.proof,world:"ISOLATED"}),
      /8A:native-/);
    assert.equal(f.calls().length,0);
  }
});

test("post-injection navigation race rejects success and stale native proofs",async()=>{
  const f=harness();
  f.hook((spec,cb,result)=>{f.browser({documentId:"doc-new"});cb(result);});
  await assert.rejects(f.adapter.inject({nativeProof:f.proof,world:"MAIN"}),
    fail("navigation-or-origin"));
  assert.equal(f.calls().length,1);
  f.browser({documentId:"doc-A"});
  f.hook((spec,cb,result)=>{f.revoke();cb(result);});
  await assert.rejects(f.adapter.inject({nativeProof:f.proof,world:"ISOLATED"}),
    /7B:unauthorized-target/);
});

test("fences old host generations and changed documents",async()=>{
  const f=harness();
  await f.adapter.inject({nativeProof:f.proof,world:"ISOLATED"});
  f.configure({hostGeneration:0});
  await assert.rejects(f.adapter.inject({nativeProof:f.proof,world:"MAIN"}),
    fail("native-target"));
  f.configure({hostGeneration:1,documentId:"doc-B"});
  f.browser({documentId:"doc-B"});
  await assert.rejects(f.adapter.inject({nativeProof:f.proof,world:"MAIN"}),
    fail("stale-native-generation"));
  f.configure({hostGeneration:2,navigationEpoch:2,documentId:"doc-B"});
  assert.equal((await f.adapter.inject({nativeProof:f.proof,world:"MAIN"})).documentId,"doc-B");
  f.configure({hostGeneration:1,navigationEpoch:1,documentId:"doc-A"});
  f.browser({documentId:"doc-A"});
  await assert.rejects(f.adapter.inject({nativeProof:f.proof,world:"ISOLATED"}),
    fail("stale-native-generation"));
});

test("concurrent injection for same browser ownership is rejected",async()=>{
  const f=harness();let release;
  f.hook((spec,cb,result)=>new Promise(resolve=>{
    release=()=>{cb(result);resolve(result);};
  }));
  const first=f.adapter.inject({nativeProof:f.proof,world:"ISOLATED"});
  // Flush microtasks until the synthetic asynchronous Chrome API is reached.
  for(let i=0;i<20 && !release;i++) await Promise.resolve();
  assert.equal(typeof release,"function");
  await assert.rejects(f.adapter.inject({nativeProof:f.proof,world:"MAIN"}),
    fail("injection-busy"));
  release();
  assert.equal((await first).world,"ISOLATED");
});

test("Chrome callback lastError is refusal, never a success receipt",async()=>{
  const f=harness({apiError:true});
  await assert.rejects(f.adapter.inject({nativeProof:f.proof,world:"MAIN"}),
    fail("chrome-api-failed:executeScript"));
});

test("native event subscription remains 7C authenticated and durable, no page RPC",async()=>{
  const f=harness();
  await assert.rejects(f.nativeEvent(event("attach",1),"page"),/7B:unverified/);
  assert.equal(f.stats().reads,0);
  assert.equal((await f.nativeEvent(event("attach",1),"native")).kind,"attached");
  const digest="a".repeat(64);
  assert.equal((await f.nativeEvent(event("claim",2,{intentId:"intent-A",digest}),"native")).status,"claimed");
  assert.equal((await f.nativeEvent(event("repair",3,{intentId:"intent-A",digest}),"native")).resendAllowed,false);
  assert.equal(f.journal()[STORAGE_KEY].intents["intent-A"].status,"claimed");
  assert.equal(f.stats().writes,3);
  assert.equal(f.stats().runtimeMessages,0);
});

test("API mismatch, uncertain execution and storage operations cannot impersonate authorization",async()=>{
  const f=harness();
  f.hook((spec,cb,result)=>cb([{frameId:123,documentId:"doc-A"}]));
  await assert.rejects(f.adapter.inject({nativeProof:f.proof,world:"MAIN"}),
    fail("injection-result-mismatch"));
  assert.equal(f.stats().writes,0);
  const invalid=f.nativeEvent(event("send",1),"native");
  await assert.rejects(invalid,/invalid-event/);
  assert.equal(f.stats().reads,0);
});
