// Execute the pinned invocation parser on public scripts; no shell command is run.
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
const names=['errors.ts','hunk.ts','streaming-parser.ts','parser.ts','invocation.ts'];
const sources=await Promise.all(names.map(async name=>[name,await fs.readFile(path.join(upstream,'src/main/codex/apply-patch',name),'utf8')]));
const code=sources.map(([,body])=>body.replace(/^import[\s\S]*?;\r?$/gm,'')).join('\n')+'\nglobalThis.fixture={maybeParseApplyPatch,maybeParseApplyPatchForExec};';
const js=await require('esbuild').transform(code,{loader:'ts',format:'cjs'});
const context=vm.createContext({module:{exports:{}},exports:{},nodePath:path,Parser:require('tree-sitter'),Bash:require('tree-sitter-bash')});
vm.runInContext(js.code,context);
const patch='*** Begin Patch\n*** Add File: public.txt\n+public 😀\n*** End Patch';
const heredoc=body=>`apply_patch <<'PATCH'\n${body}\nPATCH`;
const scripts=[heredoc(patch),heredoc('invalid patch'),patch,
  heredoc(patch).replace('apply_patch','applypatch'),
  `cd public && ${heredoc(patch)}`,`cd "public folder" && ${heredoc(patch)}`,
  `cd 'public folder' && ${heredoc(patch)}`,`cd "$HOME/public" && ${heredoc(patch)}`,
  `cd public && ${heredoc(patch).replace('apply_patch','apply_patch ignored')}`,
  heredoc(patch).replace('apply_patch','apply_patch ignored'),
  `echo public; ${heredoc(patch)}`,`${heredoc(patch)}\necho public`,
  `# public comment\n${heredoc(patch)}`,`${heredoc(patch)}\n# public comment`,
  heredoc(patch).replace('apply_patch','/usr/bin/apply_patch'),
  heredoc(patch).replace('apply_patch','"apply_patch"'),
  heredoc(patch).replace('apply_patch','APPLY_PATCH'),
  heredoc(patch).replace("<<'PATCH'",'<<PATCH'),
  heredoc(patch).replace("<<'PATCH'",'<<"PATCH"'),
  heredoc(patch).replaceAll('\n','\r\n'),
  `cd public ; ${heredoc(patch)}`,`cd public || ${heredoc(patch)}`,
  `cd a b && ${heredoc(patch)}`,`cd 'public\\folder' && ${heredoc(patch)}`,
  'echo public',`apply_patch <<EOF\n${patch}\nEOF\n\n`,
  `apply_patch <<-EOF\n\t${patch.replaceAll('\n','\n\t')}\n\tEOF`
];
const inputs=[];
for(const argv of [['apply_patch',patch],['applypatch',patch],['apply_patch','bad'],['apply_patch'],[patch],['echo'],['apply_patch',patch,'extra'],['APPLY_PATCH',patch],['/bin/apply_patch',patch]])inputs.push({argv,cwd:'/public'});
for(const [shell,flags,cwd] of [
  ['/bin/bash',['-lc'],'/public'],['sh',['-c'],'/public'],['zsh',['-c'],'/public'],
  ['C:\\Windows\\powershell.exe',['-NoProfile','-Command'],'C:\\public'],
  ['PWSH.EXE',['-cOmMaNd'],'C:\\public'],['cmd.exe',['/C'],'C:\\public'],
  ['pwsh',['-c'],'/public'],['C:\\public\\bash.exe',['-c'],'/public'],
  ['powershell',['-NoLogo','-Command'],'C:\\public']
])for(const script of scripts)inputs.push({argv:[shell,...flags,script],cwd});
const cases=[];
for(const input of inputs)for(const mode of ['parse','exec']){
  const result=context.fixture[mode==='exec'?'maybeParseApplyPatchForExec':'maybeParseApplyPatch'](input.argv,input.cwd);
  cases.push({...input,mode,expected:result.error?{kind:result.kind,error:result.error.message}:result});
}
const output=path.join(root,'fixtures/upstream/phase1-6-closure/patch-invocation.json');
const query=sources.find(([name])=>name==='invocation.ts')[1].match(/const APPLY_PATCH_QUERY = String\.raw`([\s\S]*?)`;/)[1];
await fs.writeFile(path.join(root,'fixtures/upstream/phase1-6-closure/apply-patch-query.scm'),query);
await fs.writeFile(output,JSON.stringify({upstreamCommit:commit,grammarVersion:'0.25.1',sources:Object.fromEntries(sources.map(([name,body])=>[name,createHash('sha256').update(body).digest('hex')])),cases},null,2)+'\n');
console.log(JSON.stringify({cases:cases.length,grammarVersion:'0.25.1',commandsExecuted:0}));
