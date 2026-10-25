// Native-only MV3 transport handshake. No permissions, message/content-script
// bridge, provider page reads, storage, tabs or browser observation commands.
// Bootstrap secret reaches ONLY a host-verified extension worker CDP session.
const encoder = new TextEncoder();
const decode = hex => {
  if (typeof hex !== "string" || hex.length !== 64 ||
      !/^[0-9a-f]{64}$/.test(hex)) throw Error("8P:secret-format");
  return Uint8Array.from(hex.match(/../g), pair => parseInt(pair, 16));
};
const encode = bytes =>
  Array.from(bytes, b => b.toString(16).padStart(2, "0")).join("");
const counter = value => {
  if (!Number.isSafeInteger(value) || value < 0) throw Error("8P:counter");
  const result = new Uint8Array(8);
  new DataView(result.buffer).setBigUint64(0, BigInt(value), false);
  return result;
};
const concat = (...parts) => {
  const result = new Uint8Array(parts.reduce((total, item) => total + item.length, 0));
  let index = 0;
  for (const part of parts) { result.set(part, index); index += part.length; }
  return result;
};
let generation = 0;
const instance = crypto.randomUUID();
globalThis.__cos7l1Native = Object.freeze({
  workerInstance:instance,
  async bootstrap(spec) {
    if (!spec || !Number.isSafeInteger(spec.hostGeneration) ||
        !Number.isSafeInteger(spec.channelId) || spec.hostGeneration <= generation ||
        spec.hostGeneration < 1 || spec.channelId < 1 ||
        !spec.context || typeof spec.context !== "object" ||
        Object.getPrototypeOf(spec.context) !== Object.prototype)
      throw Error("8P:stale-or-foreign-bootstrap");
    const expectedKeys = [
      "profileId","tabId","frameId","documentId","navigationEpoch",
      "sessionId","sessionEpoch","workerGeneration","hostGeneration","channelId"
    ];
    if (Object.keys(spec.context).sort().join("|") !== expectedKeys.sort().join("|") ||
        spec.context.hostGeneration !== spec.hostGeneration ||
        spec.context.channelId !== spec.channelId)
      throw Error("8P:context-drift");
    const challenge = decode(spec.challenge), key = decode(spec.key);
    const privateKey = await crypto.subtle.importKey("raw",key,
      {name:"HMAC",hash:"SHA-256"},false,["sign"]);
    key.fill(0);
    const label=encoder.encode("CoSoS/7B/handshake/v1");
    const payload=concat(counter(label.length),label,counter(spec.channelId),
      counter(spec.hostGeneration),counter(0),challenge,counter(0));
    const mac = new Uint8Array(await crypto.subtle.sign("HMAC", privateKey, payload));
    generation = spec.hostGeneration;
    return Object.freeze({proof:encode(mac),workerInstance:instance});
  }
});
// A worker restart is a fresh, untrusted instance until the host reattests it.
// No chrome.tabs/storage/host APIs or port/listener accepting page data.
chrome.runtime.onInstalled.addListener(() => {});
