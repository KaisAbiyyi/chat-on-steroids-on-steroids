"""Real executable + loopback proof with public fixtures and a disposable marked profile."""
import argparse, base64, hashlib, json, os, pathlib, subprocess, tempfile, time, urllib.request, urllib.error
p=argparse.ArgumentParser();p.add_argument('--binary',default='target/debug/cos-rust.exe');p.add_argument('--output',default='audit/2026-10-17/phase4-6-runtime.json');a=p.parse_args()
binary=pathlib.Path(a.binary).resolve();checks=[]
def check(name,condition,detail=None):
    checks.append({'name':name,'status':'PASS' if condition else 'FAIL','detail':detail});assert condition,(name,detail)
with tempfile.TemporaryDirectory(prefix='cos-rust-phase4-6-public-') as temp:
    root=pathlib.Path(temp);profile=root/'profile';project=root/'project';project.mkdir();(project/'a.txt').write_text('one\npublic fixture\n',encoding='utf8')
    def cli(*args):return json.loads(subprocess.check_output([str(binary),'--data-dir',str(profile),*args],text=True,encoding='utf8'))
    cli('info');cli('config','set','roots',json.dumps([{'name':'p','path':str(project.resolve())}]));cli('config','set','multiAgent.allowUnattributedCalls','true');cli('config','set','controlApi.enabled','true')
    original=(project/'a.txt').read_bytes();(root/'new.txt').write_text('must not be saved');cli('config','set','readOnly','true')
    denied=subprocess.run([str(binary),'--data-dir',str(profile),'files','save','/p/a.txt',str(root/'new.txt'),hashlib.sha256(original).hexdigest()],capture_output=True,text=True,encoding='utf8')
    check('human file mutation respects read-only permission',denied.returncode!=0 and (project/'a.txt').read_bytes()==original)
    cli('config','set','readOnly','false')
    proc=subprocess.Popen([str(binary),'--data-dir',str(profile),'serve','--duration-ms','20000'],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,encoding='utf8',creationflags=0x08000000 if os.name=='nt' else 0)
    try:
        ready=json.loads(proc.stdout.readline());check('persistent native MCP listener',ready['event']=='ready');url=ready['urls']['core'];port=ready['port'];control=ready['controlPort']
        def http(url,body=None,headers=None,method=None):
            data=None if body is None else json.dumps(body).encode();request=urllib.request.Request(url,data,{'Accept':'application/json, text/event-stream','Content-Type':'application/json',**(headers or {})},method=method)
            try:
                with urllib.request.urlopen(request,timeout=10) as response:return response.status,response.read().decode()
            except urllib.error.HTTPError as e:return e.code,e.read().decode()
        serial=0
        def rpc(method,params=None,surface='core'):
            global serial
            serial+=1;params=dict(params or {});params['_meta']={'io.modelcontextprotocol/protocolVersion':'2026-07-28','io.modelcontextprotocol/clientInfo':{'name':'public fixture','version':'1'},'io.modelcontextprotocol/clientCapabilities':{}}
            headers={'MCP-Protocol-Version':'2026-07-28','Mcp-Method':method,'x-request-id':'wfr_public_fixture/suffix'}
            if method=='tools/call':headers['Mcp-Name']=params['name']
            status,raw=http(ready['urls'][surface],{'jsonrpc':'2.0','id':serial,'method':method,'params':params},headers);assert status==200,(status,raw);return json.loads(raw)
        def call(name,args):return rpc('tools/call',{'name':name,'arguments':args})['result']
        tools=rpc('tools/list')['result']['tools'];check('real available discovery',all(name in [t['name'] for t in tools] for name in ['read','apply_patch','exec_command','write_stdin','exec']))
        check('separate desktop and plugins',rpc('tools/list',surface='desktop')['result']['tools']==[] and rpc('tools/list',surface='plugins')['result']['tools']==[])
        status,raw=http(url,None,{'Origin':'https://public.invalid'},'GET');check('origin refused',status==403)
        status,raw=http('http://127.0.0.1:'+str(port)+'/.well-known/oauth-protected-resource','',method='GET');check('bare metadata does not disclose token',status==404 and url not in raw)
        result=call('read',{'paths':['/p/a.txt']});check('real bounded approved file read','public fixture' in json.dumps(result),result)
        result=call('exec',{'code':"const rows=await Promise.all([tools.read({paths:['/p/a.txt']}),tools.read({paths:['/p/a.txt']})]);text(rows.map(r=>r.isError??false));text([typeof process,typeof require,typeof fetch]);"});check('isolated JavaScript worker + nested tools',result.get('isError') is not True and result['content'][0]['text']=='[false,false]',result)
        source='*** Begin Patch\n*** Update File: a.txt\n@@\n-one\n+changed\n*** End Patch'
        result=call('exec',{'code':'await tools.apply_patch({patch:'+json.dumps(source)+'});while(true){}'});check('accepted side effect remains after JS timeout',result.get('isError') is True and (project/'a.txt').read_text().startswith('changed'),result)
        result=call('exec_command',{'cmd':'Write-Output first; Start-Sleep -Milliseconds 800; Write-Output last' if os.name=='nt' else 'echo first; sleep 1; echo last','workdir':'/p','yield_time_ms':250,'login':False});session=result['structuredContent'].get('session_id');check('running native process continuation',bool(session),result)
        deadline=time.monotonic()+5;delivered=''
        while time.monotonic()<deadline:
            result=call('write_stdin',{'session_id':session,'yield_time_ms':1000});delivered+=result['structuredContent']['output']
            if 'exit_code' in result['structuredContent']:break
        check('unread background completion',result['structuredContent']['exit_code']==0 and 'last' in delivered,result)
        token=(profile/'control-api/token').read_text().strip();status,raw=http('http://127.0.0.1:'+str(control)+'/v1/health',None,{'Authorization':'Bearer '+token},'GET');check('control read projection excludes secrets',status==200 and url not in raw)
        status,raw=http('http://127.0.0.1:'+str(control)+'/v1/inputs',{}, {'Authorization':'Bearer '+token},'POST');check('actions separately off',status==403 and 'actions_disabled' in raw)
        check('public fixture only',str(profile).startswith(temp))
        proc.wait(timeout=25);shutdown=json.loads(proc.stdout.read())
        check('graceful listener and persistence shutdown',proc.returncode==0 and shutdown.get('drained') is True and shutdown.get('acceptedRemaining')==0,shutdown)
        events=[]
        for journal in (profile/'sessions').rglob('events.jsonl'):
            events.extend(json.loads(line) for line in journal.read_text(encoding='utf8').splitlines() if line.strip())
        calls=[e['call'] for e in events if e.get('kind')=='tool_call'];parents={c['callId'] for c in calls if c.get('tool')=='exec'};nested=[c for c in calls if c.get('nested')]
        check('nested native calls retain parent recording provenance',len(nested)>=3 and all(c.get('parentCallId') in parents for c in nested))
        check('control endpoint withdrawn on shutdown',not (profile/'control-api/endpoint.json').exists())
    finally:
        if proc.poll() is None:proc.terminate()
        try:proc.wait(timeout=5)
        except subprocess.TimeoutExpired:proc.kill();proc.wait()
out=pathlib.Path(a.output);out.parent.mkdir(parents=True,exist_ok=True);out.write_text(json.dumps({'binary':str(binary),'date':'2026-10-17','checks':checks,'externalSsh':'NOT RUN','publicTunnel':'NOT RUN','macOSLinuxNative':'NOT RUN'},indent=2)+'\n',encoding='utf8');print(json.dumps({'status':'PASS','checks':len(checks),'receipt':str(out.resolve())}))
