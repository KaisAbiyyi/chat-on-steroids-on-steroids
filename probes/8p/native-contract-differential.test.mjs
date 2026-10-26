// Cross-language contract differential: this test launches the compiled C#
// PRODUCTION host method (no WebView2 browser) for every synthetic vector and
// independently executes unmodified accepted JS 8B on those SAME identities.
// Native-v1 and JS-8B differ intentionally; a passing test DOES NOT certify
// physical WebView2, C#/Rust IPC atomicity, OAuth login or full 8B parity.
import test from 'node:test';
import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';
import {dirname, resolve} from 'node:path';
import {createProfileSessionController} from '../../extension/adapter/profile-session.mjs';

const repo = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const dll = resolve(repo, 'apps/windows-host/bin/Release/net8.0-windows/WindowsHost.dll');
const command = 'dotnet';
const raw = execFileSync(command, [dll, '--8p-native-differential-v1'], {
  cwd: repo, encoding: 'utf8', timeout: 30_000, maxBuffer: 1024 * 1024
});
const bundle = JSON.parse(raw);
assert.equal(bundle.version, 'CoSoS/8P/native-two-phase/v1');
assert.equal(bundle.browserPhysical, false);
assert.equal(bundle.rustBridgePhysical, false);
assert.ok(Array.isArray(bundle.cases) && bundle.cases.length >= 10);

const key = x => x.Profile;
const context = (x, tabId) => ({
  profileId: x.Profile, tabId, frameId: 0,
  documentId: x.DocumentId, navigationEpoch: x.NavigationEpoch,
  sessionId: x.SessionId, sessionEpoch: x.SessionEpoch,
  workerGeneration: x.WorkerGeneration, hostGeneration: x.HostGeneration,
  channelId: x.ChannelId, origin: x.Origin
});
const proofA = Object.freeze({target: 'A'});
const proofB = Object.freeze({target: 'B'});

async function runJs8B(vector) {
  const pre = new Map([[proofA, vector.beforeA], [proofB, vector.beforeB]]);
  const profiles = createProfileSessionController({
    allowedProviders: [{id:'chatgpt',origin:'https://chatgpt.com'}],
    authorizeNativeTarget: async proof => {
      if (!pre.has(proof)) throw Error('foreign-native-proof');
      return context(pre.get(proof), proof === proofA ? 1 : 2);
    },
    resolveNativePartition: async proof => {
      const x = pre.get(proof);
      if (!x) throw Error('foreign-partition');
      return {
        profileId: key(x), partitionId: x.ProfilePath,
        isolationGeneration: 1, mode: 'persistent'
      };
    },
    beginNativeSignIn: async () => {throw Error('not-a-sign-in-test');},
    commitNativeHandoff: async ({expectedSource,expectedTarget}) => {
      // Reject mocked arbitrary requested identities at the privileged boundary.
      if (pre.get(proofA).SessionId !== expectedSource.sessionId ||
          pre.get(proofB).SessionId !== expectedTarget.sessionId)
        throw Error('stale-native-commit');
      pre.set(proofA, vector.afterA);
      pre.set(proofB, vector.afterB);
      return {committed:true};
    }
  });
  try {
    const result = await profiles.handoff({
      sourceProof:proofA,targetProof:proofB,consentProof:{ownerApproved:true}
    });
    assert.equal(result.status, 'transferred');
    return true;
  } catch {
    return false;
  }
}

const seen = new Set();
for (const v of bundle.cases) {
  test(`C# production-native v1 vs unmodified JS 8B: ${v.id}`, async () => {
    assert.ok(!seen.has(v.id), 'duplicate differential vector');
    seen.add(v.id);
    assert.equal(typeof v.nativeAccepted, 'boolean');
    assert.equal(v.nativeAccepted, v.expectedNative,
      'compiled C# two-phase contract unexpectedly changed');
    const jsAccepted = await runJs8B(v);
    assert.equal(jsAccepted, v.expectedJs8B,
      'unmodified JS 8B unexpectedly changed');
    if (v.id === 'native-two-phase-fresh') {
      assert.equal(v.nativeAccepted, true);
      assert.equal(jsAccepted, false);
    }
    if (v.id === 'js-8b-stable-postcondition') {
      assert.equal(v.nativeAccepted, false);
      assert.equal(jsAccepted, true);
    }
  });
}
