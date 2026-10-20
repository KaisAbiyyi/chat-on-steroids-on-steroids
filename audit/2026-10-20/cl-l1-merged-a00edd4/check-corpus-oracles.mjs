import fs from 'node:fs/promises';
import path from 'node:path';
import {createRequire} from 'node:module';
import {execFileSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {isDeepStrictEqual} from 'node:util';
const run='D:/projects/CoS-Rust/.test-tmp/cl-l1-merged-20261020-a00edd4';
const repo='D:/projects/CoS-Rust/.worktrees/cl-l1-retest-6f4e6ac';
const upstream='D:/projects/CoS-Rust/upstream/chat-on-steroids';
const sha='a00edd4a67e6c8acc9fc45bee659094719d6673a';
const base='06f586e423e5abb4f95eba582996c4a5278c1311';
const pin='024731d8083d92c46e8248d64e7e0b36ab0a470f';
const git=(cwd,args)=>execFileSync('git',args,{cwd,encoding:'utf8'}).trim();
if(git(upstream,['rev-parse','HEAD'])!==pin||git(upstream,['status','--porcelain']))throw Error('Upstream identity/cleanliness failed');
if(git(repo,['rev-parse','HEAD'])!==sha||git(repo,['status','--porcelain']))throw Error('Candidate identity/cleanliness failed');
const oldPath='fixtures/upstream/phase1-6-closure/session-summaries.json';
const oldBytes=execFileSync('git',['show',`${sha}:${oldPath}`],{cwd:repo});
const baseBytes=execFileSync('git',['show',`${base}:${oldPath}`],{cwd:repo});
if(!oldBytes.equals(baseBytes))throw Error('Frozen corpus changed');
const old=JSON.parse(oldBytes);
const fresh=JSON.parse(await fs.readFile(path.join(repo,'fixtures/upstream/phase1-6-closure/cl-l1-captions.json'),'utf8'));
const hashes={};
for(const relative of Object.keys(old.sources)){
  const hash=createHash('sha256').update(await fs.readFile(path.join(upstream,relative))).digest('hex');
  if(hash!==old.sources[relative]||hash!==fresh.sources[relative])throw Error(`Source hash differs: ${relative}`);
  hashes[relative]=hash;
}
const require=createRequire(path.join(upstream,'package.json'));
async function load(relative,name){
 const dest=path.join(run,'corpus-oracle',name+'.cjs');
 await require('esbuild').build({entryPoints:[path.join(upstream,relative)],outfile:dest,bundle:true,platform:'node',format:'cjs',packages:'external',logLevel:'warning'});
 return require(dest);
}
const api=await load('src/main/session/summarize.ts','summarize');
const context=await load('src/main/mcp/call-context.ts','call-context');
const archived=JSON.parse(execFileSync('git',['show','12b2523f75799125ebfc714ba106e57048ef2226:audit/2026-10-20/cl-l1-windows-06f586e/native-runtime-projections.json'],{cwd:repo,encoding:'utf8'}));
const native=fresh.cases.filter(row=>row.group==='native');
if(native.length!==14||archived.length!==14||fresh.cases.length!==44)throw Error('Fixture cardinality changed');
for(let i=0;i<14;i++){
 const {call,case:name,stage}=archived[i];
 const row=native[i];
 const actual={name:row.name,tool:row.tool,args:row.args,evidence:row.evidence,outcome:row.outcome,durationMs:row.durationMs};
 const expected={name:`${name}/${stage}/${call.callId}`,tool:call.tool,args:JSON.parse(call.args.text),evidence:call.evidence,outcome:call.outcome,durationMs:call.durationMs};
 if(!isDeepStrictEqual(actual,expected))throw Error('Archived native input changed at '+i);
}
const compared=fresh.cases.map(row=>{
 const evidence={...context.emptyEvidence(),...row.evidence};
 const completed=api.summarizeToolCall({...row,evidence});
 const running=api.summarizeRunningCall(row.tool,row.args,evidence);
 return {name:row.name,group:row.group,pass:isDeepStrictEqual(completed,row.expected)&&isDeepStrictEqual(running,row.running),completed,running};
});
const result={status:compared.every(row=>row.pass)?'PASS':'FAIL',productSha:sha,upstreamCommit:pin,
  originalCorpusSha256:createHash('sha256').update(oldBytes).digest('hex'),originalCorpusUnchanged:true,
  pinnedSourceHashes:hashes,nativeInputAuthentication:'PASS',cases:compared.length,rows:compared};
await fs.writeFile(path.join(run,'additive-corpus-source-authentication.json'),JSON.stringify(result,null,2)+'\n');
console.log(JSON.stringify({status:result.status,cases:result.cases,originalCorpusUnchanged:true,nativeInputsAuthenticated:14,mismatches:compared.filter(row=>!row.pass)}));
process.exitCode=result.status==='PASS'?0:1;
