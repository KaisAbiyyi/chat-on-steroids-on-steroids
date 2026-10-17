/** Runs pinned source in an isolated Node oracle; does not start Electron or touch real AppData. */
import fs from 'node:fs/promises';
import path from 'node:path';
import os from 'node:os';
import { createHash } from 'node:crypto';
import { createRequire } from 'node:module';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const upstream = path.join(root, 'upstream/chat-on-steroids');
const expected = '024731d8083d92c46e8248d64e7e0b36ab0a470f';
const actual = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: upstream, encoding: 'utf8' }).trim();
if (actual !== expected) throw new Error('Upstream identity changed; baseline regeneration refused');
const require = createRequire(path.join(upstream, 'package.json'));
const esbuild = require('esbuild');
const cache = path.join(upstream, 'node_modules/.cache/cos-rust-oracle');
const out = path.join(root, 'fixtures/upstream');
await fs.mkdir(cache, { recursive: true });
await fs.mkdir(out, { recursive: true });
const sources = ['src/main/config.ts', 'src/main/session/store.ts', 'src/main/session/recorder.ts', 'src/main/session/correlation.ts', 'src/shared/session.ts', 'src/shared/chronology.ts', 'src/main/secrets.ts'];
const hashes = {};
for (const rel of sources) hashes[rel] = createHash('sha256').update(await fs.readFile(path.join(upstream, rel))).digest('hex');

const configSource = await fs.readFile(path.join(upstream, 'src/main/config.ts'), 'utf8');
await esbuild.build({ stdin: { contents: configSource + '\nexport { configSchema, conservativeRecoveryConfig, adoptCurrentGoalPrompt, adoptWiderWindow, adoptAutoCompaction, recalibrateTokens, DEFAULT_CAPABILITIES, SUPERSEDED_GOAL_SYSTEM_PROMPTS, SUPERSEDED_GOAL_OBJECTIVE_SYSTEM_PROMPTS, SUPERSEDED_GOAL_LOOP_SYSTEM_PROMPTS, SUPERSEDED_HELPER_CHAT_MODELS };', resolveDir: path.join(upstream, 'src/main'), sourcefile: 'config.ts', loader: 'ts' }, outfile: path.join(cache, 'config.cjs'), bundle: true, platform: 'node', format: 'cjs', packages: 'external', logLevel: 'warning' });
const config = require(path.join(cache, 'config.cjs'));
const write = (name, value) => fs.writeFile(path.join(out, name), JSON.stringify(value, null, 2) + '\n');
const platformDefaults = Object.fromEntries(['win32', 'darwin', 'linux'].map(p => [p, config.defaultConfig(p)]));
await write('config-defaults.json', platformDefaults);
await write('config-recovery.json', config.conservativeRecoveryConfig());
await write('config-migrations.json', Object.fromEntries(['DEFAULT_CAPABILITIES', 'SUPERSEDED_GOAL_SYSTEM_PROMPTS', 'SUPERSEDED_GOAL_OBJECTIVE_SYSTEM_PROMPTS', 'SUPERSEDED_GOAL_LOOP_SYSTEM_PROMPTS', 'SUPERSEDED_HELPER_CHAT_MODELS'].map(k => [k, config[k]])));

