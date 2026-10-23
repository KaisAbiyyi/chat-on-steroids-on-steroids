// 7L1 executable MV3 worker. No page messaging or content script listeners.
// This file is loaded only by WebView2's installed extension and accessed by
// the native host via an attested CDP service_worker target session.
import { installVerifiedCompanion } from "./worker.mjs";

const utf8 = new TextEncoder();
const decode = (hex) => {
  if (typeof hex !== "string" || hex.length % 2 || !/^[a-f0-9]+$/.test(hex)) throw Error("hex");
  return Uint8Array.from(hex.match(/../g), h => parseInt(h, 16));
};
const encode = (bytes) => Array.from(bytes, b => b.toString(16).padStart(2, "0")).join("");
const counter = (n) => {
  if (!Number.isSafeInteger(n) || n < 0) throw Error("counter");
  const out = new Uint8Array(8);
  new DataView(out.buffer).setBigUint64(0, BigInt(n), false);
  return out;
};
const concat = (...chunks) => {
  const out = new Uint8Array(chunks.reduce((a, c) => a + c.length, 0));
  let at = 0;
  for (const c of chunks) { out.set(c, at); at += c.length; }
  return out;
};
let channel = null;
let subscription;
let pending = null;
let lastSequence = 0;

async function sign(domain, sequence, payload) {
  if (!channel) throw Error("unbound");
  const bytes = utf8.encode(payload);
  const label = utf8.encode(domain);
  const framed = concat(counter(label.length), label, counter(channel.channelId),
    counter(channel.hostGeneration), counter(sequence), channel.challenge,
    counter(bytes.length), bytes);
  return encode(new Uint8Array(await crypto.subtle.sign("HMAC", channel.key, framed)));
}

const companion = installVerifiedCompanion({
  chromeApi: chrome,
  subscribeVerifiedNativeEvent(fn) { subscription = fn; },
  async verifyHostEvent(proof, event) {
    if (!pending || !proof || pending.sequence !== proof.sequence ||
        pending.payload !== proof.payload ||
        JSON.stringify(event) !== JSON.stringify(pending.event) ||
        proof.sequence !== lastSequence + 1 ||
        proof.sequence > Number.MAX_SAFE_INTEGER) throw Error("unverified-native-event");
    const receipt = await sign("CoSoS/7L1/companion-receipt/v1", proof.sequence, proof.payload);
    if (receipt !== proof.receipt) throw Error("invalid-native-receipt");
    const trusted = channel.context;
    lastSequence = proof.sequence;
    pending = null;
    return trusted;
  }
});

globalThis.__cos7l1Native = Object.freeze({
  workerInstance: crypto.randomUUID(),
  async bootstrap(spec) {
    if (!spec || !Number.isSafeInteger(spec.hostGeneration) ||
        !Number.isSafeInteger(spec.channelId) || spec.hostGeneration < 1 ||
        spec.channelId < 1 || !spec.context || pending ||
        (channel && spec.hostGeneration <= channel.hostGeneration)) throw Error("stale-bootstrap");
    const key = decode(spec.key), challenge = decode(spec.challenge);
    if (key.length !== 32 || challenge.length !== 32) throw Error("bootstrap-size");
    const context = Object.freeze({...spec.context});
    channel = Object.freeze({
      channelId: spec.channelId, hostGeneration: spec.hostGeneration,
      challenge, key: await crypto.subtle.importKey(
        "raw", key, {name:"HMAC", hash:"SHA-256"}, false, ["sign"]), context
    });
    lastSequence = 0;
    return {
      proof: await sign("CoSoS/7B/handshake/v1", 0, ""),
      workerInstance: globalThis.__cos7l1Native.workerInstance
    };
  },
  async observe(sequence) {
    if (sequence !== 1 || lastSequence !== 0 || pending) throw Error("observation-sequence");
    const identity = channel.context;
    const frame = {protocol:14, identity:{
      profileId:identity.profileId, tabId:identity.tabId, frameId:identity.frameId,
      documentId:identity.documentId, navigationEpoch:identity.navigationEpoch,
      sessionId:identity.sessionId, sessionEpoch:identity.sessionEpoch,
      workerGeneration:identity.workerGeneration
    }};
    const payload = JSON.stringify(frame);
    return {payload:encode(utf8.encode(payload)),
      mac:await sign("CoSoS/7B/observation/v1", sequence, payload)};
  },
  confirmObservation(sequence) {
    if (sequence !== 1 || lastSequence !== 0 || pending) throw Error("invalid-observation");
    lastSequence = 1;
    return true;
  },
  async prepare(event, sequence) {
    if (!channel || pending || sequence !== lastSequence + 1 || sequence < 2)
      throw Error("event-sequence");
    if (event.sequence !== sequence - 1) throw Error("ledger-sequence-map");
    const c = channel.context;
    const frame = {protocol:14, identity:{
      profileId:c.profileId, tabId:c.tabId, frameId:c.frameId,
      documentId:c.documentId, navigationEpoch:c.navigationEpoch,
      sessionId:c.sessionId, sessionEpoch:c.sessionEpoch,
      workerGeneration:c.workerGeneration
    }};
    const payload = JSON.stringify({frame,event});
    pending = {sequence,payload,event};
    return {payload:encode(utf8.encode(payload)),
      mac:await sign("CoSoS/7L1/companion-event/v1", sequence, payload)};
  },
  async accept(event, sequence, receipt) {
    if (!pending || pending.sequence !== sequence) throw Error("no-pending-event");
    return subscription(event, {sequence, payload:pending.payload, receipt});
  }
});

// Activation is browser-owned; content scripts and page messages cannot call
// the privileged adapter. This listener carries no page data into native code.
chrome.tabs.onUpdated.addListener(() => {});
void companion;
