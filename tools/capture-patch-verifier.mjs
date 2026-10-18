// Run the pinned dry-run verifier against public in-memory filesystem metadata/text.
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
const sources=await Promise.all(['errors.ts','seek-sequence.ts','text-file.ts','file-update.ts','hunk.ts','index.ts'].map(async name=>[name,await fs.readFile(path.join(upstream,'src/main/codex/apply-patch',name),'utf8')]));
const filesystem=await fs.readFile(path.join(upstream,'src/main/codex/filesystem.ts'),'utf8');
function declarations(body,names){
 const parsed=require('@babel/parser').parse(body,{sourceType:'module',plugins:['typescript']});
 const selected=parsed.program.body.map(s=>s.type==='ExportNamedDeclaration'?s.declaration:s).filter(s=>s&&((s.type==='FunctionDeclaration'&&names.includes(s.id.name))||(s.type==='VariableDeclaration'&&s.declarations.some(d=>names.includes(d.id.name)))));
 if(selected.length!==names.length)throw Error('Missing source declaration');
 return selected.map(s=>body.slice(s.start,s.end)).join('\n');
}
const index=sources.find(([name])=>name==='index.ts')[1];
const code=sources.filter(([name])=>name!=='index.ts').map(([,body])=>body.replace(/^import[\s\S]*?;\r?$/gm,'')).join('\n')+'\n'+declarations(filesystem,['invalidInput'])+'\n'+declarations(index,['MAX_PATCH_SOURCE_BYTES','defaultPathResolver','verifyPatchFileTarget','errorCode','sameResolvedPath','resolveHunkPath','readingWhatThePatchAlreadyRemoved','verifyApplyPatchArgs'])+'\nglobalThis.verify=verifyApplyPatchArgs;';
const js=await require('esbuild').transform(code,{loader:'ts',format:'cjs'});
const cases=[];
for(const platform of ['win32','linux'])for(const local of [true,false])for(const destination of ['a','A','b']){
 const pathApi=platform==='win32'&&local?path.win32:path.posix;
 const cwd=pathApi===path.win32?'C:\\public':'/public';
 const source=pathApi.join(cwd,'a');
 const key=p=>platform==='win32'&&local?p.toLowerCase():p;
 const missing=()=>Object.assign(new Error('missing public fixture'),{code:'ENOENT'});
 const backend={path:pathApi,async getMetadata(p){if(key(p)!==key(source))throw missing();return {isFile:true,size:7};},async readFileText(p){if(key(p)!==key(source))throw missing();return 'public\n';}};
 const localFs=local?backend:{path:platform==='win32'?path.win32:path.posix};
 const context=vm.createContext({module:{exports:{}},exports:{},nodePath:pathApi,process:{platform},localFs});vm.runInContext(js.code,context);
 const hunks=[{kind:'add_file',path:'earlier',contents:'must not be written\n'},{kind:'update_file',path:'a',movePath:destination,chunks:[{oldLines:['public'],newLines:['public'],changeContext:null,contextLineIndices:[[0,0]],isEndOfFile:false}]}];
 let expected;
 try{await context.verify({patch:'public fixture',hunks,workdir:null},cwd,'preserve_line_endings',(p,base)=>pathApi.resolve(base,p),backend);expected={allowed:true};}
 catch(error){expected={allowed:false,error:error.message,code:error.code??null};}
 cases.push({platform,local,destination,expected});
}
await fs.writeFile(path.join(root,'fixtures/upstream/phase1-6-closure/patch-verifier-paths.json'),JSON.stringify({upstreamCommit:commit,sources:Object.fromEntries([...sources,['filesystem.ts',filesystem]].map(([name,body])=>[name,createHash('sha256').update(body).digest('hex')])),cases},null,2)+'\n');
console.log(JSON.stringify({verifierCases:cases.length,actualFilesystemMutations:0}));
