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
const cache = path.join(upstream, 'node_modules/.cache/cos-rust-oracle/session-activity');
await fs.mkdir(cache, {recursive: true});
const sources = ['src/main/session/store.ts', 'src/shared/session.ts', 'src/shared/chronology.ts'];
async function load(relative, name) {
  const file = path.join(cache, name + '.cjs');
  await require('esbuild').build({entryPoints: [path.join(upstream, relative)], outfile: file, bundle: true, platform: 'node', format: 'cjs', packages: 'external', logLevel: 'warning'});
  return require(file);
}
const session = await load(sources[1], 'session');
const chronology = await load(sources[2], 'chronology');
const source = await fs.readFile(path.join(upstream, sources[0]), 'utf8');
const ast = require('@babel/parser').parse(source, {sourceType: 'module', plugins: ['typescript']});
const names = ['emptySummary', 'noteFinishWork', 'applyToSummary'];
const declarations = ast.program.body.map(node => node.type === 'ExportNamedDeclaration' ? node.declaration : node).filter(node => node?.type === 'FunctionDeclaration' && names.includes(node.id.name));
if (declarations.length !== names.length) throw Error('Missing declarations');
const code = declarations.map(node => source.slice(node.start, node.end)).join('\n') + '\nglobalThis.fixture={emptySummary,applyToSummary};';
const transformed = await require('esbuild').transform(code, {loader: 'ts', format: 'cjs'});
const context = vm.createContext({module: {exports: {}}, exports: {}, eventTokens: session.eventTokens, normalizedToolOutcome: session.normalizedToolOutcome, workSequence: session.workSequence, applyTurnIdentity: chronology.applyTurnIdentity});
vm.runInContext('Date.now=()=>1792108800000;', context);
vm.runInContext(transformed.code, context);
const api = context.fixture;
const stored = text => ({text, truncated: false, chars: text.length});
const call = (time, title, more = {}) => ({kind: 'tool_call', source: 'mcp', seq: 1, time, call: {callId: 'public-call', tool: 'read', args: stored('public'), result: stored('public result'), summary: {kind: 'read', title}, outcome: 'ok', ...more}});
const revision = 'a'.repeat(64);
const start = {kind: 'turn_start', source: 'extension', seq: 1, time: 100, turnId: 'turn-public'};
const work = {kind: 'user_message', source: 'extension', seq: 2, time: 110, turnId: 'turn-public', messageId: 'public-user', message: stored('public'), contentSeq: 6};
const control = (state, more = {}) => ({kind: 'progress', source: 'app', seq: 3, time: 120, turnId: 'turn-public', message: stored('public'), progressId: 'public-progress', finishControl: {state, conversationId: 'conversation-public', ...more}});
const sequences = [
  {name: 'recent-activity', events: [call(120, 'Newest'), {...call(100, 'Older'), seq: 2}, {...call(120, 'Equal newest', {endsActivity: true}), seq: 3}]},
  {name: 'fractional-time', events: [call(1.5, 'Fractional activity')]},
  ...['a'.repeat(205), '😀'.repeat(101), 'a'.repeat(199) + '😀'].map((title, i) => ({name: 'utf16-activity-' + i, events: [call(120, title)]})),
  {name: 'finish-work', events: [start, work, control('decision', {revision, workSeq: 6, inputRevision: 'b'.repeat(64)}), {...work, seq: 4, contentSeq: 8}, control('released')]},
  {name: 'notified-does-not-decide', events: [start, work, control('notified', {revision, workSeq: 90})]},
  {name: 'invalid-decision-sequence', events: [start, work, control('decision', {revision, workSeq: '90'})]},
  {name: 'negative-decision-sequence', events: [start, work, control('decision', {revision, workSeq: -2})]},
  {name: 'notified-does-not-advance-decision', events: [start, work, control('decision', {revision, workSeq: 6}), {...control('notified', {revision, workSeq: 99}), progressId: 'finish-goal:turn-public:' + revision, seq: 4}]},
  {name: 'repeat-decision-advances', events: [start, work, control('decision', {revision, workSeq: 6}), {...control('decision', {revision, workSeq: 8}), seq: 4}]},
  {name: 'legacy-finish-control', events: [start, work, {kind: 'progress', source: 'app', seq: 3, time: 120, turnId: 'turn-public', progressId: 'finish-goal:turn-public:' + revision, message: stored('public')}]},
  {name: 'empty-turn-and-agent', events: [start, {...work, turnId: '', agent: ''}]},
  {name: 'outcome-taxonomy', events: [call(100, 'Failure', {outcome: 'process_exit_nonzero'}), {...call(110, 'Rejected', {outcome: 'tool_rejected'}), seq: 2}, {...call(120, 'Internal', {outcome: 'tool_internal_error'}), seq: 3}, {...call(130, 'Legacy exit', {tool: 'exec_command', outcome: 'error', summary: {kind: 'run', title: 'Legacy', metric: '✕ exit -7'}}), seq: 4}]},
];
for (const row of sequences) {
  const summary = api.emptySummary('session-activity-public', 'Public activity', 'conversation-public');
  row.initial = structuredClone(summary);
  row.expected = [];
  for (const event of row.events) {
    api.applyToSummary(summary, event);
    const snapshot = structuredClone(summary);
    const text = snapshot.lastToolActivity?.title;
    if (text !== undefined) {
      snapshot.lastToolActivity.titleUnits = Array.from({length: text.length}, (_, i) => text.charCodeAt(i));
      snapshot.lastToolActivity.needsLosslessString = !text.isWellFormed();
      delete snapshot.lastToolActivity.title;
    }
    row.expected.push(snapshot);
  }
}
const output = path.join(root, 'fixtures/upstream/phase1-6-closure/session-activity.json');
const hashes = Object.fromEntries(await Promise.all(sources.map(async relative => [relative, createHash('sha256').update(await fs.readFile(path.join(upstream, relative))).digest('hex')])));
await fs.writeFile(output, JSON.stringify({upstreamCommit: commit, scope: 'Actual private summary reducer with actual shared session/chronology functions; fixed public clock', sources: hashes, sequences}, null, 2) + '\n');
console.log(JSON.stringify({sequences: sequences.length, projections: sequences.reduce((n, row) => n + row.expected.length, 0), output}));
