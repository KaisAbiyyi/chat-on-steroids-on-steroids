// Pure source calls against the source's injectable filesystem probe, with public paths.
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
const sources=await Promise.all(['env.ts','toolchain.ts'].map(async name=>[name,await fs.readFile(path.join(upstream,'src/main',name),'utf8')]));
const code=sources.map(([,body])=>body.replace(/^import[\s\S]*?;\r?$/gm,'')).join('\n')+'\nglobalThis.fixture={ensureDevToolchain,resetToolchainCache,byVersionDescending};';
const js=await require('esbuild').transform(code,{loader:'ts',format:'cjs'});
const p=path.win32;
const base={'ProgramFiles':'C:\\public\\Program Files','ProgramFiles(x86)':'C:\\public\\Program Files x86','LOCALAPPDATA':'C:\\public\\Local','Path':'C:\\public\\path'};
const jbr=p.join(base.ProgramFiles,'Android','Android Studio','jbr');
const jre=p.join(base.ProgramFiles,'Android','Android Studio','jre');
const java=p.join(base.ProgramFiles,'Java');
const go=p.join(base.ProgramFiles,'Go');
const scenarios=[
 {files:[p.join(jbr,'bin','javac.exe'),p.join(go,'bin','go.exe')],steps:[{},{}]},
 {files:[p.join(jre,'bin','java.exe'),p.join(java,'jdk-21','bin','javac.exe')],directories:{[java]:[p.join(java,'jdk-9'),p.join(java,'jre1.8.0_411'),p.join(java,'jdk-21')]},steps:[{}]},
 {files:[p.join(java,'jdk-21.0.12','bin','javac.exe'),p.join(java,'jdk-21.0.5','bin','javac.exe')],directories:{[java]:[p.join(java,'jdk-21.0.5'),p.join(java,'jdk-21.0.12')]},steps:[{}]},
 {files:[p.join(jbr,'bin','javac.exe'),p.join(go,'bin','go.exe')],steps:[{JAVA_HOME:'',GOROOT:''},{java_home:'C:\\chosen\\jdk',goroot:'C:\\chosen\\go'}]},
 {files:[p.join(base.Path,'java.exe'),p.join(base.Path,'go.exe'),p.join(jbr,'bin','javac.exe')],steps:[{}]},
 {files:[p.join(jbr,'bin','javac.exe'),p.join(go,'bin','go.exe')],steps:[{Path:' "C:\\public\\path" ; ; C:\\PUBLIC\\PROGRAM FILES\\GO\\bin '}]},
 {files:[],steps:[{},{}]},
 {files:[p.join(jbr,'bin','javac.exe')],steps:[{GOROOT:'chosen'}, {ProgramFiles:'C:\\different',GOROOT:'chosen'}]},
 {files:[p.join(jbr,'bin','javac.exe')],steps:Array.from({length:18},(_,i)=>({Path:`C:\\public\\path${i}`,GOROOT:'chosen'})).concat([{Path:'C:\\public\\path0',GOROOT:'chosen'}])},
 {files:[p.join('C:','Android','Android Studio','jbr','bin','javac.exe')],steps:[{ProgramFiles:'C:',GOROOT:'chosen'}]},
 {files:[p.join('\\\\public-server\\share','Android','Android Studio','jbr','bin','javac.exe')],steps:[{ProgramFiles:'\\\\public-server\\share',GOROOT:'chosen'}]},
 {files:[p.join('C:\\public\\a\\..\\b','Android','Android Studio','jbr','bin','javac.exe')],steps:[{ProgramFiles:'C:\\public\\a\\..\\b',GOROOT:'chosen'}]}
];
const output=[];
for(const scenario of scenarios){
 const context=vm.createContext({module:{exports:{}},exports:{},process:{platform:'win32'},path:p});vm.runInContext(js.code,context);
 const calls=[];
 const probe={isFile(target){calls.push(['isFile',target]);return scenario.files.includes(target);},directories(target){calls.push(['directories',target]);return [...(scenario.directories?.[target]??[])];}};
 const steps=[];
 for(const override of scenario.steps){
  const env={...base,...override};const before=structuredClone(env);calls.length=0;
  const added=context.fixture.ensureDevToolchain(env,probe);
  steps.push({env:before,expected:{env,added},calls:structuredClone(calls)});
 }
 output.push({files:scenario.files,directories:scenario.directories??{},steps});
}
const context=vm.createContext({module:{exports:{}},exports:{},process:{platform:'win32'},path:p});vm.runInContext(js.code,context);
const versions=[];
for(const names of [['jdk-9','jdk-21','jdk-21.0.5','jdk-21.0.12'],['JDK-21','jdk-21','current','jbr'],['jdk-9007199254740993','jdk-9007199254740992'],['jdk-'+('9'.repeat(400)),'jdk-'+('8'.repeat(400))],['jdk-😀','jdk-\ue000','jdk-Ä','jdk-a']])versions.push({names,expected:[...names].sort(context.fixture.byVersionDescending)});
await fs.writeFile(path.join(root,'fixtures/upstream/phase1-6-closure/toolchain.json'),JSON.stringify({upstreamCommit:commit,sources:Object.fromEntries(sources.map(([name,body])=>[name,createHash('sha256').update(body).digest('hex')])),scenarios:output,versions},null,2)+'\n');
console.log(JSON.stringify({scenarios:output.length,steps:output.reduce((n,s)=>n+s.steps.length,0),versions:versions.length}));
