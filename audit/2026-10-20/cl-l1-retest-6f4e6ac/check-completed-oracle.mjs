import fs from 'node:fs/promises';
import path from 'node:path';
import {createRequire} from 'node:module';
import {execFileSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {isDeepStrictEqual} from 'node:util';
const run = 'D:/projects/CoS-Rust/.test-tmp/cl-l1-retest-20261020-01';
const repo = 'D:/projects/CoS-Rust/.worktrees/cl-l1-retest-6f4e6ac';
const upstream = 'D:/projects/CoS-Rust/upstream/chat-on-steroids';
const commit = '024731d8083d92c46e8248d64e7e0b36ab0a470f';
if (execFileSync('git',['rev-parse','HEAD'],{cwd:upstream,encoding:'utf8'}).trim()!==commit) throw Error('Upstream mismatch');
const require=createRequire(path.join(upstream,'package.json'));
const source=path.join(upstream,'src/main/session/summarize.ts');
const built=path.join(run,'oracle/summarize.cjs');
await require('esbuild').build({entryPoints:[source],outfile:built,bundle:true,platform:'node',format:'cjs',packages:'external',logLevel:'warning'});
const api=require(built);
const corpus=JSON.parse(await fs.readFile(path.join(repo,'fixtures/upstream/phase1-6-closure/session-summaries.json'),'utf8'));
const rows=JSON.parse(await fs.readFile(path.join(run,'native-runtime-projections.json'),'utf8'));
const compared=rows.map(row=>{
  const call=row.call;
  const args=JSON.parse(call.args.text);
  const input={tool:call.tool,args,evidence:{...corpus.emptyEvidence,...call.evidence},outcome:call.outcome,durationMs:call.durationMs,resultHead:''};
  const expected=api.summarizeToolCall(input);
  const expectedRunning=api.summarizeRunningCall(call.tool,args,input.evidence);
  return {case:row.case,stage:row.stage,callId:call.callId,tool:call.tool,args,evidence:call.evidence,
    actual:call.summary,expected,actualRunning:call.runningSummary,expectedRunning,
    pass:isDeepStrictEqual(call.summary,expected)&&isDeepStrictEqual(call.runningSummary,expectedRunning)};
});
const result={upstreamCommit:commit,sourceSha256:createHash('sha256').update(await fs.readFile(source)).digest('hex'),
  sourceFile:'src/main/session/summarize.ts',sourceUnchanged:execFileSync('git',['status','--porcelain'],{cwd:upstream,encoding:'utf8'}).trim()==='',
  productSha:'6f4e6acc3594edc2759c8cb1dbefe551cb51e132',
  existingCorpus:{total:corpus.cases.length,successfulWriteStdinCases:corpus.cases.filter(row=>row.tool==='write_stdin'&&row.evidence.exitCode===0).length},
  status:compared.every(row=>row.pass)?'PASS':'FAIL',rows:compared};
await fs.writeFile(path.join(run,'completed-source-oracle.json'),JSON.stringify(result,null,2)+'\n');
console.log(JSON.stringify({status:result.status,compared:compared.length,mismatches:compared.filter(x=>!x.pass).map(x=>({case:x.case,stage:x.stage,tool:x.tool,actual:x.actual,expected:x.expected}))}));
process.exitCode=result.status==='PASS'?0:1;
