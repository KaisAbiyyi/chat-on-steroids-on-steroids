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
const projectsRelative='src/main/projects.ts',projectsSource=await fs.readFile(path.join(upstream,projectsRelative),'utf8');
let catalog=[],at=4_000_000_000_000,uuidCounter=100;
const projectContext=vm.createContext({module:{exports:{}},exports:{},path,z:require('zod').z,
  PROJECT_COLORS:['blue','green','amber','purple','rose','teal'],CHATGPT_PROJECT_ID:/^g-p-[0-9a-f]{32}$/,
  Date:{now:()=>at},randomUUID:()=>`00000000-0000-4000-8000-${String(++uuidCounter).padStart(12,'0')}`,
  readDurable:async()=>structuredClone(catalog),writeDurableNow:async(_key,value)=>{catalog=structuredClone(value)},
  setTimeout:()=>({unref(){}}),clearTimeout(){}});
const projectJs=await esbuild.transform(projectsSource.replace(/^import[\s\S]*?;\r?$/gm,'')+'\nglobalThis.fixture={setProjectColor,removeProject,pendingProjectCreates,claimProjectCreate,armProjectCreate,completeProjectCreate,failProjectCreate,retryProjectCreate,markProjectGone,listProjects};',{loader:'ts',format:'cjs'});
vm.runInContext(projectJs.code,projectContext);
const projectApi=projectContext.fixture,projectId='11111111-1111-4111-8111-111111111111',requestId='22222222-2222-4222-8222-222222222222';
const initial={id:projectId,name:'Public project',path:'/public/project',createdAt:at,chatgpt:{state:'requested',requestId,updatedAt:at}};
const normalize=value=>JSON.parse(JSON.stringify(value).replace(/00000000-0000-4000-8000-\d{12}/g,'<new-uuid>'));
const scenarios=[];
for(const operations of [
  [['setProjectColor',[projectId,'teal'],0],['setProjectColor',[projectId,null],1],['removeProject',[projectId],2],['pendingProjectCreates',[],3]],
  [['claimProjectCreate',[requestId,'a'],0],['claimProjectCreate',[requestId,'b'],59999],['armProjectCreate',[requestId,'a'],60000],['pendingProjectCreates',[],60000],['claimProjectCreate',[requestId,'b'],60000],['armProjectCreate',[requestId,'b'],60001],['completeProjectCreate',[requestId,'a','g-p-0123456789abcdef0123456789abcdef'],60002],['pendingProjectCreates',[],120001],['retryProjectCreate',[projectId],120002]],
  [['claimProjectCreate',[requestId,'a'],0],['failProjectCreate',[requestId,'a','public failure'],1],['retryProjectCreate',[projectId],2]],
  [['claimProjectCreate',[requestId,'a'],0],['armProjectCreate',[requestId,'a'],1],['failProjectCreate',[requestId,'a','public failure'],2]],
  [['claimProjectCreate',[requestId,'a'],0],['armProjectCreate',[requestId,'a'],1],['completeProjectCreate',[requestId,'b','g-p-0123456789abcdef0123456789abcdef'],2],['completeProjectCreate',[requestId,'a','invalid-id'],3],['completeProjectCreate',[requestId,'a','g-p-0123456789abcdef0123456789abcdef'],4],['retryProjectCreate',[projectId],5],['markProjectGone',[projectId,'g-p-ffffffffffffffffffffffffffffffff'],6],['markProjectGone',[projectId,'g-p-0123456789abcdef0123456789abcdef'],7]]
]) {
  catalog=[structuredClone(initial)]; const steps=[];
  for(const [method,args,offset] of operations) {
    at=initial.createdAt+offset;
    const expected=await projectApi[method](...args);
    steps.push({method,args,at,expected:normalize(expected),catalog:normalize(catalog)});
  }
  scenarios.push({initial:[initial],steps});
}
await fs.writeFile(path.join(out,'project-links.json'),JSON.stringify(scenarios,null,2)+'\n');
await fs.writeFile(path.join(out,'manifest.json'),JSON.stringify({upstreamCommit:commit,sources:{[relative]:createHash('sha256').update(source).digest('hex'),[projectsRelative]:createHash('sha256').update(projectsSource).digest('hex')},gitCases:cases.length,projectLinkSteps:scenarios.reduce((n,s)=>n+s.steps.length,0),execution:'Declaration-only VM; synthetic in-memory durable owner and clock for project leases; no app/profile imports, handlers or network calls'},null,2)+'\n');
console.log(JSON.stringify({gitCases:cases.length,projectLinkScenarios:scenarios.length,upstreamCommit:commit}));
