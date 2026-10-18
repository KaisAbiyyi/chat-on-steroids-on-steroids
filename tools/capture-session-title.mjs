// Execute pinned presentation functions and the actual store in fresh public profiles.
import fs from 'node:fs/promises';
import path from 'node:path';
import {createRequire} from 'node:module';
import {fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
import {execFileSync} from 'node:child_process';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const upstream = path.join(root, 'upstream/chat-on-steroids');
const commit = '024731d8083d92c46e8248d64e7e0b36ab0a470f';
if (execFileSync('git', ['rev-parse', 'HEAD'], {cwd: upstream, encoding: 'utf8'}).trim() !== commit) throw Error('Upstream identity changed');
const require = createRequire(path.join(upstream, 'package.json'));
const esbuild = require('esbuild');
const cache = path.join(upstream, 'node_modules/.cache/cos-rust-oracle/session-title');
await fs.mkdir(cache, {recursive: true});
async function load(relative, name, plugins = []) {
  const file = path.join(cache, name + '.cjs');
  await esbuild.build({entryPoints: [path.join(upstream, relative)], outfile: file, bundle: true, platform: 'node', format: 'cjs', packages: 'external', plugins, logLevel: 'warning'});
  return require(file);
}
const title = await load('src/main/session/title.ts', 'title');
const prompt = await load('src/shared/user-prompt.ts', 'prompt');
const stored = text => ({text, truncated: false, chars: text.length});
const message = (text, id = 'public-user', more = {}) => ({kind: 'user_message', source: 'extension', time: 100, messageId: id, message: stored(text), ...more});
const identity = '[[CLF-RESUME:public_identity_123456789]]\n\n';
const frame = prompt.prependUserPrompt('  public\n  request  ', 'internal instructions 😀');
const escapedFrame = frame.replace(/[!-/:-@[-`{-~]/g, '\\$&').replace(/\n/g, '\\\n');
const texts = ['', 'plain \\*literal', '  public\n  request  ', '\u0085public\u0085', '\ufeff public\u00a0', frame, frame.replace(/\n/g, '\r\n'), escapedFrame,
  prompt.prependUserPrompt(identity + 'public authored', 'instructions'),
  '[[COS_CONTEXT:4]]\nwrong length\n[[/COS_CONTEXT]]\n\nprivate instruction is not a title',
  ' [[COS_CONTEXT:0]]', frame.replace('internal instructions', '&#x20;internal instructions'),
  'a'.repeat(78) + '😀tail', 'a'.repeat(79) + '😀tail', 'a'.repeat(79) + '  tail',
  prompt.prependUserPrompt('literal\n[[/COS_CONTEXT]]\n\nremains authored', 'marker [[/COS_CONTEXT]] text'),
  prompt.prependUserPrompt('  indent\n&#x20;inside line & safe', '\nleading instructions'),
];
const promptCases = texts.map(text => ({text, expected: prompt.userPromptText(text), instructions: 'new instructions 😀\r\nline', prepended: prompt.prependUserPrompt(text, 'new instructions 😀\r\nline')}));
const userCases = texts.flatMap(text => [undefined, '', '  explicit\n  authored  '].map(authored => {
  const expected = title.userTitle(text, authored);
  return {text, ...(authored === undefined ? {} : {authored}), expectedUnits: Array.from({length: expected.length}, (_, i) => expected.charCodeAt(i)), needsLosslessString: !expected.isWellFormed()};
}));
const events = [message('later text', 'later', {seq: 1, origin: 20}), message(frame, 'first', {seq: 3, origin: 2}), {kind: 'note', seq: 0}];
const summarize = value => Object.fromEntries(['title', 'titleSource', 'autoTitle', 'origin'].filter(key => key in value).map(key => [key, value[key]]));
const refreshCases = [];
for (const original of [
  {title: 'ChatGPT session'}, {title: 'New chat', origin: {kind: 'desktop'}},
  {title: 'provider request', titleSource: 'provider'}, {title: 'provider request', titleSource: 'provider', origin: {kind: 'desktop'}},
  {title: 'my name', titleSource: 'manual'}, {title: 'worker task', origin: {kind: 'worker'}},
  {title: 'ChatGPT - Project public', titleSource: 'provider'}, {title: 'ChatGPT · Project public'},
  {title: 'ChatGPT | Project public', titleSource: 'manual'}, {title: '[[COS_CONTEXT:0]]'},
  {title: 'You are a task planner, not the executor: public fixture'},
  {title: '  public\n  request  '.trim()}, {title: '\u0085public\u0085'},
]) for (const rows of [[], events, [message('You are a task planner, not the executor public', 'planner', {seq: 1})]]) {
  const summary = structuredClone(original);
  const first = title.firstTitleMessage(rows);
  const automatic = title.automaticTitle(summary, first);
  const legacy = title.legacyContextTitle(summary);
  const pending = title.legacyLabelPending(summary);
  const providerIgnored = title.providerTitleIgnored(summary);
  const projectPage = title.projectPageTitle(summary.title);
  const changed = title.refreshUserTitle(summary, rows);
  refreshCases.push({summary: original, events: rows, firstMessageId: first?.messageId ?? null, automatic, legacy, pending, providerIgnored, projectPage, changed, expected: summary});
}
// The real store owns sequence, canonical shard, restart and rename operations here.
const fixtureConfig = JSON.parse(await fs.readFile(path.join(root, 'fixtures/upstream/config-defaults.json'), 'utf8')).win32;
const stubs = {name: 'public-config-and-diagnostic-sinks', setup(build) {
  build.onResolve({filter: /(?:^|\/)config\.js$/}, () => ({path: 'config', namespace: 'fixture'}));
  build.onResolve({filter: /(?:^|\/)logger\.js$/}, () => ({path: 'logger', namespace: 'fixture'}));
  build.onLoad({filter: /.*/, namespace: 'fixture'}, args => ({contents: args.path === 'config' ? `export const getConfig=()=>(${JSON.stringify(fixtureConfig)});` : 'export const logInfo=()=>{};export const logWarn=()=>{};export const logError=()=>{};', loader: 'js'}));
}};
const store = await load('src/main/session/store.ts', 'store', [stubs]);
const rename = (name, source = 'manual', conversationId) => ['rename', {title: name, source, ...(conversationId === undefined ? {} : {conversationId})}];
const scenarios = [
  {name: 'framed-revisions', options: {}, operations: [['upsert', message(frame)], ['upsert', message(prompt.prependUserPrompt('new authored\n  text', 'instructions'))], ['upsert', message('second request', 'second')], ['restart']]},
  {name: 'manual-provider-tracking', options: {}, operations: [['upsert', message('public first')], rename('Personal label'), rename('Provider public', 'provider', 'conversation-public'), rename('Wrong conversation', 'provider', 'conversation-old'), rename('cannot downgrade', 'fallback'), ['clear'], ['restart'], rename('other provider', 'provider'), rename('another manual'), rename('ChatGPT - page', 'provider'), ['clear'], ['restart']]},
  {name: 'desktop-provider-ignored', options: {title: 'New chat', origin: {kind: 'desktop'}}, operations: [['upsert', message(frame)], rename('Coding instructions', 'provider'), rename('My desktop name'), rename('latest provider', 'provider'), ['clear'], ['restart']]},
  {name: 'worker-authority', options: {title: 'worker task', origin: {kind: 'worker', fromSessionId: null, agentId: 'worker-public', task: 'public'}}, operations: [['upsert', message(frame)], rename('wrong provider', 'provider'), rename('manual worker'), rename('wrong fallback', 'fallback'), ['clear'], ['restart']]},
  {name: 'project-page-repair', options: {title: 'ChatGPT - Public project', titleSource: 'provider'}, operations: [['restart'], ['upsert', message('real request')], ['restart']]},
  {name: 'damaged-context-title', options: {title: '[[COS_CONTEXT:0]]'}, operations: [['restart'], ['upsert', message('[[COS_CONTEXT:44]]\ncorrupt')], ['restart']]},
  {name: 'planner-helper', options: {}, operations: [['upsert', message('  You are a task planner, not the executor public fixture')], ['restart']]},
  {name: 'first-message-identity', options: {}, operations: [['upsert', message('  first\n request  ', 'z-first')], ['upsert', message('second', 'a-later')], ['upsert', message('revised first', 'z-first')], ['restart']]},
  {name: 'manual-empty-auto', options: {}, operations: [rename('manual label'), rename('', 'provider'), ['clear'], ['restart']]},
];
const temporaryRoot = path.join(root, '.test-tmp');
await fs.mkdir(temporaryRoot, {recursive: true});
const profile = await fs.mkdtemp(path.join(temporaryRoot, 'cos-rust-public-title-oracle-'));
const nativeNow = Date.now;
const recovery = [];
let historicalRestart;
const selection = [];
Date.now = () => 1792108800000;
try {
  store.initSessionStore(profile);
  for (const [index, scenario] of scenarios.entries()) {
    const id = 'session-title-case-' + index;
    scenario.id = id;
    await store.createSession({reservedId: id, conversationId: 'conversation-public', ...scenario.options});
    scenario.initial = summarize(await store.getSession(id));
    scenario.steps = [];
    for (const operation of scenario.operations) {
      const [op, input] = operation;
      if (op === 'upsert') await store.upsertMessageEvent(id, input);
      else if (op === 'rename') await store.renameSession(id, input.title, input.source, input.conversationId);
      else if (op === 'clear') await store.clearSessionName(id);
      else if (op === 'restart') {await store.flushSessions(); store.resetSessionStoreForTests(); store.initSessionStore(profile);}
      else throw Error('Unknown operation');
      scenario.steps.push({operation, expected: summarize(await store.getSession(id))});
    }
    scenario.events = await store.readEvents(id);
    delete scenario.operations;
  }
  const selectionId = 'session-model-selection-public';
  await store.createSession({reservedId: selectionId, conversationId: 'conversation-public'});
  for (const operation of [
    ['observe', {conversationId: 'conversation-public', model: 'gpt-6', observedAt: 10}],
    ['observe', {conversationId: 'conversation-public', model: 'gpt-6', observedAt: 10}],
    ['observe', {conversationId: 'conversation-public', model: 'gpt-5', observedAt: 5}],
    ['observe', {conversationId: 'conversation-old', model: 'gpt-5', observedAt: 100}],
    ['observe', {conversationId: 'conversation-public', model: 'gpt-6', observedAt: 10, reasoningEffort: 'high'}],
    ['observe', {conversationId: 'conversation-public', model: '', observedAt: 20}],
    ['observe', {conversationId: 'conversation-public', model: 'a'.repeat(81), observedAt: 20}],
    ['observe', {conversationId: 'conversation-public', model: 'é', observedAt: 20}],
    ['observe', {conversationId: 'conversation-public', model: 'gpt-6', nonFinite: 'NaN'}],
    ['observe', {conversationId: 'conversation-public', model: 'gpt-6', nonFinite: 'Infinity'}],
    ['observe', {conversationId: 'conversation-public', model: 'gpt-6-sol', observedAt: 15.5, reasoningEffort: ''}],
    ['restart'], ['rebind', {from: 'conversation-public', to: 'conversation-new'}],
    ['observe', {conversationId: 'conversation-public', model: 'gpt-6', observedAt: 100}],
    ['observe', {conversationId: 'conversation-new', model: 'gpt-6', observedAt: 1}],
    ['observe', {conversationId: 'conversation-new', model: 'gpt-6', observedAt: 20, reasoningEffort: 'low'}],
    ['restart'],
  ]) {
    const [kind, input] = operation;
    if (kind === 'observe') await store.observeSessionModel(selectionId, input.conversationId, input.model, input.nonFinite ? Number(input.nonFinite) : input.observedAt, input.reasoningEffort);
    else if (kind === 'rebind') await store.rebindSession(selectionId, input.from, input.to);
    else {await store.flushSessions(); store.resetSessionStoreForTests(); store.initSessionStore(profile);}
    selection.push({operation, expected: await store.getSession(selectionId)});
  }
  for (const mode of ['stale-message', 'stale-after-rebind', 'missing-outcome-counters']) {
    const id = 'session-recovery-' + mode;
    await store.createSession({reservedId: id, title: 'Persisted manual public', titleSource: 'manual', conversationId: 'conversation-public'});
    await store.upsertMessageEvent(id, message('12345678'));
    if (mode === 'missing-outcome-counters') await store.appendEvent(id, {kind: 'tool_call', source: 'mcp', time: 110, call: {callId: 'public-call', tool: 'read', args: stored('public'), result: stored('public'), outcome: 'tool_internal_error', summary: {kind: 'read', title: 'Read public'}}});
    if (mode === 'stale-after-rebind') await store.rebindSession(id, 'conversation-public', 'conversation-new');
    await store.flushSessions();
    const metaPath = path.join(profile, 'sessions', id, 'meta.json');
    const checkpoint = JSON.parse(await fs.readFile(metaPath, 'utf8'));
    checkpoint.futureField = {literal: 'retain through recovery'};
    checkpoint.selectedModel = {conversationId: checkpoint.conversationId, model: 'gpt-6', observedAt: 80};
    if (mode === 'missing-outcome-counters') for (const key of ['processExitNonzero', 'toolRejected', 'toolInternalErrors']) delete checkpoint[key];
    else if (mode === 'stale-message') await store.upsertMessageEvent(id, message('12345678901234567890'));
    else await store.upsertMessageEvent(id, {kind: 'assistant_message', source: 'extension', time: 120, messageId: 'public-assistant', message: stored('12345678901234567890'), state: 'final', final: true});
    store.resetSessionStoreForTests();
    await fs.writeFile(metaPath, JSON.stringify(checkpoint));
    const files = {};
    async function walk(directory, relative = '') {
      for (const entry of await fs.readdir(directory, {withFileTypes: true})) {
        const name = path.posix.join(relative, entry.name);
        if (entry.isDirectory()) await walk(path.join(directory, entry.name), name);
        else files[name] = (await fs.readFile(path.join(directory, entry.name))).toString('base64');
      }
    }
    await walk(path.join(profile, 'sessions', id));
    store.initSessionStore(profile);
    const expected = await store.getSession(id);
    recovery.push({name: mode, id, files, expected, events: await store.readEvents(id)});
  }
  store.resetSessionStoreForTests();
  const historicalPath = path.join(root, 'fixtures/upstream/history-case.json');
  const historicalBytes = await fs.readFile(historicalPath);
  const historical = JSON.parse(historicalBytes);
  for (const [name, bytes] of Object.entries(historical.files)) {
    if (name.startsWith('/') || name.includes('..') || name.includes('\\')) throw Error('Unsafe historical fixture path');
    const target = path.join(profile, 'sessions', historical.id, name);
    await fs.mkdir(path.dirname(target), {recursive: true});
    await fs.writeFile(target, Buffer.from(bytes, 'base64'));
  }
  store.initSessionStore(profile);
  historicalRestart = {fixture: 'fixtures/upstream/history-case.json', fixtureSha256: createHash('sha256').update(historicalBytes).digest('hex'), summary: await store.getSession(historical.id), events: await store.readEvents(historical.id), recent: await store.readRecentEvents(historical.id, 3)};
} finally {
  await store.flushSessions();
  store.resetSessionStoreForTests();
  Date.now = nativeNow;
  const resolved = await fs.realpath(profile);
  const parent = await fs.realpath(temporaryRoot);
  if (path.dirname(resolved) !== parent || !path.basename(resolved).startsWith('cos-rust-public-title-oracle-')) throw Error('Refusing escaped cleanup');
  await fs.rm(resolved, {recursive: true});
}
const sources = ['src/main/session/title.ts', 'src/shared/user-prompt.ts', 'src/main/session/store.ts'];
const manifest = Object.fromEntries(await Promise.all(sources.map(async relative => [relative, createHash('sha256').update(await fs.readFile(path.join(upstream, relative))).digest('hex')])));
const output = path.join(root, 'fixtures/upstream/phase1-6-closure/session-title.json');
await fs.writeFile(output, JSON.stringify({upstreamCommit: commit, scope: 'Actual title/prompt functions and actual persistent store; only config and diagnostics injected; public disposable profile', sources: manifest, promptCases, userCases, refreshCases, scenarios}, null, 2) + '\n');
await fs.writeFile(path.join(root, 'fixtures/upstream/phase1-6-closure/session-recovery.json'), JSON.stringify({upstreamCommit: commit, sources: manifest, scope: 'Actual source durable recovery from stale public checkpoints and canonical shards; metadata-only future fields retained', recovery}, null, 2) + '\n');
await fs.writeFile(path.join(root, 'fixtures/upstream/phase1-6-closure/session-history-restart.json'), JSON.stringify({upstreamCommit: commit, sources: manifest, scope: 'Actual source cold load of the unchanged historical baseline files; warm output remains in the original fixture', ...historicalRestart}, null, 2) + '\n');
await fs.writeFile(path.join(root, 'fixtures/upstream/phase1-6-closure/session-model-selection.json'), JSON.stringify({upstreamCommit: commit, sources: manifest, id: 'session-model-selection-public', scope: 'Actual persistent observeSessionModel operations, restart and rebind; non-finite times explicitly described', selection}, null, 2) + '\n');
console.log(JSON.stringify({promptCases: promptCases.length, userCases: userCases.length, refreshCases: refreshCases.length, storeSteps: scenarios.reduce((n, scenario) => n + scenario.steps.length, 0), output}));
