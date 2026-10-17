import fs from 'node:fs/promises';
import path from 'node:path';
import os from 'node:os';
import { createRequire } from 'node:module';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'..');
const upstream=path.join(root,'upstream/chat-on-steroids');
const commit='024731d8083d92c46e8248d64e7e0b36ab0a470f';
if(execFileSync('git',['rev-parse','HEAD'],{cwd:upstream,encoding:'utf8'}).trim()!==commit)throw Error('Baseline changed');
const require=createRequire(path.join(upstream,'package.json')),esbuild=require('esbuild');
const cache=path.join(upstream,'node_modules/.cache/cos-rust-oracle'),out=path.join(root,'fixtures/upstream');
const config=require(path.join(cache,'config.cjs'));
const fixtureConfig=config.defaultConfig('win32');
const stubs={name:'isolated-fixtures',setup(build){build.onResolve({filter:/(?:^|\/)config\.js$/},()=>({path:'config',namespace:'fixture'}));build.onResolve({filter:/(?:^|\/)logger\.js$/},()=>({path:'logger',namespace:'fixture'}));build.onLoad({filter:/.*/,namespace:'fixture'},args=>({contents:args.path==='config'?`export const getConfig=()=>(${JSON.stringify(fixtureConfig)});`:'export const logInfo=()=>{};export const logWarn=()=>{};export const logError=()=>{};',loader:'js'}));}};
async function load(rel,name,plugins=[]){const file=path.join(cache,name+'.cjs');await esbuild.build({entryPoints:[path.join(upstream,rel)],outfile:file,bundle:true,platform:'node',format:'cjs',packages:'external',plugins,logLevel:'warning'});return require(file);}
const store=await load('src/main/session/store.ts','store',[stubs]);
const session=await load('src/shared/session.ts','session');
const chronology=await load('src/shared/chronology.ts','chronology');
const usage=await load('src/shared/usage.ts','usage');
const provider=await load('src/shared/provider-usage.ts','provider-usage');
const markdown=await load('src/shared/markdown-export.ts','markdown-export');
const search=await load('src/main/session/search.ts','search',[stubs]);
const temp=await fs.mkdtemp(path.join(os.tmpdir(),'cos-rust-history-oracle-'));
const nativeNow=Date.now;Date.now=()=>1792108800000;
const stored=text=>({text,truncated:false,chars:text.length});
const id='2026-10-16-baseline',conversationId='11111111-1111-4111-8111-111111111111';
const operations=[
 ['create',{reservedId:id,title:'Public baseline',conversationId}],
 ['append',{time:1000,source:'extension',kind:'turn_start',turnId:'turn-a'}],
 ['upsert',{time:1001,source:'extension',kind:'user_message',messageId:'user-a',message:stored('Hello 😀'),conversationId,turnId:'turn-a'}],
 ['upsert',{time:1002,source:'extension',kind:'assistant_message',messageId:'assistant-a',providerMessageId:'provider-a',message:stored('work'),conversationId,turnId:'turn-a',state:'streaming'}],
 ['append',{time:1003,source:'mcp',kind:'tool_call',turnId:'turn-a',call:{callId:'call-a',tool:'read',requestId:'request-a',conversationId,attribution:'request-id',startedAt:1003,durationMs:4,args:stored('{"path":"/project/a"}'),result:stored('public result'),outcome:'ok',summary:{title:'Read a'}}}],
 ['upsert',{time:1008,source:'extension',kind:'assistant_message',messageId:'assistant-a',providerMessageId:'provider-a',message:stored('finished'),conversationId,turnId:'turn-a',state:'final',final:true}],
 ['upsert',{time:1009,source:'extension',kind:'assistant_message',messageId:'remounted-a',providerMessageId:'provider-a',message:stored('stale'),conversationId,turnId:'turn-a',state:'streaming'}],
 ['append',{time:1010,source:'extension',kind:'turn_end',turnId:'turn-a',outcome:'completed'}],
 ['asset',{text:'Public overflow text 😀',mimeType:'text/plain'}]
];
const results=[];
try{store.initSessionStore(temp);for(const [op,input]of operations){const value=op==='create'?await store.createSession(input):op==='append'?await store.appendEvent(id,input):op==='upsert'?await store.upsertMessageEvent(id,input):await store.writeAsset(id,Buffer.from(input.text),input.mimeType);results.push(value??null);}await store.flushSessions();
 const events=await store.readEvents(id),recent=await store.readRecentEvents(id,3);
 const summary=await store.getSession(id);const tree={};async function walk(dir,rel=''){for(const entry of await fs.readdir(dir,{withFileTypes:true})){const name=path.posix.join(rel,entry.name);if(entry.isDirectory())await walk(path.join(dir,entry.name),name);else tree[name]=(await fs.readFile(path.join(dir,entry.name))).toString('base64');}}await walk(path.join(temp,'sessions',id));
 await fs.writeFile(path.join(out,'history-case.json'),JSON.stringify({commit,id,operations,results,events,recent,summary,files:tree},null,2)+'\n');
 const rebind=await store.rebindSession(id,conversationId,'22222222-2222-4222-8222-222222222222');await store.flushSessions();await fs.writeFile(path.join(out,'rebind-case.json'),JSON.stringify({rebind,summary:await store.getSession(id)},null,2)+'\n');
 const vectors=[{name:'late-tool',entries:[{seq:1,time:10,kind:'turn_start',turnId:'a'},{seq:2,time:15,kind:'assistant_message',turnId:'a'},{seq:3,time:20,kind:'turn_end',turnId:'a'},{seq:4,time:12,kind:'tool_call',turnId:'a',call:{callId:'c'}}]},{name:'unowned-reload',entries:[{seq:1,time:200,kind:'assistant_message'},{seq:2,time:50,kind:'user_message'}]},{name:'two-turns',entries:[{seq:1,time:100,kind:'turn_start',turnId:'a'},{seq:2,time:90,kind:'assistant_message',turnId:'a'},{seq:3,time:110,kind:'turn_end',turnId:'a'},{seq:4,time:120,kind:'turn_start',turnId:'b'},{seq:5,time:125,kind:'assistant_message',turnId:'b'},{seq:6,time:95,kind:'tool_call',turnId:'a'}]}];
 await fs.writeFile(path.join(out,'chronology-cases.json'),JSON.stringify(vectors.map(v=>({...v,expected:chronology.chronological(v.entries)})),null,2)+'\n');
 const tokenCases=events.concat([{kind:'user_message',message:{text:'clip',chars:40000,truncated:true},wireTokenEstimate:14000},{kind:'tool_call',call:{nested:true,args:stored('abc'),result:stored('abc'),summary:{title:'x'}}}]).map(event=>({event,expected:session.eventTokens(event)}));
 const models=['gpt-6','gpt-6-thinking','gpt-6-sol','gpt-5-6-pro','unknown',' Astra ','sol'];
 await fs.writeFile(path.join(out,'usage-cases.json'),JSON.stringify({tokens:tokenCases,families:models.map(model=>({model,expected:usage.usageMessageFamily(model)})),formula:usage.DEFAULT_USAGE_FORMULA},null,2)+'\n');
 await fs.writeFile(path.join(out,'markdown-case.json'),JSON.stringify({expected:markdown.sessionMarkdown(summary.title,markdown.transcriptEntries(events).map(entry=>({role:entry.role,text:entry.stored.text})))},null,2)+'\n');
 const foldTexts=['École café','İSTANBUL','STRASSE ß ẞ','😀 Alpha','e\u0301 A'];await fs.writeFile(path.join(out,'search-cases.json'),JSON.stringify(foldTexts.map(text=>({text,expected:search.foldCase(text)})),null,2)+'\n');
 const codexLines=[{input_tokens:100,cached_input_tokens:40,output_tokens:10},{input_tokens:150,cached_input_tokens:60,output_tokens:20},{input_tokens:5,cached_input_tokens:2,output_tokens:1}].map(total=>JSON.stringify({type:'event_msg',timestamp:'2026-10-16T02:00:00Z',payload:{type:'token_count',info:{total_token_usage:total},rate_limits:{limit_id:'codex',plan_type:'pro',primary:{used_percent:12,window_minutes:300,resets_at:1792120000},credits:{has_credits:true,unlimited:false,balance:'4'}}}}));const codexState=provider.newCodexFileState();codexLines.forEach(line=>provider.foldCodexLine(codexState,line));
 const claudeLines=[{id:'message-one',tokens:10},{id:'message-one',tokens:99},{id:'message-two',tokens:5}].map(row=>JSON.stringify({type:'assistant',timestamp:'2026-10-16T02:00:00Z',message:{id:row.id,model:'claude-public',usage:{input_tokens:row.tokens,cache_creation_input_tokens:2,cache_read_input_tokens:3,output_tokens:1}}}));const claudeState=provider.newClaudeFileState();claudeLines.forEach(line=>provider.foldClaudeLine(claudeState,line));
 await fs.writeFile(path.join(out,'provider-cases.json'),JSON.stringify({codexLines,claudeLines,codex:{days:Object.fromEntries(codexState.days),limits:[...codexState.limits.values()]},claude:Object.fromEntries([...provider.claudeDayTotals([claudeState,claudeState])].map(([day,models])=>[day,Object.fromEntries(models)]))},null,2)+'\n');
 const sources=['src/main/session/store.ts','src/shared/chronology.ts','src/shared/session.ts','src/shared/usage.ts'];await fs.writeFile(path.join(out,'history-manifest.json'),JSON.stringify({commit,scope:'Actual upstream functions in disposable synthetic profile; config and diagnostic sinks stubbed only',sources:Object.fromEntries(await Promise.all(sources.map(async rel=>[rel,createHash('sha256').update(await fs.readFile(path.join(upstream,rel))).digest('hex')]))),operations:operations.length},null,2)+'\n');
}finally{await store.flushSessions();store.resetSessionStoreForTests();Date.now=nativeNow;await fs.rm(temp,{recursive:true,force:true});}
console.log('Captured upstream history, rebind, chronology and usage fixtures');