function clean(value) {
  if (value === undefined) return { $undefined: true };
  return value;
}
const transforms = new Map();
function schema(node) {
  const d = node._zod.def;
  const result = { type: d.type };
  if (d.type === 'object') {
    result.properties = Object.fromEntries(Object.entries(d.shape).map(([k, v]) => [k, schema(v)]));
    result.strict = d.catchall?._zod?.def?.type === 'never';
  } else if (d.type === 'array') result.element = schema(d.element);
  else if (d.type === 'enum') result.values = Object.values(d.entries);
  else if (d.type === 'union') result.options = d.options.map(schema);
  else if (d.type === 'literal') result.values = d.values;
  else if (d.type === 'pipe') { result.input = schema(d.in); result.output = schema(d.out); }
  else if (d.type === 'transform') { result.source = d.transform.toString(); transforms.set(result.source, true); }
  else if (['optional', 'default', 'catch'].includes(d.type)) {
    result.inner = schema(d.innerType);
    if (d.type === 'default') result.value = clean(d.defaultValue);
    if (d.type === 'catch') result.value = clean(d.catchValue({}));
  }
  if (d.checks?.length) result.checks = d.checks.map(check => {
    const def = check._zod.def;
    const c = { ...def };
    if (def.pattern instanceof RegExp) c.pattern = def.pattern.source;
    if (def.tx) c.source = def.tx.toString();
    if (def.fn) c.source = def.fn.toString();
    delete c.tx; delete c.fn;
    return c;
  });
  return result;
}
await write('config-rules.json', schema(config.configSchema));
await write('config-transforms.json', [...transforms.keys()]);
const base = config.defaultConfig('win32');
const cases = [];
function add(name, mutate) {
  const input = structuredClone(base); mutate(input);
  const parsed = config.configSchema.safeParse(input);
  cases.push({ name, input, valid: parsed.success, ...(parsed.success ? { expected: parsed.data, loaded: config.adoptCurrentGoalPrompt(config.adoptWiderWindow(config.adoptAutoCompaction(config.recalibrateTokens(parsed.data)))) } : { issuePaths: parsed.error.issues.map(i => i.path) }) });
}
add('fresh-normalized', () => {});
add('legacy-conservative-permissions', c => { c.capabilities = {}; delete c.multiAgent; delete c.sessions; delete c.compaction; delete c.mcp; delete c.controlApi; });
add('powershell-does-not-grant-deleteFolder', c => { c.capabilities = { powershell: true, deleteFolder: true }; });
add('reserved-and-duplicate-root-names', c => { c.roots = [{name:'skills',path:'C:\\one'},{name:'skills',path:'C:\\two'},{name:'project',path:'C:\\three'},{name:'project',path:'C:\\four'}]; });
add('recording-cannot-be-disabled', c => { c.sessions.record = false; c.sessions.retainDays = 1; c.sessions.storageLimitGiB = 3; });
add('old-context-and-compaction-defaults', c => { c.sessions.advisoryTokens = 180000; c.sessions.limitTokens = 200000; c.compaction.auto = false; c.compaction.autoTokens = 300000; });
add('custom-context-preserved', c => { c.sessions.advisoryTokens = 234567; c.sessions.limitTokens = 345678; c.compaction.autoTokens = 234567; });
add('actions-require-api-enabled', c => { c.controlApi = { enabled: false, allowActions: true }; });
add('malformed-safe-fields-repair', c => { c.controlApi = { enabled: 'yes', allowActions: 9 }; c.goal.mode = 'future'; c.goal.impulseMinutes = -5; c.ui.rotateTurnMinutes = 'future'; c.ui.followUp = 'future'; c.ui.backgroundPlacement = 'future'; });
add('unknown-preference-does-not-grant', c => { c.ui.chatBrowser = 'future-browser'; });
add('invalid-capability-refused', c => { c.capabilities.command = 'yes'; });
add('invalid-root-refused', c => { c.roots = [{name:'../escape',path:'C:\\project'}]; });
add('prompt-trimming-and-slicing', c => { c.goal.model = '  '; c.goal.prompt = '   '; c.mcp.instructions = ' '.repeat(5) + 'x'.repeat(4100); });
add('ui-catches-drop-unknown-fields', c => { c.ui.defaultChatModel = 2; c.ui.defaultChatReasoning = 'future'; c.ui.lastSeenVersion = ''; c.unknownFutureTop = true; });
add('duplicate-setup-profile-rejected', c => { c.setupProfiles = [1,2].map(() => ({id:'same',name:'Profile',tunnelId:'',desktopTunnelId:'',pluginsTunnelId:''})); });
add('schema-unicode-limit-counts-codepoints', c => { c.tunnel.tunnelId = '😀'.repeat(65); });
add('schema-unicode-over-limit-refused', c => { c.tunnel.tunnelId = '😀'.repeat(129); });
add('strict-browser-preferences-rejected', c => { c.ui.browserPreferences = {overwrite:false,durations:false,unknown:true}; });
add('unknown-control-api-object-repairs', c => { c.controlApi = null; c.mcp = null; });
await write('config-cases.json', cases);
await esbuild.build({ entryPoints: [path.join(upstream, 'src/shared/session.ts')], outfile:path.join(cache,'session.cjs'), bundle:true,platform:'node',format:'cjs',packages:'external',logLevel:'warning' });
await esbuild.build({ entryPoints: [path.join(upstream, 'src/shared/chronology.ts')], outfile:path.join(cache,'chronology.cjs'),bundle:true,platform:'node',format:'cjs',packages:'external',logLevel:'warning' });
const session = require(path.join(cache,'session.cjs'));
const textCases = ['', 'abc', 'abcd', 'abcde', '😀', '😀😀😀', 'a\n\tb'];
await write('text-tokens.json', textCases.map(text => ({text,expected:session.estimateTokens(text)})));
await write('baseline-manifest.json', { upstreamCommit:actual, sourceHashes:hashes, generatedBy:'tools/capture-upstream-baseline.mjs', scope:'actual pinned source functions in isolated Node; no live provider or Electron UI claim', configCases:cases.length, transformations:[...transforms.keys()].length, platform:process.platform, node:process.version });
console.log(JSON.stringify({upstream:actual,configCases:cases.length,transforms:[...transforms.keys()],fixtureDirectory:out},null,2));
