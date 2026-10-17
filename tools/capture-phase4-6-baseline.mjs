// Public, isolated upstream functions; never imports the application bootstrap or live profile.
import fs from 'node:fs/promises';
import path from 'node:path';
import { createRequire } from 'node:module';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'..');
const upstream=path.join(root,'upstream/chat-on-steroids');
const commit='024731d8083d92c46e8248d64e7e0b36ab0a470f';
if(execFileSync('git',['rev-parse','HEAD'],{cwd:upstream,encoding:'utf8'}).trim()!==commit)throw Error('Baseline changed');
const require=createRequire(path.join(upstream,'package.json')),esbuild=require('esbuild');
const cache=path.join(upstream,'node_modules/.cache/cos-rust-oracle/phase4-6');
const out=path.join(root,'fixtures/upstream/phase4-6');await fs.mkdir(cache,{recursive:true});await fs.mkdir(out,{recursive:true});
const files=new Set();
async function load(rel,name){files.add(rel);const dest=path.join(cache,name+'.cjs');await esbuild.build({entryPoints:[path.join(upstream,rel)],outfile:dest,bundle:true,platform:'node',format:'cjs',packages:'external',logLevel:'warning'});return require(dest);}
async function save(name,value){await fs.writeFile(path.join(out,name+'.json'),JSON.stringify(value,null,2)+'\n');}
const policy=await load('src/shared/command-allowlist.ts','policy');
const commands=['git status','git status --short','git "status"','npm test','npm test -- --run','echo a; echo b','echo $HOME','echo "hello world"','echo \'hello world\'','echo a\\b','echo #hi','FOO=bar git status','& git status','echo %PATH%','echo a,b','echo *.ts','git sta"tus"','echo a\nb','echo "unclosed',' git  status '];
const policies=[{enabled:false,mode:'allow',rules:[]},{enabled:true,mode:'allow',rules:['git status','npm test *','echo *']},{enabled:true,mode:'deny',rules:['git status *']},{enabled:true,mode:'allow',rules:['git * status']}];
await save('command-policy',policies.flatMap(p=>['powershell','cmd','bash','sh','zsh'].flatMap(shell=>commands.map(command=>({policy:p,shell,commands:[command],expected:policy.evaluateCommandAllowlist(p,[command],shell)})))));
const parser=await load('src/main/codex/apply-patch/parser.ts','parser');
const patches=['','*** Begin Patch\n*** End Patch','bad','*** Begin Patch\n*** Add File: a\n+x\n*** End Patch','*** Begin Patch\n*** Delete File: a\n*** End Patch','*** Begin Patch\n*** Update File: a\n@@\n-old\n+new\n*** End Patch','*** Begin Patch\n*** Update File: a\n*** Move to: b\n@@ suffix\n x\n+y\n*** End of File\n*** End Patch','*** Begin Patch\n*** Environment ID: e\n*** Add File: a\n+x\n*** End Patch',"<<'EOF'\n*** Begin Patch\n*** Add File: a\n+x\n*** End Patch\nEOF",'*** Begin Patch\n*** Update File: a\nBAD\n*** End Patch','*** Begin Patch\n*** Add File: a\nbad\n*** End Patch','*** Begin Patch\n*** Update File: a\n@@\n*** End Patch'];
await save('patch-parser',patches.map(patch=>{try{return{patch,expected:parser.parsePatch(patch)}}catch(e){return{patch,error:e.message}}}));
const text=await load('src/main/codex/apply-patch/text-file.ts','text');
await save('text-replacements',['a\nb\n','a\r\nb\r\n','a\rb\r','a\r\nb\nc','a\nb','😀\r\nz',''].flatMap(contents=>[[[0,1,['new']]],[[1,0,['extra']]],[]].map(replacements=>{const source=text.SourceFile.parse(contents),lines=source.lineTexts();source.applyReplacements(replacements);return{contents,replacements,lines,expected:source.intoContents()}})));
// SSH pure helpers have SFTP imports but perform no network operation at import time.
const ssh=await load('src/main/remote/ssh.ts','ssh');
const batch=await load('src/main/codex/command-batch.ts','batch');
const truncate=await load('src/main/codex/truncate.ts','truncate');
await save('output-truncation',['','hello\nworld\n','😀 Unicode é\nsecond\n','x'.repeat(120)].flatMap(text=>[0,1,2,5,100].map(tokens=>({text,tokens,expected:truncate.formattedTruncateText(text,{kind:'tokens',tokens})}))));
const health=await load('src/main/tunnel/health.ts','tunnel-health');
await save('tunnel-health', [
 {metrics:''}, {metrics:'commands_poll_last_successful_timestamp_seconds 0\ncommands_poll_cycles_total{channel="main"} 3\ncommands_poll_errors_total{kind="timeout"} 2\ncommands_poll_errors_total{kind="network"} 1'},
 {metrics:'commands_poll_last_successful_timestamp_seconds 1729000000.123\ncommands_poll_cycles_total 9\ncommands_poll_errors_total NaN\ncommands_poll_errors_total_extra 500'},
 {status:{}},{status:{version:'1',channels:[{name:'other',probe_status:'failed'},{name:'main',probe_status:'ready'}],uptime_seconds:12,control_plane_route:{target:'api.openai.com:443',route_mode:'direct',proxy_source:'none'}}},
 {status:{tunnel_metadata_error:'public fixture unavailable',control_plane_route:{target:'api.openai.com:443',proxy_source:'environment'}}}
].map(c=>({...c,expected:c.metrics!==undefined?health.parsePollHealth(c.metrics):health.parseClientStatus(c.status)})));
await save('command-batch',['powershell','cmd','bash','sh','zsh'].map(shell=>{const commands=['echo first','echo "two words"','echo last'];const result=batch.composeCommandBatch(commands,shell);return{shell,commands,marker:'0'.repeat(24),expected:result.command.replaceAll(result.marker,'0'.repeat(24))};}));
await save('ssh',['dev','me@server:2222','ssh://me@server','[::1]:22','bad;host','-x','dev:0'].flatMap(address=>['/srv/app','~/a b','/a/../b'].map(folder=>{try{const remote=ssh.parseRemote(address,folder);return{address,folder,remote,args:ssh.sshArgs(remote,['-s'],['sftp']),command:ssh.remoteCommand(remote.path,"printf '%s\\n' '😀 a'"),login:ssh.remoteLoginShell(remote.path)}}catch(e){return{address,folder,error:e.message}}})));
const specs=await load('src/main/codex/tool-specs.ts','specs');
const agentPlan=await load('src/shared/agent-plan.ts','agent-plan');
const z=require('zod').z;
const source=await fs.readFile(path.join(upstream,'src/main/mcp/tools-core.ts'),'utf8');
// Only declarations are evaluated; handler bodies are retained but never invoked.
const stripped=source.replace(/^import[\s\S]*?\bfrom\s+['"][^'"]+['"];\s*/gm,'');
const transformed=await esbuild.transform(stripped,{loader:'ts',format:'cjs',target:'node24'});
const vm=await import('node:vm');
const planSource=await fs.readFile(path.join(upstream,'src/main/mcp/plan-tool.ts'),'utf8');files.add('src/main/mcp/plan-tool.ts');
const planCompiled=await esbuild.transform(planSource.replace(/^import[\s\S]*?\bfrom\s+['"][^'"]+['"];\s*/gm,''),{loader:'ts',format:'cjs',target:'node24'});
const planSandbox={module:{exports:{}},exports:{},toolDeclaration:(_name,build)=>build(),agentPlanUpdateSchema:agentPlan.agentPlanUpdateSchema};vm.runInNewContext(planCompiled.code,planSandbox);
const sandbox={module:{exports:{}},exports:{},...specs,z,toolDeclaration:(_name,build)=>build(),
 DEFAULT_READ_BYTES:256*1024,MAX_READ_BYTES:512*1024,MAX_CONTENT_FILE_BYTES:2*1024*1024,
 formatBytes:n=>n<1024?`${n} B`:n<1024*1024?`${(n/1024).toFixed(1)} KB`:n<1024*1024*1024?`${(n/(1024*1024)).toFixed(1)} MB`:`${(n/(1024*1024*1024)).toFixed(2)} GB`,pathArg:z.string().min(1).max(4096),lineNumberArg:z.number().int().min(1).max(100_000_000),
 VIEW_IMAGE_DESCRIPTION:'View a local image file from the filesystem when visual inspection is needed. Use this for images already available on disk.',VIEW_IMAGE_PATH_DESCRIPTION:'Local filesystem path to an image file.',
 registerPlanTool:planSandbox.module.exports.registerPlanTool,imageExportCapable:()=>false,defaultUserShell:()=>({shellType:'powershell',shellPath:'powershell'}),
 getConfig:()=>({ui:{finishTool:false}}),os:require('node:os'),nodePath:require('node:path'),REASONING_EFFORTS:[]};
