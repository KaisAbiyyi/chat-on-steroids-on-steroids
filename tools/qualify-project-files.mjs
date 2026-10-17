// Compare the native executable against the pinned source's exact text snapshot.
import fs from 'node:fs/promises';
import {createReadStream} from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import vm from 'node:vm';
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {createRequire} from 'node:module';
import {fileURLToPath} from 'node:url';
import {execFileSync} from 'node:child_process';
const repo=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'..');
const upstream=path.join(repo,'upstream/chat-on-steroids');
const commit='024731d8083d92c46e8248d64e7e0b36ab0a470f';
assert.equal(execFileSync('git',['rev-parse','HEAD'],{cwd:upstream,encoding:'utf8'}).trim(),commit);
const require=createRequire(path.join(upstream,'package.json'));
const source=await fs.readFile(path.join(upstream,'src/main/project-files.ts'),'utf8');
function declarations(body,names){
  const parsed=require('@babel/parser').parse(body,{sourceType:'module',plugins:['typescript']});
  const selected=parsed.program.body.map(s=>s.type==='ExportNamedDeclaration'?s.declaration:s).filter(s=>s&&((['FunctionDeclaration','ClassDeclaration'].includes(s.type)&&names.includes(s.id.name))||(s.type==='VariableDeclaration'&&s.declarations.some(d=>names.includes(d.id.name)))));
  assert.equal(selected.length,names.length,names.join(','));
  return selected.map(s=>body.slice(s.start,s.end)).join('\n');
}
const inputs=[
  ['src/main/project-files.ts',['sameFile','textSnapshot','previewProjectFile','MAX_PREVIEW_BYTES','MAX_IMAGE_PREVIEW_BYTES','MAX_PDF_PREVIEW_BYTES','IMAGE_MIME_BY_EXTENSION']],
  ['src/main/fsops.ts',['sniffBinaryBytes','FsOpError','formatBytes','clamp','DEFAULT_READ_BYTES','MAX_READ_BYTES']],
  ['src/main/codex/read-backend.ts',['statInfo','scanFile','readTextFile','detectTextFormat','stripBom','BinaryReadError','MAX_LINE_COUNT_BYTES','MAX_DECODED_LINE_CHARS','BINARY_SNIFF_BYTES']],
  ['src/main/codex/filesystem.ts',['getMetadata','systemTimeToUnixMs']]
];
const bodies=await Promise.all(inputs.map(async([file,names])=>declarations(await fs.readFile(path.join(upstream,file),'utf8'),names)));
const js=await require('esbuild').transform(bodies.join('\n')+'\nglobalThis.snapshot=textSnapshot;globalThis.preview=previewProjectFile;globalThis.metadata=getMetadata;',{loader:'ts',format:'cjs'});
const localFs={getMetadata:async(file)=>context.metadata(file),readFileStream:(file)=>createReadStream(file,{highWaterMark:1024*1024})};
const context=vm.createContext({module:{exports:{}},exports:{},fs,rawPromises:fs,localFs,Buffer,TextDecoder,createHash,path,sharp:require('sharp'),recheckTarget:async()=>{},serverTarget:async()=>null,projectFileTarget:async(_id,relative)=>({projectName:'project',path:relative,real:path.join(project,relative)})});
vm.runInContext(js.code,context);
const binary=path.resolve(process.argv[2]??path.join(repo,'target/debug/cos-rust'+(process.platform==='win32'?'.exe':'')));
const folder=await fs.mkdtemp(path.join(os.tmpdir(),'cos-rust-public-file-panel-'));
const profile=path.join(folder,'profile'),project=path.join(folder,'project');
await fs.mkdir(project);
function run(...args){return JSON.parse(execFileSync(binary,['--data-dir',profile,...args],{encoding:'utf8'}));}
const checks=[];
try {
  run('info');run('config','set','roots',JSON.stringify([{name:'p',path:project}]));
  const {id}=run('projects','add','/p');
  for(const [name,bytes] of [['utf8.txt',Buffer.from('é 😀 public\n')],['bom-crlf.txt',Buffer.from('\ufeffpublic\r\nlast')],['empty.txt',Buffer.alloc(0)]]){
    const real=path.join(project,name);await fs.writeFile(real,bytes);
    const expected=await context.snapshot({real});
    const actual=run('projects','preview',id,name);
    const after=await context.snapshot({real});
    assert.equal(after.revision,expected.revision,'Public fixture changed during comparison');
    if(actual.revision!==expected.revision) console.error(JSON.stringify({sourceStamp:[expected.stat.dev,expected.stat.ino,expected.stat.size,expected.stat.mtimeMs,expected.stat.ctimeMs],sourceStampJson:JSON.stringify([expected.stat.dev,expected.stat.ino,expected.stat.size,expected.stat.mtimeMs,expected.stat.ctimeMs])}));
    assert.deepEqual({revision:actual.revision,text:actual.text,bytes:actual.bytes,modifiedAt:actual.modifiedAt},{revision:expected.revision,text:expected.text,bytes:expected.data.length,modifiedAt:expected.stat.mtime.toISOString()},name);
    const edit=path.join(folder,'edit.json');
    await fs.writeFile(edit,JSON.stringify({text:'saved\r\n',expectedRevision:expected.revision,expectedBytes:expected.data.length,expectedModifiedAt:expected.stat.mtime.toISOString()}));
    run('projects','save-file',id,name,edit);
    assert.equal(await fs.readFile(real,'utf8'),'saved\r\n');
    checks.push({name,status:'PASS',checks:['source-byte/text/mtime/revision equality','save using actual source revision']});
  }
  for(const [name,bytes] of [['invalid.txt',Buffer.from([0xC0,0xAF])],['binary.dat',Buffer.from([1,2,0,4])],['control.txt',Buffer.from([65,1,66])],['utf16.txt',Buffer.from([255,254,65,0])],['fake-large.png',Buffer.from('public line\n'.repeat(450_000))],['large.txt',Buffer.from('public line\r\n'.repeat(30_000))],['first-line.txt',Buffer.alloc(300_000,120)],['document.pdf',Buffer.from('%PDF-1.7\npublic fixture\n%%EOF')],['invalid.pdf',Buffer.from('not PDF')],['invalid.png',Buffer.from('not PNG')]]){
    await fs.writeFile(path.join(project,name),bytes);
    await fs.utimes(path.join(project,name),1700000000.1236,1700000000.1236);
    const expected=JSON.parse(JSON.stringify(await context.preview(id,name)));
    assert.deepEqual(run('projects','preview',id,name),expected,name);
    checks.push({name,status:'PASS',checks:['entire preview projection equals actual source']});
  }
  const oracleSources=Object.fromEntries(await Promise.all(inputs.map(async([file])=>[file,createHash('sha256').update(await fs.readFile(path.join(upstream,file))).digest('hex')])));
  const receipt={status:'PASS',upstreamCommit:commit,sourceSha256:createHash('sha256').update(source).digest('hex'),oracleSources,binarySha256:createHash('sha256').update(await fs.readFile(binary)).digest('hex'),platform:process.platform,node:process.version,checks,scope:'Pinned declaration-only textSnapshot and previewProjectFile on actual public files versus the Rust host CLI. Native metadata and stream providers; no app bootstrap/profile imports. Raster encoders, all locales and filesystem races are separate gates.'};
  await fs.writeFile(path.join(repo,'audit/2026-10-17/project-file-snapshot-qualification.json'),JSON.stringify(receipt,null,2)+'\n');
  console.log(JSON.stringify(receipt,null,2));
} finally {
  assert.equal(path.dirname(folder),os.tmpdir());
  assert.ok(path.basename(folder).startsWith('cos-rust-public-file-panel-'));
  await fs.rm(folder,{recursive:true,force:true});
}
