// Actual source text projections; no browser, personal profile, clipboard or dialog is used.
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
const cache = path.join(upstream, 'node_modules/.cache/cos-rust-oracle/session-content');
await fs.mkdir(cache, {recursive: true});
const sources = ['src/shared/content-reference.ts', 'src/shared/dil-tables.ts', 'src/shared/markdown-export.ts', 'src/main/session/markdown-export.ts', 'src/shared/session.ts'];
async function load(relative, name) {
  const file = path.join(cache, name + '.cjs');
  await require('esbuild').build({entryPoints: [path.join(upstream, relative)], outfile: file, bundle: true, platform: 'node', format: 'cjs', packages: 'external', logLevel: 'warning'});
  return require(file);
}
const content = await load(sources[0], 'content');
const tables = await load(sources[1], 'tables');
const markdown = await load(sources[2], 'markdown');
function capture(fn) {
  try {
    const value = fn();
    if (typeof value === 'string' && !value.isWellFormed()) return {needsLosslessString: true, units: Array.from({length: value.length}, (_, i) => value.charCodeAt(i))};
    return {value};
  } catch (error) { return {error: {name: error.name, message: error.message}}; }
}
const pure = [];
const texts = ['', 'plain text', 'before\n::chatgpt-content-reference{index="0" source_message_id="public"}\nafter', '::widget', '  ::Widget{x="public"}\t', ':::unknown\nbody\n:::', ':::writing\nbody\n:::', ':::writing-card\nbody\n:::', ':::writing_card\nbody\n:::', '::widget\r\nnext', 'left\r::widget\rright', 'left\u2028::widget\u2029right', '::widget{x="across\rcarriage"}', '::ſpecial', '::widget\u0085', 'x ::widget', '::widget\n\n\n\nnext', '::widget{x="a}b"}', '::writing\n:::unknown\nbody\n:::'];
for (const text of texts) for (const name of ['hasProviderDirective', 'withoutProviderDirectives', 'hasContentReference']) pure.push({name, args: [text], expected: capture(() => content[name](text))});
const htmls = ['', '<p>public &amp; safe</p><div>next<br/>line</div>', '<SCRIPT>private()</SCRIPT><style>hidden</style><p>shown</p>', '<script>mismatched</style><p>shown</p>', '<scripture>literal</scripture>', '<scripté>removed by ASCII word boundary</script>', '<p>  a \t <br >b\n\n\nend </p>', '&unknown; &AMP; &#39; &#x1F600; &#65; &#x00;', '&#xD800;', '&#x110000;', '<!-- comment --><h2>Title</h2><table><tr><td>a</td></tr></table>', '<p>&#X61; &#00065; &nbsp; &apos; &quot;</p>', '<p>&lt;b&gt;literal&lt;/b&gt;</p>'];
htmls.push('&#9999999999999999999999999999999;', '&#' + '9'.repeat(400) + ';', '&#xFFFFFFFFFFFFFFFFFFFF;', '&#x123456789abcdef012345;');
for (const hex of ['20000000000000', '20000000000001', '20000000000003', '4000000000000201', 'f'.repeat(256), '7' + 'f'.repeat(255), 'f'.repeat(13) + '8' + '0'.repeat(242), '0'.repeat(300) + '41']) htmls.push('&#x' + hex + ';');
for (const text of htmls) pure.push({name: 'plainTextOfHtml', args: [text], expected: capture(() => content.plainTextOfHtml(text))});
for (const text of ['plain  text', '\ue200url\ue202 Public label \ue202 https://example.invalid/public \ue201', 'See \ue200cite\ue202public\ue201 , next', '\ue200URL\ue202Label\ue202https://example.invalid\ue201', '\ue200broken\ue202public', 'before\ue200x\ue202one\ue202two\ue201\tafter', 'x \t !  next', '\ue200url\ue202\t\ue202 https://example.invalid \ue201']) pure.push({name: 'providerEntitiesAsMarkdown', args: [text], expected: capture(() => content.providerEntitiesAsMarkdown(text))});
const captures = [null, {text: ''}, {text: '<p>Resolved <b>public</b></p>'}, {text: '<p>Resolved public</p>', truncated: true}, {text: '<p>::widget</p>'}, {text: '<p> &#xD800; </p>'}, {text: '<p>&#x110000;</p>'}, {text: '   '}];
for (const html of captures) {
  pure.push({name: 'resolvedCapture', args: [html], expected: capture(() => content.resolvedCapture(html))});
  for (const text of ['::chatgpt-content-reference{index="0"}', 'before\n:::unknown\ninner\n:::\nafter', 'ordinary \ue200url\ue202link\ue202https://example.invalid\ue201']) pure.push({name: 'modelFacingText', args: [text, html], expected: capture(() => content.modelFacingText(text, html))});
}
const tableHtml = '<div data-d-component="table"><table><tr><th><b>Name</b></th><th>Value</th></tr><tr><td>A | B</td><td>1\\2</td></tr><tr><td>C</td><td>&amp; 3</td></tr></table></div><button data-d-component="button">Open public</button>';
const tableCases = [];
for (const text of ['| Name | Value |\n| --- | --- |', 'before\n\n| **Name** | Value |\n| :--- | ---: |\n| incomplete | row |\n\nafter', '| Name | Value |\n| --- | --- |\n| one | row |\n| two | row |', '| Other | Value |\n| --- | --- |', 'Before\n\n[Open public]\n\nAfter', '[Different]\n[Open public]', '| Name | Value |\r\n| --- | --- |\r\n', '| Name | Value |\n| -- | --- |']) for (const html of [null, {text: tableHtml}, {text: tableHtml, truncated: true}]) tableCases.push({text, capture: html, expected: capture(() => tables.completedDilText(text, html))});
const stored = text => ({text, truncated: false, chars: text.length});
const event = (kind, seq, text, extra = {}) => ({kind, seq, time: 100 + seq, source: 'extension', messageId: kind + '-public', message: stored(text), ...extra});
const transcriptCases = [
  [event('user_message', 1, 'raw instructions', {authoredText: 'public request'}), event('assistant_message', 2, 'stream', {final: false}), event('assistant_message', 3, 'final public', {final: true, origin: 2, turnId: 'turn-public', renderedHtml: stored('<p>drawing</p>')}), event('assistant_message', 4, 'older revision', {final: true, origin: 2, turnId: 'turn-public'}), event('assistant_message', 2, 'out of order', {final: true, origin: 2})],
  [event('user_message', 1, '[[CLF-RESUME:public_identity_123456789]]\n\ncontrol'), event('user_message', 2, 'second', {messageId: 'second'}), event('assistant_message', 3, 'state-only final', {final: false, state: 'final'}), event('assistant_message', 4, 'actual final', {final: true, state: 'final'})],
  [event('assistant_message', 3, 'anonymous answer', {messageId: '', final: true}), event('user_message', 1, 'anonymous question', {messageId: ''}), event('assistant_message', 5, 'not final', {messageId: 'later', final: false})],
  [event('user_message', 4, 'first revised', {origin: 1}), event('assistant_message', 3, 'answer', {origin: 2, final: true}), event('user_message', 2, 'earlier body', {origin: 1})],
];
const escapedMarker = '\\[\\[CLF\\-RESUME\\:public_identity_123456789\\]\\]';
for (const units of [199, 200, 201]) {
  const text = ' '.repeat(units - escapedMarker.length) + escapedMarker + '😀tail';
  transcriptCases.push([event('user_message', 1, text)]);
}
const transcripts = transcriptCases.map(events => ({events, expected: markdown.transcriptEntries(events)}));
const lifecycle = [{kind: 'turn_end', seq: 1, time: 1, turnId: 'a', outcome: 'completed'}, {kind: 'turn_end', seq: 2, time: 2, turnId: 'b', outcome: 'completed'}, {kind: 'turn_end', seq: 3, time: 3, turnId: 'a', outcome: 'stopped'}, {kind: 'turn_end', seq: 4, time: 4, turnId: 'a', outcome: 'completed'}];
const completed = Array.from(markdown.completedTurnIds(lifecycle));
const anchoredEvents = [event('assistant_message', 1, 'first A', {messageId: 'answer-a-1', turnId: 'a', final: true}), event('assistant_message', 2, 'answer B', {messageId: 'answer-b', turnId: 'b', final: true}), event('assistant_message', 3, 'second A', {messageId: 'answer-a-2', turnId: 'a', final: true}), ...lifecycle.map((entry, index) => ({...entry, seq: index + 4}))];
const anchors = {events: anchoredEvents, completed: Array.from(markdown.completedTurnIds(anchoredEvents)), expected: Array.from(markdown.answerAnchors(anchoredEvents)), answers: ['a', 'b', 'unknown'].map(turnId => ({turnId, expected: markdown.turnAnswerEntries(anchoredEvents, turnId)}))};
const formatting = [
  {title: '  Public title  ', entries: [{role: 'user', text: '  hello  '}, {role: 'assistant', text: ' answer '}]},
  {title: '\ufeff\u00a0', entries: [{role: 'assistant', text: ''}, {role: 'user', text: '\u0085literal\u0085'}], labels: {user: 'Kamu', assistant: 'Asisten'}},
].map(row => ({...row, expected: markdown.sessionMarkdown(row.title, row.entries, row.labels)}));
const filenames = ['', ' public / unsafe:*?"<>| name.  ', 'e\u0301cole', 'a'.repeat(79) + '😀tail', '\u0085private\u0085', '\ufeffpublic\u00a0', 'CON', '😀'.repeat(50)].map(title => ({title, suffix: ' - answer', expected: markdown.markdownFileName(title, ' - answer')}));
// Execute the real main-process export function with public reads and a memory-only text sink.
const exportSource = await fs.readFile(path.join(upstream, sources[3]), 'utf8');
const ast = require('@babel/parser').parse(exportSource, {sourceType: 'module', plugins: ['typescript']});
const declarations = ast.program.body.map(node => node.type === 'ExportNamedDeclaration' ? node.declaration : node).filter(node => node?.type === 'FunctionDeclaration' && ['fullText', 'exportSessionMarkdown'].includes(node.id.name));
if (declarations.length !== 2) throw Error('Missing export declarations');
const code = declarations.map(node => exportSource.slice(node.start, node.end)).join('\n') + '\nglobalThis.exportMarkdown=exportSessionMarkdown;';
const transformed = await require('esbuild').transform(code, {loader: 'ts', format: 'cjs'});
const overflow = 'Full public overflow \ue200url\ue202link\ue202https://example.invalid\ue201';
const assetId = createHash('sha256').update(overflow).digest('hex').slice(0, 32) + '.txt';
const exports = [];
for (const [name, events] of [
  ['entities', [event('user_message', 1, 'request'), event('assistant_message', 2, 'See \ue200url\ue202Public\ue202https://example.invalid\ue201', {final: true, turnId: 'turn-public'}), {kind: 'turn_end', seq: 3, source: 'extension', time: 103, turnId: 'turn-public', outcome: 'completed'}]],
  ['drawn-table', [event('user_message', 1, 'request'), event('assistant_message', 2, '| Name | Value |\n| --- | --- |\n\n[Open public]', {final: true, turnId: 'turn-public', renderedHtml: stored(tableHtml)}), {kind: 'turn_end', seq: 3, source: 'extension', time: 103, turnId: 'turn-public', outcome: 'completed'}]],
  ['overflow', [event('user_message', 1, 'inline public', {message: {...stored('inline public'), assetId}}), event('assistant_message', 2, 'clipped', {final: true, turnId: 'turn-public', message: {text: 'clipped', truncated: true, chars: overflow.length, assetId}}), {kind: 'turn_end', seq: 3, source: 'extension', time: 103, turnId: 'turn-public', outcome: 'completed'}]],
  ['incomplete', [event('assistant_message', 1, 'final text', {final: true, turnId: 'turn-public'})]],
]) {
  let copied;
  const context = vm.createContext({...markdown, providerEntitiesAsMarkdown: content.providerEntitiesAsMarkdown, completedDilText: tables.completedDilText, getSession: async () => ({title: 'Public export'}), readEvents: async () => events, readOverflowText: async (_id, id) => id === assetId ? overflow : null, clipboard: {writeText(text) {copied = text;}}});
  vm.runInContext(transformed.code, context);
  const outcomes = [];
  for (const scope of ['session', 'answer']) {
    copied = undefined;
    try {await context.exportMarkdown({id: 'session-export-public', scope, turnId: 'turn-public', target: 'clipboard'}, null); outcomes.push({scope, text: copied});}
    catch (error) {outcomes.push({scope, error: error.message});}
  }
  exports.push({name, title: 'Public export', events, overflow: {[assetId]: overflow}, outcomes});
}
const output = path.join(root, 'fixtures/upstream/phase1-6-closure/session-content.json');
const hashes = Object.fromEntries(await Promise.all(sources.map(async relative => [relative, createHash('sha256').update(await fs.readFile(path.join(upstream, relative))).digest('hex')])));
await fs.writeFile(output, JSON.stringify({upstreamCommit: commit, scope: 'Actual pure content/table/transcript functions and real export function with public reads plus a memory-only text sink; native clipboard/dialog NOT RUN', sources: hashes, pure, tableCases, transcripts, lifecycle, completed, anchors, formatting, filenames, exports}, null, 2) + '\n');
console.log(JSON.stringify({pure: pure.length, tables: tableCases.length, transcripts: transcripts.length, exportProjections: exports.reduce((n, row) => n + row.outcomes.length, 0), filenames: filenames.length, output}));