vm.runInNewContext(transformed.code,sandbox);
const declarations=[];const caps=Object.fromEntries(['read','browse','metadata','search','create','edit','move','deleteFile','command'].map(k=>[k,true]));
const reg={ctx:{roots:[{name:'project',path:'C:\\public-fixture'}]},caps,exposedCaps:caps,findExposed:false,sessionToolsExposed:true,agentToolsExposed:false,register(name,definition){declarations.push({name,...definition,inputSchema:z.toJSONSchema(definition.inputSchema),...(definition.outputSchema?{outputSchema:z.toJSONSchema(definition.outputSchema)}:{})});}};
sandbox.module.exports.registerCoreTools(reg);
// find only exists when commands were disabled at endpoint creation.
reg.exposedCaps={...caps,command:false};reg.findExposed=true;sandbox.module.exports.registerCoreTools(reg);
const codeSource=await fs.readFile(path.join(upstream,'src/main/mcp/code-mode-tool.ts'),'utf8');
const codeCompiled=await esbuild.transform(codeSource.replace(/^import[\s\S]*?\bfrom\s+['"][^'"]+['"];\s*/gm,''),{loader:'ts',format:'cjs',target:'node24'});
const codeSandbox={module:{exports:{}},exports:{},toolDeclaration:(_name,build)=>build(),codeModeSchema:z.object({code:z.string().min(1).max(64000).describe('Raw JavaScript source with top-level await. Emit results with text(...) or image(...).')}).strict()};vm.runInNewContext(codeCompiled.code,codeSandbox);reg.register('exec',codeSandbox.module.exports.codeModeDeclaration());
await save('tool-declarations',[...new Map(declarations.map(d=>[d.name,d])).values()]);
files.add('src/main/mcp/tools-core.ts');
await save('manifest',{commit,scope:'Pure upstream function oracles and actual Zod declarations; no handler or provider action',files:await Promise.all([...files].sort().map(async file=>({file,sha256:createHash('sha256').update(await fs.readFile(path.join(upstream,file))).digest('hex')})))});
console.log(JSON.stringify({commit,fixtures:out,policyCases:policies.length*5*commands.length,tools:declarations.map(d=>d.name)}));
const {McpServer,createMcpHandler}=require('@modelcontextprotocol/server');
const handler=createMcpHandler(()=>{const server=new McpServer({name:'fixture',version:'1'});server.registerTool('echo',{inputSchema:z.object({text:z.string()}).strict()},async({text})=>({content:[{type:'text',text}]}));return server;});
const meta={'io.modelcontextprotocol/protocolVersion':'2026-07-28','io.modelcontextprotocol/clientInfo':{name:'fixture',version:'1'},'io.modelcontextprotocol/clientCapabilities':{}};
const requests=[
 {method:'GET'},{method:'DELETE'},
 ...['2024-11-05','2025-03-26','2025-06-18','2025-11-25','2026-07-28','1900-01-01'].map(protocolVersion=>({method:'POST',body:{jsonrpc:'2.0',id:1,method:'initialize',params:{protocolVersion,capabilities:{},clientInfo:{name:'fixture',version:'1'}}}})),
 ...['ping','tools/list','server/discover','unknown'].map(method=>({method:'POST',body:{jsonrpc:'2.0',id:2,method,params:{_meta:meta}}})),
 ...[{name:'echo',arguments:{text:'public fixture'}},{name:'echo',arguments:{text:123}},{name:'computer',arguments:{}}].map(params=>({method:'POST',body:{jsonrpc:'2.0',id:3,method:'tools/call',params:{...params,_meta:meta}}})),
 {method:'POST',raw:'invalid JSON'},
 {method:'POST',body:[{jsonrpc:'2.0',id:5,method:'ping'}]},
 {method:'POST',body:{jsonrpc:'2.0',method:'notifications/initialized'}}
];
requests.push(...requests.filter(r=>r.body?.params?._meta).map(r=>({...r,headers:{'mcp-protocol-version':'2026-07-28'}})),{method:'POST',headers:{'mcp-protocol-version':'2025-11-25'},body:{jsonrpc:'2.0',id:4,method:'tools/list',params:{}}});
requests.push(...requests.filter(r=>r.body?.params?._meta&&!r.headers).map(r=>({...r,headers:{'mcp-protocol-version':'2026-07-28','mcp-method':r.body.method,...(r.body.params.name?{'mcp-name':r.body.params.name}:{})}})));
requests.push({method:'POST',headers:{accept:'application/json'},body:{jsonrpc:'2.0',id:9,method:'initialize',params:{protocolVersion:'2025-11-25',capabilities:{},clientInfo:{name:'fixture',version:'1'}}}},{method:'POST',headers:{accept:'application/json','mcp-protocol-version':'2026-07-28','mcp-method':'tools/list'},body:{jsonrpc:'2.0',id:10,method:'tools/list',params:{_meta:meta}}},{method:'POST',headers:{'mcp-protocol-version':'2026-07-28','mcp-method':'tools/list'},body:[{jsonrpc:'2.0',id:10,method:'tools/list',params:{_meta:meta}}]});
const protocol=[];for(const request of requests){const response=await handler.fetch(new Request('http://127.0.0.1/mcp',{method:request.method,headers:{'content-type':'application/json',accept:'application/json, text/event-stream',...request.headers},...(request.method==='POST'?{body:request.raw??JSON.stringify(request.body)}:{})}));const raw=await response.text();let result;try{result=JSON.parse(raw)}catch{result=raw;}protocol.push({request,status:response.status,contentType:response.headers.get('content-type'),result});}
await handler.close();await save('mcp-protocol',{sdk:'@modelcontextprotocol/server@2.1.0',scope:'Actual SDK handler protocol oracle with a public fixture echo tool; not a product tool invocation',cases:protocol});
