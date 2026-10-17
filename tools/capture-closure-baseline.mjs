// Executes only declaration-scoped upstream helpers, never the app bootstrap.
import fs from 'node:fs/promises';
import path from 'node:path';
import vm from 'node:vm';
import {createRequire} from 'node:module';
import {fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
import {execFileSync} from 'node:child_process';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'..');
const upstream=path.join(root,'upstream/chat-on-steroids');
const commit='024731d8083d92c46e8248d64e7e0b36ab0a470f';
if(execFileSync('git',['rev-parse','HEAD'],{cwd:upstream,encoding:'utf8'}).trim()!==commit) throw Error('Upstream identity changed');
const require=createRequire(path.join(upstream,'package.json'));
const esbuild=require('esbuild');
const relative='src/main/project-git.ts', source=await fs.readFile(path.join(upstream,relative),'utf8');
const declarations=source.replace(/^import[\s\S]*?;\r?$/gm,'');
const js=await esbuild.transform(declarations+'\nglobalThis.fixture={parseStatus,parseCommittedChanges,parseNumstat,safeRelative,textFromBytes};',{loader:'ts',format:'cjs'});
const context=vm.createContext({module:{exports:{}},exports:{},Buffer,TextDecoder,path,process:{platform:'win32',env:{}},setTimeout,clearTimeout});
vm.runInContext(js.code,context);
const api=context.fixture, cases=[];
for(const prefix of ['', 'sub']) for(const raw of ['?? a\0 M sub/a\0','R  new\0old\0','!! ignored\0 M ../escape\0',' M sub/a\0?? sub/nested/a\0',' M sub/space name\0']) {
  cases.push({kind:'status',prefix,raw,expected:api.parseStatus(Buffer.from(raw),prefix)});
}
for(const prefix of ['', 'sub']) for(const raw of ['M\0a\0A\0sub/new\0','R100\0sub/old\0new\0','R100\0old\0sub/new\0','D\0sub/deleted\0']) {
  cases.push({kind:'committed',prefix,raw,expected:api.parseCommittedChanges(Buffer.from(raw),prefix)});
}
for(const prefix of ['', 'sub']) for(const raw of ['1\t2\ta\0-\t-\tsub/binary\0','0\t0\t\0sub/old\0sub/new\0','3\t0\tsub/space\tname\0']) {
  cases.push({kind:'numstat',prefix,raw,expected:Object.fromEntries(api.parseNumstat(Buffer.from(raw),prefix))});
}
for(const value of ['','a','a/b','a\\b','../x','a/../x','./a','a//b','C:/x','/absolute','é/😀']) cases.push({kind:'relative',value,expected:api.safeRelative(value)});
const out=path.join(root,'fixtures/upstream/phase1-6-closure'); await fs.mkdir(out,{recursive:true});
await fs.writeFile(path.join(out,'git.json'),JSON.stringify(cases,null,2)+'\n');
await fs.writeFile(path.join(out,'manifest.json'),JSON.stringify({upstreamCommit:commit,sources:{[relative]:createHash('sha256').update(source).digest('hex')},gitCases:cases.length,execution:'Declaration-only VM; no handlers, app/profile imports or network calls'},null,2)+'\n');
console.log(JSON.stringify({gitCases:cases.length,upstreamCommit:commit}));
