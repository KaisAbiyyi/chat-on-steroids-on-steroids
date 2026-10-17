// Public test values only; never opens the normal application profile.
const { app, safeStorage } = require('electron');
const fs = require('node:fs');
const path = require('node:path');
const dir = process.argv.at(-1);
if (!dir || !path.isAbsolute(dir) || !dir.includes('phase1-3-runtime')) throw new Error('Explicit isolated probe directory required');
fs.mkdirSync(dir, { recursive: true });
app.setPath('userData', path.join(dir, 'electron-profile'));
app.setPath('sessionData', path.join(dir, 'electron-profile'));
app.whenReady().then(async () => {
  const payload = JSON.stringify({ bridgeToken: 'public-test-token', openaiApiKey: 'public-test-key', 'future:key': 'preserved' });
  const blob = await safeStorage.encryptStringAsync(payload);
  fs.writeFileSync(path.join(dir, 'electron-secrets.bin'), blob);
  const decrypted = await safeStorage.decryptStringAsync(blob);
  const native = path.join(dir, 'rust-secrets.bin');
  let rustRead = null;
  if (fs.existsSync(native)) {
    const answer = await safeStorage.decryptStringAsync(fs.readFileSync(native));
    rustRead = (typeof answer === 'string' ? answer : answer.result) === payload;
  }
  const metadata = { electron: process.versions.electron, platform: process.platform, userData: app.getPath('userData'), sessionData: app.getPath('sessionData'), available: await safeStorage.isAsyncEncryptionAvailable(), prefixHex: blob.subarray(0, 12).toString('hex'), bytes: blob.length, electronRoundTrip: (typeof decrypted === 'string' ? decrypted : decrypted.result) === payload, rustRead };
  fs.writeFileSync(path.join(dir, 'crypto-probe.json'), JSON.stringify(metadata, null, 2));
  console.log(JSON.stringify(metadata));
  setImmediate(() => app.quit());
}).catch(error => { console.error(error.message); app.exit(1); });
