// Hosted Windows, SOURCE-EXECUTED 8P C# -> real Rust child-process IPC ->
// source-owned MV3 worker. Synthetic identities only; no WebView2/owner login.
// This is stronger than a mocked Rust transport, but not a live browser test.
import test from 'node:test';
import assert from 'node:assert/strict';
import {spawn, execFileSync} from 'node:child_process';
import {readFileSync} from 'node:fs';
import {createInterface} from 'node:readline';
import {webcrypto} from 'node:crypto';
import vm from 'node:vm';
import {resolve, dirname} from 'node:path';
import {fileURLToPath} from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const host = resolve(root, 'apps/windows-host/bin/Release/net8.0-windows/WindowsHost.dll');
const bridge = resolve(root, 'target/release/examples/7l2_bridge.exe');
const sw = readFileSync(resolve(root, 'probes/8p/extension/sw.mjs'), 'utf8');
const vectors = JSON.parse(execFileSync('dotnet', [host, '--8p-native-differential-v1'], {
  cwd: root, encoding: 'utf8', timeout: 30000
}));
assert.equal(vectors.version, 'CoSoS/8P/native-two-phase/v1');
const positive = vectors.cases.find(c => c.id === 'native-two-phase-fresh');
assert.equal(positive?.nativeAccepted, true, 'compiled production C# predicate');

function hostEvidence(s, tabId) {
  return {
    identity: {profileId:s.Profile,tabId,frameId:0,documentId:s.DocumentId,
      navigationEpoch:s.NavigationEpoch,sessionId:s.SessionId,
      sessionEpoch:s.SessionEpoch,workerGeneration:s.WorkerGeneration},
    origin:s.Origin,hostGeneration:s.HostGeneration
  };
}
function newWorker() {
  const realm = vm.createContext({
    crypto:webcrypto,TextEncoder,Uint8Array,DataView,BigInt,
    chrome:{runtime:{onInstalled:{addListener(){}}}}
  });
  vm.runInContext(sw, realm, {filename:'source-owned-8p-sw.mjs'});
  return {
    bootstrap: spec => vm.runInContext(
      'globalThis.__cos7l1Native.bootstrap(' + JSON.stringify(spec) + ')', realm),
    attest: () => vm.runInContext('globalThis.__cos7l1Native.attestSession()', realm)
  };
}
function rustProcess() {
  const child = spawn(bridge, [], {cwd:root,stdio:['pipe','pipe','pipe']});
  const lines = createInterface({input:child.stdout});
  const waiters = [];
  const errors = [];
  child.stderr.on('data', chunk => errors.push(String(chunk).slice(0,1000)));
  lines.on('line', line => {
    const next = waiters.shift();
    if (next) next(JSON.parse(line));
  });
  async function ask(command) {
    if (child.exitCode !== null || child.killed || child.stdin.destroyed)
      throw Error('8P Rust IPC authority unavailable');
    return await new Promise((resolveAnswer,reject) => {
      const timer = setTimeout(() => reject(Error('8P Rust IPC timeout')),15000);
      waiters.push(value => {clearTimeout(timer);resolveAnswer(value);});
      child.stdin.write(JSON.stringify(command)+'\n', e => {
        if (e) {clearTimeout(timer);reject(e);}
      });
    });
  }
  function stop() {
    if (child.exitCode === null && !child.killed) child.kill();
  }
  return {ask,stop,child,errors};
}
async function prove(rust, snap, tab, worker) {
  const authority = hostEvidence(snap,tab);
  const register = await rust.ask({op:'register',host:authority});
  assert.equal(register.ok,true,'real Rust register rejected: '+register.error);
  const {channelId,hostGeneration,key,challenge} = register.value;
  const spec = {
    channelId,hostGeneration,key,challenge,
    context:{...authority.identity,hostGeneration,channelId}
  };
  const {proof,workerInstance} = await worker.bootstrap(spec);
  assert.ok(workerInstance,'real source-owned MV3 did not instantiate');
  const handshake = await rust.ask({
    op:'handshake',host:authority,channelId,generation:hostGeneration,proof
  });
  assert.deepEqual(handshake,{ok:true,value:{authenticated:true}});
  const signed = await worker.attest();
  const observed = await rust.ask({
    op:'observe',host:authority,channelId,generation:hostGeneration,
    sequence:1,payload:signed.payload,mac:signed.mac
  });
  assert.equal(observed.ok,true,'real Rust refused MV3 first proof: '+observed.error);
  assert.equal(observed.value.observation,'FirstProof');
  return {authority,channelId,hostGeneration,signed};
}
async function seed(rust) {
  const a = await prove(rust,positive.beforeA,1,newWorker());
  const b = await prove(rust,positive.beforeB,2,newWorker());
  const wire = {
    op:'handoff',source:a.authority,target:b.authority,
    contractVersion:vectors.version,newSourceSessionId:positive.sourceSessionTicket,
    sourceNextSessionEpoch:positive.afterA.SessionEpoch,
    targetNextSessionEpoch:positive.afterB.SessionEpoch
  };
  const pre = await rust.ask(wire);
  assert.equal(pre.ok,false,'handoff before both native retirements');
  assert.equal((await rust.ask({op:'retire',channelId:a.channelId})).value.retired,true);
  assert.equal((await rust.ask(wire)).ok,false,'remaining sibling lease admitted');
  assert.equal((await rust.ask({op:'retire',channelId:b.channelId})).value.retired,true);
  assert.equal((await rust.ask({...wire,contractVersion:'legacy-v0'})).ok,false,
    'downgraded IPC contract');
  assert.equal((await rust.ask({...wire,contractVersion:undefined})).ok,false,
    'unversioned IPC contract');
  assert.equal((await rust.ask({...wire,target:{...b.authority,identity:{
    ...b.authority.identity,profileId:'legacy-B'}}})).ok,false,
    'mixed native 8P/legacy profile');
  const commit = await rust.ask(wire);
  assert.deepEqual(commit,{ok:true,value:{committed:true}});
  assert.equal((await rust.ask(wire)).ok,false,'replayed Rust commit');
  assert.equal((await rust.ask({
    op:'handshake',host:a.authority,channelId:a.channelId,
    generation:a.hostGeneration,proof:'00'.repeat(32)
  })).ok,false,'retired channel revived');
  return {a,b,wire};
}

