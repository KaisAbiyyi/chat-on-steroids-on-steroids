// Actual pure timeline captions from the pinned source; no tools are executed.
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
const cache = path.join(upstream, 'node_modules/.cache/cos-rust-oracle/session-summaries');
await fs.mkdir(cache, {recursive: true});
async function load(relative, name) {
  const file = path.join(cache, name + '.cjs');
  await require('esbuild').build({entryPoints: [path.join(upstream, relative)], outfile: file, bundle: true, platform: 'node', format: 'cjs', packages: 'external', logLevel: 'warning'});
  return require(file);
}
const sources = ['src/main/session/summarize.ts', 'src/main/mcp/call-context.ts', 'src/main/diffstat.ts'];
const api = await load(sources[0], 'summarize');
const context = await load(sources[1], 'context');
function lossless(value) {
  if (typeof value === 'string' && !value.isWellFormed()) return {$utf16: Array.from({length: value.length}, (_, i) => value.charCodeAt(i))};
  if (Array.isArray(value)) return value.map(lossless);
  if (value && typeof value === 'object') return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, lossless(item)]));
  return value;
}
const change = {path: '/p/src/a.rs', added: 2, removed: 1, approximate: false};
const variants = [
  ['read', {paths: ['/p/src/a.rs']}], ['read', {paths: ['/p/src/a.rs'], start_line: 2, end_line: 8}],
  ['read', {paths: ['/p/a', '/p/b', '/p/c', '/p/d']}], ['read', {paths: []}], ['read', {paths: ['/p/a']}, {detail: 'lines 5-9 of 100'}],
  ['find', {query: 'public "needle"', mode: 'content'}, {count: 1}], ['find', {query: 'a'.repeat(59) + '😀'}], ['find', {query: 'public'}, {count: 0}],
  ...['Add File', 'Delete File', 'Update File', 'Move to'].flatMap(header => [[], [change], [change, {path: '/p/b', added: 0, removed: 3, approximate: true}]].map(changes => ['apply_patch', {patch: `*** Begin Patch\n*** ${header}: a\n*** End Patch`}, {changes}])),
  ['exec_command', {cmd: '# comment\n\nWrite-Output public\nWrite-Output second'}, {running: true, durationMs: 1250}],
  ['exec_command', {cmds: ['echo one', 3, 'echo two']}, {exitCode: 7, durationMs: 1150}], ['exec_command', {cmd: 'echo public'}, {timedOut: true}],
  ['exec_command', {}, {exitCode: 7, benignExit: true, durationMs: 1250}], ['exec_command', {cmd: 'a'.repeat(69) + '😀tail'}, {running: true}],
  ...['kill', 'int', null].flatMap(signal => ['', 'public'].map(chars => ['write_stdin', {session_id: 2, chars, ...(signal ? {signal} : {})}, {exitCode: signal ? null : 7}])),
  ['observe', {what: 'windows'}, {count: 1}], ['observe', {what: 'ui', match: 'public'}, {count: 3}], ['observe', {wait_for: 'Public window'}], ['observe', {}],
  ...['list_windows', 'get_window', 'list_apps', 'get_window_state', 'launch_app', 'click', 'press_key', 'type_text', 'scroll', 'set_value', 'drag', 'perform_secondary_action', 'activate_window', 'read_clipboard', 'write_clipboard'].map(tool => [tool, {}]),
  ['computer', {actions: [{type: 'read_clipboard'}, {type: 'write_clipboard'}]}], ['computer', {actions: [{type: 'click'}, {type: 'type'}, {type: 'click'}]}], ['computer', {actions: []}],
  ['update_plan', {plan: [{step: 'public', status: 'completed'}, {step: 'next', status: 'pending'}]}], ['update_plan', {}],
  ['session', {action: 'search', query: 'public'}, {count: 2}], ['session', {action: 'read', tool_call: 'public'}], ['session', {action: 'read', cursor: 'public'}], ['session', {}],
  ['agents', {action: 'spawn', workers: [{}, {}]}], ['agents', {action: 'message', messages: [{to: 'A'}, {to: 'B'}]}], ['agents', {action: 'message', to: 'prime'}], ['agents', {action: 'finish'}], ['agents', {}, {count: 1}], ['unknown_public', {}],
];
const cases = [];
for (const [tool, args, evidence = {}] of variants) for (const outcome of ['ok', 'process_exit_nonzero', 'tool_rejected', 'tool_execution_error', 'tool_internal_error']) {
  const input = {tool, args, evidence: {...context.emptyEvidence(), ...evidence}, outcome, durationMs: 2.5, resultHead: 'public error detail'};
  const expected = api.summarizeToolCall(input);
  const encoded = lossless(expected);
  cases.push({tool, args, evidence, outcome, durationMs: input.durationMs, resultHead: input.resultHead, needsLosslessString: JSON.stringify(encoded) !== JSON.stringify(expected), expected: encoded, running: lossless(api.summarizeRunningCall(tool, args, input.evidence))});
}
for (const durationMs of [0, 0.5, -0.5, -1.5, 999.5, 1000, 1050, 1150, 1250, 1350, 1750, 59_999, 60_000, 90_000, 150_000]) {
  const input = {tool: 'exec_command', args: {cmd: 'public'}, evidence: {...context.emptyEvidence(), running: false, exitCode: 0, durationMs}, outcome: 'ok', durationMs};
  cases.push({tool: input.tool, args: input.args, evidence: {running: false, exitCode: 0, durationMs}, outcome: input.outcome, durationMs, expected: api.summarizeToolCall(input), running: api.summarizeRunningCall(input.tool, input.args, input.evidence)});
}
for (const resultHead of ['apply_patch verification failed: Failed to find expected lines in /p/a', "apply_patch verification failed: Failed to find context 'public'", 'permission refused']) {
  const input = {tool: 'apply_patch', args: {patch: '*** Begin Patch\n*** Update File: a\n*** End Patch'}, evidence: context.emptyEvidence(), outcome: 'tool_rejected', durationMs: 0, resultHead};
  cases.push({tool: input.tool, args: input.args, evidence: {}, outcome: input.outcome, durationMs: 0, resultHead, expected: api.summarizeToolCall(input), running: api.summarizeRunningCall(input.tool, input.args, input.evidence)});
}
const paths = ['', '/p/src/a.rs', '//p//a//', 'relative', '/😀/a', null, 42, ['p', 'a']].map(value => ({value, expected: api.shortPath(value)}));
const hashes = Object.fromEntries(await Promise.all(sources.map(async relative => [relative, createHash('sha256').update(await fs.readFile(path.join(upstream, relative))).digest('hex')])));
const output = path.join(root, 'fixtures/upstream/phase1-6-closure/session-summaries.json');
await fs.writeFile(output, JSON.stringify({upstreamCommit: commit, scope: 'Actual pure summary/running-caption functions with actual emptyEvidence; no desktop/process/agent tool was executed', sources: hashes, emptyEvidence: context.emptyEvidence(), paths, cases}, null, 2) + '\n');
console.log(JSON.stringify({cases: cases.length, pathCases: paths.length, losslessGaps: cases.filter(row => row.needsLosslessString).length, output}));
