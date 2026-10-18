// Actual source search functions with a public injectable ProjectFs; no private paths.
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
const search=await fs.readFile(path.join(upstream,'src/main/search.ts'),'utf8');
const fsops=await fs.readFile(path.join(upstream,'src/main/fsops.ts'),'utf8');
function declarations(body,names){
 const parsed=require('@babel/parser').parse(body,{sourceType:'module',plugins:['typescript']});
 const selected=parsed.program.body.map(s=>s.type==='ExportNamedDeclaration'?s.declaration:s).filter(s=>s&&s.type==='FunctionDeclaration'&&names.includes(s.id.name));
 if(selected.length!==names.length)throw Error('Missing source declaration');
 return selected.map(s=>body.slice(s.start,s.end)).join('\n');
}
const code=search.replace(/^import[\s\S]*?;\r?$/gm,'')+'\n'+declarations(fsops,['isExcludedFolderName','sniffBinaryBytes'])+'\nglobalThis.fixture={search,searchOneFile,globToRegExp,compileIncludeMatcher,isExcludedFolderName,DEFAULT_EXCLUDES};';
const js=await require('esbuild').transform(code,{loader:'ts',format:'cjs'});
const context=vm.createContext({module:{exports:{}},exports:{},path:path.posix,Buffer,TextDecoder,locateRipgrep:()=>null});vm.runInContext(js.code,context);
const api=context.fixture,globs=[],excludes=[];
for(const platform of ['linux','win32']){
 context.path=platform==='win32'?path.win32:path.posix;
 for(const pattern of ['*.ts','**/*.ts','src/?a.*','[a].txt','foo{bar}','file?.txt','**','','a.ts','k.txt','s.txt','ß.txt','Σ.txt','i.txt'])for(const sensitive of [false,true])for(const file of ['src/a.ts','src/a.TS','a.ts','src/aa.rs','[a].txt','foob','foo{bar}','file😀.txt','filex.txt','','src\\a.ts','a.ts\n','a.ts\r','a.ts\u2028','K.txt','ſ.txt','ẞ.txt','ς.txt','ı.txt'])globs.push({platform,pattern,sensitive,file,expected:api.compileIncludeMatcher(pattern,sensitive)(file)});
}
context.path=path.posix;
for(const pattern of [['build'],['release-*'],['.claude-*'],['[cache]'],[],['A*B'],['äbuild'],['ß'],['ΑΣ']])for(const name of ['build','BUILD','builder','release-public','release','[cache]','cache','.claude-x','AXXB','a*b','Äbuild','ẞ','Ας'])excludes.push({patterns:pattern,name,expected:api.isExcludedFolderName(name,pattern)});
function backend(files,dirs){return {path:path.posix,
 async getMetadata(file){if(!files.has(file))throw Error('ENOENT');return {size:files.get(file).length,isFile:true,isDirectory:false};},
 async readDirectory(dir){if(!(dir in dirs))throw Error('ENOENT');return dirs[dir].map(([name,type])=>({fileName:name,isFile:type==='file',isDirectory:type==='directory'}));},
 async *readFileStream(file){yield files.get(file);}
};}
const scans=[];
for(const [text,encoding] of [['  needle  \r\nlast needle\r','utf8'],['\ufeffneedle\0\n','utf8'],['needle\n','utf16le'],['needle\n','utf16be'],['needle '+('😀'.repeat(160)),'utf8'],['needle\u0085','utf8'],['needle\n\n','utf8'],['','utf8'],['\x01\x02needle','utf8']]){
 let bytes=Buffer.from(text,encoding==='utf16le'||encoding==='utf16be'?'utf16le':'utf8');
 if(encoding==='utf16le')bytes=Buffer.concat([Buffer.from([255,254]),bytes]);
 if(encoding==='utf16be'){bytes.swap16();bytes=Buffer.concat([Buffer.from([254,255]),bytes]);}
 const files=new Map([['/public/a.txt',bytes]]);
 const req={query:'needle',mode:'content',caseSensitive:false,regex:false,maxResults:50,server:backend(files,{})};
 const expected=await api.searchOneFile('/public/a.txt','/p/a.txt',req);delete expected.elapsedMs;
 scans.push({bytes:bytes.toString('base64'),query:req.query,expected});
}
for(const [bytes,req] of [[Buffer.from([110,101,101,100,108,101,255]),{}],[Buffer.from('needle\nneedle\n'),{maxResults:1}],[Buffer.alloc(2*1024*1024+1),{}]]){
 const options={query:'needle',mode:'content',caseSensitive:false,regex:false,maxResults:50,...req};
 const expected=await api.searchOneFile('/public/a.txt','/p/a.txt',{...options,server:backend(new Map([['/public/a.txt',bytes]]),{})});delete expected.elapsedMs;
 // Huge byte fixtures are described, not expanded into multi-megabyte JSON.
 scans.push({...(bytes.length>1024?{zeroBytes:bytes.length}:{bytes:bytes.toString('base64')}),query:options.query,maxResults:options.maxResults,expected});
}
const directories={'/public':[['deep','directory'],['z.txt','file'],['BUILD','directory'],['link','link'],['a.TS','file']],'/public/deep':[['inner','directory'],['needle.txt','file']],'/public/deep/inner':[['needle.rs','file']],'/public/BUILD':[['needle.js','file']]};
const files={'/public/z.txt':'root needle\n','/public/a.TS':'Alpha needle\n','/public/deep/needle.txt':'deep needle\n','/public/deep/inner/needle.rs':'last needle\n','/public/BUILD/needle.js':'hidden needle\n'};
const trees=[];
for(const options of [{query:'needle'},{query:'deep'},{query:'needle',maxResults:1},{query:'needle',exclude:[]},{query:'a',include:'*.ts'},{query:'a',include:'*.ts',caseSensitive:true}]){
 const req={realDir:'/public',virtualDir:'/p',mode:'name',caseSensitive:false,maxResults:50,exclude:api.DEFAULT_EXCLUDES,...options,server:backend(new Map(Object.entries(files).map(([p,t])=>[p,Buffer.from(t)])),directories)};
 const expected=await api.search(req);delete expected.elapsedMs;
 trees.push({options,expected});
}
await fs.writeFile(path.join(root,'fixtures/upstream/phase1-6-closure/search.json'),JSON.stringify({upstreamCommit:commit,sources:{'search.ts':createHash('sha256').update(search).digest('hex'),'fsops.ts':createHash('sha256').update(fsops).digest('hex')},defaultExcludes:api.DEFAULT_EXCLUDES,globs,excludes,scans,trees,directories,files},null,2)+'\n');
await fs.writeFile(path.join(root,'fixtures/upstream/phase1-6-closure/search-globs.json'),JSON.stringify({upstreamCommit:commit,sources:{'search.ts':createHash('sha256').update(search).digest('hex'),'fsops.ts':createHash('sha256').update(fsops).digest('hex')},defaultExcludes:api.DEFAULT_EXCLUDES,globs,excludes},null,2)+'\n');
console.log(JSON.stringify({globs:globs.length,excludes:excludes.length,scans:scans.length,trees:trees.length}));