test('compiled C# native-v1 + real Rust IPC + source MV3: commit/rebind/2 first proofs', {
  timeout:90000
}, async () => {
  const rust = rustProcess();
  try {
    await seed(rust);
    const a = await prove(rust,positive.afterA,1,newWorker());
    const b = await prove(rust,positive.afterB,2,newWorker());
    assert.notEqual(a.channelId,b.channelId);
    assert.notEqual(a.authority.identity.profileId,b.authority.identity.profileId);
    assert.equal(a.authority.identity.sessionId,positive.sourceSessionTicket);
    assert.equal(b.authority.identity.sessionId,positive.beforeA.SessionId);
    assert.equal((await rust.ask({op:'retire',channelId:a.channelId})).value.retired,true);
    assert.equal((await rust.ask({op:'retire',channelId:b.channelId})).value.retired,true);
  } finally {rust.stop();}
});

test('failure injection AFTER Rust commit but BEFORE target fresh MV3 proof: no reuse', {
  timeout:90000
}, async () => {
  const rust = rustProcess();
  try {
    await seed(rust);
    const nextA = await prove(rust,positive.afterA,1,newWorker());
    // Inject failed target rebind after the irreversible Rust commit. Abort the
    // private child authority (native host must quarantine, never rollback).
    rust.stop();
    await new Promise(resolve => rust.child.once('exit',resolve));
    await assert.rejects(rust.ask({
      op:'handshake',host:nextA.authority,channelId:nextA.channelId,
      generation:nextA.hostGeneration,proof:'00'.repeat(32)
    }),/authority unavailable/);
    // The abandoned worker/key has no authenticated IPC path after failure.
    assert.equal(rust.child.exitCode === null,false);
  } finally {rust.stop();}
});
