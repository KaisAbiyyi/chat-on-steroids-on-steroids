// Additive source oracle for CL-L1-F1/F2. Never rewrites the historical corpus.
// Prerequisites: pinned upstream checkout; npm install --prefix .cache/cl-l1-oracle esbuild@0.25.12
// Fetch PR #10's immutable evidence commit before running this script.
import fs from 'node:fs/promises';
import path from 'node:path';
import {createRequire} from 'node:module';
import {fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
import {execFileSync} from 'node:child_process';
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const upstream = path.join(root, 'upstream/chat-on-steroids');
const commit = '024731d8083d92c46e8248d64e7e0b36ab0a470f';
const evidenceCommit = '12b2523f75799125ebfc714ba106e57048ef2226';
const evidencePath = 'audit/2026-10-20/cl-l1-windows-06f586e/native-runtime-projections.json';
if (execFileSync('git', ['rev-parse', 'HEAD'], {cwd: upstream, encoding: 'utf8'}).trim() !== commit) throw Error('Upstream identity changed');
if (execFileSync('git', ['status', '--porcelain'], {cwd: upstream, encoding: 'utf8'}).trim()) throw Error('Upstream must be clean');
const cache = path.join(root, '.cache/cl-l1-oracle');
const require = createRequire(path.join(cache, 'package.json'));
const sources = ['src/main/session/summarize.ts', 'src/main/mcp/call-context.ts', 'src/main/diffstat.ts'];
const hashes = Object.fromEntries(await Promise.all(sources.map(async relative => [relative, createHash('sha256').update(await fs.readFile(path.join(upstream, relative))).digest('hex')])));
const original = JSON.parse(await fs.readFile(path.join(root, 'fixtures/upstream/phase1-6-closure/session-summaries.json'), 'utf8'));
for (const relative of sources) if (hashes[relative] !== original.sources[relative]) throw Error(`Source hash changed: ${relative}`);
async function load(relative, name) {
  const file = path.join(cache, name + '.cjs');
  await require('esbuild').build({entryPoints: [path.join(upstream, relative)], outfile: file, bundle: true, platform: 'node', format: 'cjs', packages: 'external', logLevel: 'warning'});
  return require(file);
}
const api = await load(sources[0], 'summarize');
const context = await load(sources[1], 'context');
const native = JSON.parse(execFileSync('git', ['show', `${evidenceCommit}:${evidencePath}`], {cwd: root, encoding: 'utf8'}));
const inputs = native.map(({case: name, stage, call}) => ({
  group: 'native', name: `${name}/${stage}/${call.callId}`, tool: call.tool,
  args: JSON.parse(call.args.text), evidence: call.evidence, outcome: call.outcome,
  durationMs: call.durationMs, resultHead: '',
}));
for (const [name, evidence, outcome = 'ok'] of [
  ['completed', {exitCode: 0, running: false}],
  ['failed', {exitCode: 7, running: false}, 'process_exit_nonzero'],
  ['benign-exit', {exitCode: 1, benignExit: true, running: false}],
  ['running', {exitCode: null, running: true}],
  ['missing-exit', {}],
  ['internal-error', {exitCode: 0, running: false}, 'tool_internal_error'],
]) inputs.push({group: 'completion', name, tool: 'write_stdin', args: {session_id: 42, chars: ''}, evidence, outcome, durationMs: 5, resultHead: ''});
for (const [name, args] of [
  ['69-units', {cmd: 'a'.repeat(69)}],
  ['70-units', {cmd: 'a'.repeat(70)}],
  ['71-units', {cmd: 'a'.repeat(71)}],
  ['astral-at-boundary', {cmd: 'a'.repeat(68) + '😀tail'}],
  ['bmp-over-boundary', {cmd: 'é'.repeat(71)}],
  ['whitespace', {cmd: '# comment\r\n\r\n  echo\t\tpublic   text  \r\n  echo next'}],
  ['js-whitespace', {cmd: '\uFEFFecho\u00A0\u2003public\uFEFF text\uFEFF'}],
  ['non-js-whitespace', {cmd: 'echo\u0085public'}],
  ['multiline', {cmd: 'echo public\necho second'}],
  ['empty', {cmd: '# comment\n  '}],
  ['commands-array', {cmds: ['a'.repeat(68), 3, 'echo second']}],
  ['empty-commands-array', {cmds: [], cmd: 'echo fallback'}],
]) inputs.push({group: 'label', name, tool: 'exec_command', args, evidence: {running: true, durationMs: 5}, outcome: 'ok', durationMs: 5, resultHead: ''});
const cases = inputs.map(input => {
  const evidence = {...context.emptyEvidence(), ...input.evidence};
  const expected = api.summarizeToolCall({...input, evidence});
  const running = api.summarizeRunningCall(input.tool, input.args, evidence);
  // CL-04C owns unpaired UTF-16 outputs. This additive fixture has no exclusions.
  for (const summary of [expected, running]) if (!summary.title.isWellFormed()) throw Error(`Unrepresentable case: ${input.name}`);
  return {...input, expected, running};
});
const output = path.join(root, 'fixtures/upstream/phase1-6-closure/cl-l1-captions.json');
await fs.writeFile(output, JSON.stringify({upstreamCommit: commit, sources: hashes,
  nativeEvidence: {commit: evidenceCommit, path: evidencePath, productSha: '06f586e423e5abb4f95eba582996c4a5278c1311'},
  scope: 'Actual pinned completed/running functions; 14 archived native inputs plus completion and representable command-label boundaries. No tools executed.',
  cases}, null, 2) + '\n');
console.log(JSON.stringify({output, cases: cases.length, nativeCases: native.length}));
