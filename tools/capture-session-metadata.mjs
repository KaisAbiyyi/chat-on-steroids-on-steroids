// Extract and execute actual pure private declarations without application bootstrap.
import fs from 'node:fs/promises';
import path from 'node:path';
import vm from 'node:vm';
import {createRequire} from 'node:module';
import {fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
import {execFileSync} from 'node:child_process';
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const upstream = path.join(root, 'upstream/chat-on-steroids');
const commit = '024731d8083d92c46e8248d64e7e0b36ab0a470f';
if (execFileSync('git', ['rev-parse', 'HEAD'], {cwd: upstream, encoding: 'utf8'}).trim() !== commit) throw Error('Upstream identity changed');
const require = createRequire(path.join(upstream, 'package.json'));
const relative = 'src/main/session/store.ts';
const source = await fs.readFile(path.join(upstream, relative), 'utf8');
const ast = require('@babel/parser').parse(source, {sourceType: 'module', plugins: ['typescript']});
const names = ['emptySummary', 'normalizeSummary'];
const declarations = ast.program.body.map(node => node.type === 'ExportNamedDeclaration' ? node.declaration : node).filter(node => node?.type === 'FunctionDeclaration' && names.includes(node.id.name));
if (declarations.length !== names.length) throw Error('Missing source declarations');
const code = "const META_HISTORY_SEQ='__historySeq',META_CANONICAL_PROJECTION='__canonicalProjection',META_TOKEN_ESTIMATE='__tokenEstimate';\n" + declarations.map(node => source.slice(node.start, node.end)).join('\n') + '\nglobalThis.fixture={emptySummary,normalizeSummary};';
const transformed = await require('esbuild').transform(code, {loader: 'ts', format: 'cjs'});
const context = vm.createContext({module: {exports: {}}, exports: {}});
vm.runInContext('Date.now=()=>1792108800000;', context);
vm.runInContext(transformed.code, context);
const api = context.fixture;
const id = 'session-meta-public';
const base = {...api.emptySummary(id, 'Public persisted name', 'conversation-public'), __historySeq: 0, __canonicalProjection: 1, __tokenEstimate: 1};
const cases = [];
function capture(name, patch = {}, omit = []) {
  const raw = {...structuredClone(base), ...patch, futureField: {literal: 'keep all unknown metadata'}};
  for (const key of omit) delete raw[key];
  const serialized = JSON.stringify(raw);
  cases.push({name, raw, expected: api.normalizeSummary(id, serialized)});
}
capture('modern');
capture('legacy-defaults', {}, ['chatIds', 'origin', 'contextTokens', 'processExitNonzero', 'toolRejected', 'toolInternalErrors', 'lastAssistantFinalAt', 'lastTurnEndAt', 'lastFinishReportAt', 'lastToolCallAt', 'lastToolActivity', 'agents', 'lastCommittedResumeHandoffId']);
for (const titleSource of ['user', '', null, [], 'manual', 'provider', 'fallback']) capture('title-source-' + JSON.stringify(titleSource), {titleSource, autoTitle: {title: 'Provider public', source: 'provider'}});
for (const autoTitle of [null, [], {}, {title: '', source: 'fallback'}, {title: 'public', source: 'provider'}, {title: 3, source: 'provider'}, {title: 'public', source: 'manual'}]) capture('auto-' + JSON.stringify(autoTitle), {titleSource: 'manual', autoTitle});
for (const selectedModel of [null, {}, {conversationId: 'conversation-public', model: 'gpt-6', observedAt: 40}, {conversationId: 'conversation-public', model: '', observedAt: 40}, {conversationId: 'conversation-public', model: 'a'.repeat(81), observedAt: 40}, {conversationId: 'conversation-public', model: 'é', observedAt: 40}, {conversationId: 'conversation-public', model: 'gpt-6', observedAt: '40'}]) capture('selected-' + JSON.stringify(selectedModel), {selectedModel});
for (const lastToolActivity of [null, {}, {kind: 'run', title: 'public'}, {kind: 'unknown', title: 'public'}, {kind: 'read', title: ''}, {kind: 'read', title: '😀'.repeat(100)}, {kind: 'read', title: '😀'.repeat(101)}]) capture('activity-' + cases.length, {lastToolActivity});
const assignment = {conversationId: 'conversation-public', agentId: 'worker-public', label: 'Public worker', task: 'Public task', recordedAt: 10};
for (const workerAssignment of [null, assignment, {...assignment, label: '😀'.repeat(31)}, {...assignment, task: 'a'.repeat(8201)}, {...assignment, recordedAt: null}]) capture('assignment-' + cases.length, {workerAssignment});
const finish = {turnId: 'turn-public', startedAt: 10, notified: false, released: false, workSeq: 2, decisionSeq: 1, decisionInputRevision: null, conversationId: 'conversation-public', decisionRevision: 'a'.repeat(64)};
for (const finishTurn of [null, finish, {...finish, workSeq: 1.5}, {...finish, decisionSeq: -1}, {...finish, decisionInputRevision: 'wrong'}, {...finish, decisionRevision: ['a'.repeat(64)]}]) capture('finish-' + cases.length, {finishTurn});
capture('lineage', {chatIds: ['conversation-public', 'conversation-retired'], retiredChatAt: {'conversation-public': 3, 'conversation-retired': 4, 'conversation-unknown': 5, negative: -1}, lastCommittedResumeHandoffId: 'public-resume-1234'});
capture('bad-lineage', {retiredChatAt: [], lastCommittedResumeHandoffId: 'bad!'});
capture('invalid-times', {lastToolCallAt: '3', lastAssistantFinalAt: {}, lastTurnEndAt: false, lastFinishReportAt: null});
capture('wrong-id', {id: 'other-session'});
const rawCases = ['{broken', 'null', '[]', '42'].map(raw => ({raw, expected: api.normalizeSummary(id, raw)}));
const cache = path.join(upstream, 'node_modules/.cache/cos-rust-oracle/session-metadata');
await fs.mkdir(cache, {recursive: true});
const config = JSON.parse(await fs.readFile(path.join(root, 'fixtures/upstream/config-defaults.json'), 'utf8')).win32;
const stubs = {name: 'public-config-and-diagnostics', setup(build) {
  build.onResolve({filter: /(?:^|\/)config\.js$/}, () => ({path: 'config', namespace: 'fixture'}));
  build.onResolve({filter: /(?:^|\/)logger\.js$/}, () => ({path: 'logger', namespace: 'fixture'}));
  build.onLoad({filter: /.*/, namespace: 'fixture'}, args => ({contents: args.path === 'config' ? `export const getConfig=()=>(${JSON.stringify(config)});` : 'export const logInfo=()=>{};export const logWarn=()=>{};export const logError=()=>{};', loader: 'js'}));
}};
const bundle = path.join(cache, 'store.cjs');
await require('esbuild').build({entryPoints: [path.join(upstream, relative)], outfile: bundle, bundle: true, platform: 'node', format: 'cjs', packages: 'external', plugins: [stubs], logLevel: 'warning'});
const store = require(bundle);
const temporaryRoot = path.join(root, '.test-tmp');
await fs.mkdir(temporaryRoot, {recursive: true});
const profile = await fs.mkdtemp(path.join(temporaryRoot, 'cos-rust-public-metadata-oracle-'));
try {
  for (const [index, row] of [...cases, ...rawCases].entries()) {
    const home = path.join(profile, 'case-' + index);
    const session = path.join(home, 'sessions', id);
    await fs.mkdir(session, {recursive: true});
    await fs.writeFile(path.join(session, 'events.jsonl'), '');
    await fs.writeFile(path.join(session, 'messages.json'), '{}');
    await fs.writeFile(path.join(session, 'meta.json'), typeof row.raw === 'string' ? row.raw : JSON.stringify(row.raw));
    store.initSessionStore(home);
    row.storeExpected = await store.getSession(id);
    store.resetSessionStoreForTests();
  }
} finally {
  store.resetSessionStoreForTests();
  const resolved = await fs.realpath(profile);
  if (path.dirname(resolved) !== await fs.realpath(temporaryRoot) || !path.basename(resolved).startsWith('cos-rust-public-metadata-oracle-')) throw Error('Refusing escaped cleanup');
  await fs.rm(resolved, {recursive: true});
}
const output = path.join(root, 'fixtures/upstream/phase1-6-closure/session-metadata.json');
await fs.writeFile(output, JSON.stringify({upstreamCommit: commit, sources: {[relative]: createHash('sha256').update(source).digest('hex')}, id, base, cases, rawCases}, null, 2) + '\n');
console.log(JSON.stringify({cases: cases.length, damagedCases: rawCases.length, output}));
