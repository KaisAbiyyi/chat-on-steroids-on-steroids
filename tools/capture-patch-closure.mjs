// Actual pinned applier/seek functions with supplied public text, no application imports.
import fs from 'node:fs/promises';
import path from 'node:path';
import vm from 'node:vm';
import {createRequire} from 'node:module';
import {fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
import {execFileSync} from 'node:child_process';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'..');
const upstream=path.join(root,'upstream/chat-on-steroids'),commit='024731d8083d92c46e8248d64e7e0b36ab0a470f';
if(execFileSync('git',['rev-parse','HEAD'],{cwd:upstream,encoding:'utf8'}).trim()!==commit)throw Error('Upstream identity changed');
const require=createRequire(path.join(upstream,'package.json'));
const files=['errors.ts','seek-sequence.ts','text-file.ts','file-update.ts','hunk.ts','streaming-parser.ts','parser.ts'];
const sources=await Promise.all(files.map(async name=>[name,await fs.readFile(path.join(upstream,'src/main/codex/apply-patch',name),'utf8')]));
const code=sources.map(([,body])=>body.replace(/^import[\s\S]*?;\r?$/gm,'')).join('\n')+'\nglobalThis.fixture={deriveNewContentsFromChunks,seekSequence,StreamingPatchParser,parsePatchText};';
const js=await require('esbuild').transform(code,{loader:'ts',format:'cjs'});
const context=vm.createContext({module:{exports:{}},exports:{},localFs:{readFileText(){throw Error('Only supplied public text is allowed')}}});
vm.runInContext(js.code,context);const api=context.fixture;
const chunk=(oldLines,newLines,options={})=>({oldLines,newLines,changeContext:null,contextLineIndices:[],isEndOfFile:false,...options});
const examples=[
  ['one\r\ntwo\r\n',[chunk(['one','two'],['one','TWO'],{contextLineIndices:[[0,0]]})]],
  ['\ufeffone\r\ntwo\nthree',[chunk(['one','two','three'],['one','TWO','three'],{contextLineIndices:[[0,0],[2,2]]})]],
  ['one\rtwo\r',[chunk(['one'],['ONE'])]],
  ['', [chunk([],['new'])]],['one',[chunk([],['new'])]],['one\n\n',[chunk([],['new'])]],
  ['one',[chunk(['one'],['one'],{contextLineIndices:[[0,0]]})]],
  ['a\nb\n',[chunk(['b'],['B']),chunk(['a'],['A'],{isEndOfFile:true})]],
  ['repeated\nrepeated\n',[chunk(['repeated'],['first']),chunk(['repeated'],['last'])]],
  ['prefix\nunique public anchor\nactual line\nsuffix\n',[chunk(['unique public anchor','wrong line'],['new'])]],
  ['early unique block\nlate unique block\n',[chunk(['late unique block'],['LATE']),chunk(['early unique block'],['EARLY'])]],
  ['anchor\nactual\n',[chunk(['missing'],['new'],{changeContext:'anchor'})]],
  ['public\n',[chunk(['missing'],['new'],{changeContext:'missing context'})]],
  ['\ufeffpublic line\n',[chunk(['\ufeffpublic line'],['new'])]],
  ['public line\n',[chunk(['\ufeffpublic line'],['new'])]],
  ['public\u0085\n',[chunk(['public'],['new'])]],
  [Array.from({length:14},(_,i)=>'public source '+i).join('\n'),[chunk(Array.from({length:13},(_,i)=>i===3?'wrong line':'public source '+i),['replacement'])]],
  ['long public anchor '+('x'.repeat(300))+'\nactual\n',[chunk(['long public anchor '+('x'.repeat(300)),'wrong'],['new'])]]
];
const updates=[];
for(const mode of ['normalize_to_lf','preserve_line_endings'])for(const [contents,chunks] of examples){
  try{const expected=await api.deriveNewContentsFromChunks('public.txt',chunks,mode,contents);updates.push({contents,chunks,mode,expected:expected.newContents});}
  catch(error){updates.push({contents,chunks,mode,error:error.message,sourceContext:error.sourceContext??null});}
}
const seeks=[];
for(const mode of ['normalize_to_lf','preserve_line_endings'])for(const [lines,pattern,start,eof] of [
  [['a','b'],['a'],2,true],[['a','b'],['b'],5,true],[['public\ufeff'],['public'],0,false],
  [['public\u0085'],['public'],0,false],[['“public”—\u00a0'],['"public"- '],0,false],
  [['a','b'],[],5,false],[['a'],['a','b'],0,false]
])seeks.push({lines,pattern,start,eof,mode,expected:api.seekSequence(lines,pattern,start,eof,mode)});
const out=path.join(root,'fixtures/upstream/phase1-6-closure');
await fs.writeFile(path.join(out,'patch-updates.json'),JSON.stringify({updates,seeks,upstreamCommit:commit,sources:Object.fromEntries(sources.map(([name,body])=>[name,createHash('sha256').update(body).digest('hex')]))},null,2)+'\n');
console.log(JSON.stringify({updates:updates.length,seeks:seeks.length,upstreamCommit:commit}));
