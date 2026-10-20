"""CL-L1 local-only qualification harness; no product implementation changes."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time
import traceback
import urllib.request

REPO = Path(r'D:\projects\CoS-Rust\.worktrees\cl-l1-06f586e')
RUN = Path(__file__).resolve().parent
SHA = '06f586e423e5abb4f95eba582996c4a5278c1311'
BINARY = Path(r'D:\projects\CoS-Rust\.cache\cl-l1-20261020\target\release\cos-rust.exe')
ENV = os.environ.copy()
ENV.update(TEMP=str(RUN/'temp'), TMP=str(RUN/'temp'), TMPDIR=str(RUN/'temp'),
           PYTHONPYCACHEPREFIX=str(RUN/'pycache'))
FLAGS = {'creationflags': subprocess.CREATE_NO_WINDOW}
CHECKS = []
PROFILES = RUN/'profiles-attempt-2'
SECRETS = {key: 'CLL1_SYNTHETIC_'+key.upper()+'_20261020_X9'
           for key in ['environment','password','token','binary','clipboard_write',
                       'clipboard_read','nested_clipboard','file_handle','image','textual']}
MANIFESTS = []
PROJECTIONS = []

def write(name, obj):
    target = RUN/name
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(json.dumps(obj, ensure_ascii=False, indent=2)+'\n', encoding='utf-8')

def check(name, condition, detail=None):
    CHECKS.append({'name':name,'status':'PASS' if condition else 'FAIL','detail':detail})
    write('native-checks-progress.json',CHECKS)
    print(('PASS ' if condition else 'FAIL ')+name, flush=True)
    if not condition:
        raise AssertionError(name)

def clean(value):
    raw = json.dumps(value, ensure_ascii=False) if not isinstance(value,str) else value
    return not any(secret in raw for secret in SECRETS.values())

def cli(profile, *args):
    proc = subprocess.run([str(BINARY),'--data-dir',str(profile),*args], cwd=REPO,
                          env=ENV, capture_output=True, text=True, encoding='utf-8',
                          timeout=30, **FLAGS)
    if proc.returncode:
        # Never dump raw response/argument material on a failing privacy case.
        raise RuntimeError('CLI '+args[0]+' failed, exit='+str(proc.returncode))
    return json.loads(proc.stdout)

def record(profile, label, tool, args, result):
    payload = RUN/'inputs'/('public-synthetic-'+label+'.json')
    payload.parent.mkdir(exist_ok=True)
    payload.write_text(json.dumps({'requestId':'cl-l1-public-fixture','tool':tool,
                                  'args':args,'result':result,'outcome':'ok'}),encoding='utf-8')
    output = cli(profile,'record-call',str(payload))
    check(label+': returned record sanitized',clean(output))
    return output

def events(profile):
    rows=[]
    for path in sorted((profile/'sessions').rglob('events.jsonl')):
        rows.extend(json.loads(line) for line in path.read_text(encoding='utf-8').splitlines() if line.strip())
    return rows

def sentinel_ids(blob):
    return [key for key,value in SECRETS.items()
            if value.encode() in blob or value.encode('utf-16le') in blob]

def scan(profile, label):
    files=[]
    leaks=[]
    for path in sorted((profile/'sessions').rglob('*')):
        if path.is_file():
            blob=path.read_bytes()
            found=sentinel_ids(blob)
            if found:
                leaks.append({'file':path.relative_to(profile).as_posix(),'sentinelIds':found})
            files.append({'path':path.relative_to(profile).as_posix(),'bytes':len(blob),
                          'sha256':hashlib.sha256(blob).hexdigest()})
    check(label+': nonempty journal scan',any(row['path'].endswith('events.jsonl') for row in files))
    check(label+': disk journal/assets contain zero synthetic sentinels',not leaks,leaks)
    MANIFESTS.append({'profile':profile.name,'stage':label,'files':files,'leaks':leaks})
    return files

def rehydrate(profile, sid, stored):
    if stored.get('assetId'):
        target=profile/'sessions'/sid/'assets'/stored['assetId']
        return target.read_text(encoding='utf-8')
    return stored['text']

class Service:
    def __init__(self,profile,duration):
        self.profile=profile
        self.proc=subprocess.Popen([str(BINARY),'--data-dir',str(profile),'serve','--duration-ms',str(duration)],
                                   cwd=REPO,env=ENV,stdout=subprocess.PIPE,stderr=subprocess.PIPE,
                                   text=True,encoding='utf-8',**FLAGS)
        self.ready=json.loads(self.proc.stdout.readline())
        if self.ready.get('event') != 'ready':
            raise RuntimeError('service did not become ready')
        # URLs contain ephemeral authentication secrets: keep in memory only.
        self.serial=0
    def call(self,name,args):
        self.serial+=1
        params={'name':name,'arguments':args,'_meta':{
            'io.modelcontextprotocol/protocolVersion':'2026-07-28',
            'io.modelcontextprotocol/clientInfo':{'name':'CL-L1 public synthetic fixture','version':'1'},
            'io.modelcontextprotocol/clientCapabilities':{}}}
        request=urllib.request.Request(self.ready['urls']['core'],
            json.dumps({'jsonrpc':'2.0','id':self.serial,'method':'tools/call','params':params}).encode(),
            {'Accept':'application/json, text/event-stream','Content-Type':'application/json',
             'MCP-Protocol-Version':'2026-07-28','Mcp-Method':'tools/call','Mcp-Name':name,
             'x-request-id':'cl-l1-public-runtime/suffix'})
        with urllib.request.urlopen(request,timeout=15) as response:
            result=json.load(response)['result']
        check(name+': private recording evidence stripped', '_recordingEvidence' not in result)
        if result.get('isError') is True:
            raise RuntimeError('native '+name+' rejected')
        return result
    def stop(self,label):
        self.proc.wait(timeout=35)
        tail=self.proc.stdout.read()
        shutdown=json.loads(tail)
        check(label+': graceful process shutdown',self.proc.returncode == 0 and shutdown.get('drained') is True
              and shutdown.get('acceptedRemaining') == 0,
              {'pid':self.proc.pid,'exit':self.proc.returncode,'shutdown':shutdown})
    def cleanup(self):
        if self.proc.poll() is None:
            self.proc.terminate()
            self.proc.wait(timeout=5)

def latest(profile,tool):
    return next(row['call'] for row in reversed(events(profile))
                if row.get('kind')=='tool_call' and row['call']['tool']==tool)

def process_case(service,profile,label,cmd,expect_exit,require_running=False,tty=False):
    result=service.call('exec_command',{'cmd':cmd,'workdir':'/p','login':False,'tty':tty,
                                      'yield_time_ms':250 if require_running else 10000})
    observed=latest(profile,'exec_command')
    PROJECTIONS.append({'case':label,'stage':'launch','response':result,'call':observed})
    facts=observed['evidence']
    session=result['structuredContent'].get('session_id')
    if require_running:
        check(label+': native running session',isinstance(session,int) and facts['running'] is True
              and facts['exitCode'] is None and facts['processSessionId']==session)
        check(label+': started caption without success verdict',observed['summary']['title'].startswith('Started ')
              and observed['summary']['tone']=='neutral' and observed['summary'].get('metric')=='started')
    output=result['structuredContent'].get('output','')
    deadline=time.monotonic()+12
    while session is not None and time.monotonic()<deadline:
        result=service.call('write_stdin',{'session_id':session,'chars':'','yield_time_ms':1000})
        observed=latest(profile,'write_stdin')
        PROJECTIONS.append({'case':label,'stage':'poll','response':result,'call':observed})
        output+=result['structuredContent'].get('output','')
        if 'exit_code' in result['structuredContent']:
            break
    facts=observed['evidence']
    check(label+': actual completed exit',result['structuredContent'].get('exit_code')==expect_exit
          and facts['exitCode']==expect_exit and facts['running'] is False,
          {'exitCode':facts.get('exitCode'),'running':facts.get('running'),'session':session})
    check(label+': recorded outcome matches process',observed['outcome']==('process_exit_nonzero' if expect_exit else 'ok'))
    if expect_exit:
        check(label+': nonzero caption',observed['summary']['tone']=='bad' and observed['summary'].get('metric')=='✕ exit '+str(expect_exit))
    elif session is None:
        check(label+': successful command caption',observed['summary']['tone']=='good' and observed['summary']['title'].startswith('Ran '))
    check(label+': progressive caption persisted',isinstance(observed.get('runningSummary',{}).get('title'),str))
    return output

def live_privacy(service,profile,overflow):
    label='live-privacy-overflow' if overflow else 'live-privacy-inline'
    command="Write-Output 'token="+SECRETS['textual']+"'; Write-Output "+("('public output ' * 1200)" if overflow else "'public output'")+"; exit 0"
    if overflow:
        command+='\n# '+'public metadata '*900
    response=service.call('exec_command',{'cmd':command,'workdir':'/p','login':False,'yield_time_ms':10000})
    launch=latest(profile,'exec_command')
    raw=response['structuredContent'].get('output','')
    session=response['structuredContent'].get('session_id')
    deadline=time.monotonic()+12
    while session is not None and 'exit_code' not in response['structuredContent'] and time.monotonic()<deadline:
        response=service.call('write_stdin',{'session_id':session,'chars':'','yield_time_ms':1000})
        raw+=response['structuredContent'].get('output','')
    check(label+': real stdout positive sentinel control',SECRETS['textual'] in raw)
    check(label+': successful real process',response['structuredContent'].get('exit_code')==0)
    check(label+': stored args result captions sanitized',clean(launch))
    if overflow:
        check(label+': actual runtime argument and result assets',all(isinstance(launch[field].get('assetId'),str) for field in ['args','result']))
        for field in ['args','result']:
            check(label+': complete sanitized '+field+' asset',clean(rehydrate(profile,'unattributed-activity',launch[field])))
    else:
        check(label+': actual runtime inline fields',all('assetId' not in launch[field] for field in ['args','result']))

def main():
    check('Windows native host',os.name=='nt')
    actual=subprocess.check_output(['git','rev-parse','HEAD'],cwd=REPO,text=True).strip()
    check('exact requested product SHA',actual==SHA)
    check('clean product source before native probe',not subprocess.check_output(['git','status','--porcelain'],cwd=REPO,text=True).strip())
    check('all task output roots on D',all(str(path).lower().startswith('d:\\projects\\cos-rust\\') for path in [RUN,REPO,BINARY,PROFILES]))
    check('sentinel scanner positive control',all(key in sentinel_ids(('public '+value).encode()) for key,value in SECRETS.items()))
    PROFILES.mkdir(exist_ok=False)
    profile=PROFILES/'recorder-synthetic'
    cli(profile,'info')
    cli(profile,'config','set','sessions.record','true')
    args={'cmd':'Write-Output public','workdir':'/p','env':{'PRIVATE_ENV':SECRETS['environment']},
          'nested':{'password':SECRETS['password'],'secret':SECRETS['token']},'dataBase64':SECRETS['binary']}
    result='public output '+' '.join(SECRETS[key] for key in ['environment','password','token','binary'])+' token='+SECRETS['textual']
    small=record(profile,'inline','exec_command',args,result)
    check('inline: no overflow asset',all('assetId' not in small['call'][field] for field in ['args','result']))
    large=record(profile,'overflow','exec_command',{**args,'notes':'public metadata '*1000},'public output '*1200+result)
    check('overflow: both argument and result assets',all(isinstance(large['call'][field].get('assetId'),str) for field in ['args','result']))
    sid=large['sessionId']
    for field in ['args','result']:
        full=rehydrate(profile,sid,large['call'][field])
        check('overflow: '+field+' complete and sanitized',len(full)>8000 and clean(full))
        if field=='args':
            check('overflow: complete argument JSON',json.loads(full)['cmd']=='Write-Output public')
    record(profile,'clipboard-write','write_clipboard',{'text':SECRETS['clipboard_write']},'echo '+SECRETS['clipboard_write'])
    record(profile,'clipboard-read','read_clipboard',{'format':'text'},SECRETS['clipboard_read'])
    record(profile,'nested-clipboard','computer',{'actions':[{'type':'read_clipboard'},{'type':'write_clipboard','text':SECRETS['nested_clipboard']}]},SECRETS['nested_clipboard'])
    record(profile,'binary-and-handle','download_artifact',{'file':SECRETS['file_handle'],'image':{'type':'image','mimeType':'image/png','data':SECRETS['image']}},'public download')
    before=events(profile)
    check('privacy: captions only use sanitized values',clean([row['call'].get('summary') for row in before]))
    scan(profile,'privacy-before-restart')
    # Fresh OS process reconstructs history and export from the explicit synthetic profile.
    history=cli(profile,'history',sid,'--limit','200')['events']
    # History deliberately projects turnOrigin (chronology.rs:320); raw JSONL
    # has no such field. Here all six calls have no turn boundary.
    check('privacy: stable complete events after process restart',history==[{**row,'turnOrigin':None} for row in before])
    exported=cli(profile,'export',sid,'--format','json')
    check('privacy: restarted JSON export sanitized',clean(exported))
    write('privacy-history-after-restart.json',history)
    scan(profile,'privacy-after-restart')

    live=PROFILES/'runtime-synthetic'
    project=RUN/'public-project'
    project.mkdir()
    (project/'a.txt').write_text('one needle\ntwo needle\n',encoding='utf-8')
    cli(live,'info')
    for key,value in [('roots',json.dumps([{'name':'p','path':str(project.resolve())}])),
                      ('multiAgent.allowUnattributedCalls','true'),('sessions.record','true'),
                      ('controlApi.enabled','true'),('capabilities.command','true')]:
        cli(live,'config','set',key,value)
    cli(live,'sessions','create','CL-L1 synthetic timeline','--id','unattributed-activity','--conversation','cl-l1-timeline')
    now=int(time.time()*1000)-100
    observations=[{'kind':'user_message','messageId':'cl-l1-question','text':'Public synthetic qualification question','time':now},
                  {'kind':'turn_start','turnId':'cl-l1-turn','time':now+1},
                  {'kind':'assistant_message','messageId':'cl-l1-answer','turnId':'cl-l1-turn',
                   'text':'Public synthetic qualification answer','time':now+2,'state':'final','final':True}]
    observation_file=RUN/'inputs'/'public-timeline.json'
    observation_file.write_text(json.dumps(observations),encoding='utf-8')
    cli(live,'observe','cl-l1-timeline',str(observation_file))
    service=Service(live,25000)
    try:
        check('service: real Windows process started',service.proc.pid>0)
        process_case(service,live,'nonzero','Write-Output CL_L1_NONZERO; exit 7',7)
        output=process_case(service,live,'background-pipe','Write-Output CL_L1_BEGIN; Start-Sleep -Milliseconds 1800; Write-Output CL_L1_END; exit 0',0,True)
        check('background-pipe: completed unread output delivered','CL_L1_END' in output)
        process_case(service,live,'completed','Write-Output CL_L1_COMPLETE; exit 0',0)
        # The CL-L1 contract names the live PTY/shell boundary. Exercise actual ConPTY as well.
        output=process_case(service,live,'background-conpty','Write-Output CL_L1_PTY_BEGIN; Start-Sleep -Milliseconds 1800; Write-Output CL_L1_PTY_END; exit 0',0,True,True)
        check('background-conpty: completed output delivered','CL_L1_PTY_END' in output)
        live_privacy(service,live,False)
        live_privacy(service,live,True)
        found=service.call('find',{'query':'needle','mode':'content','path':'/p/a.txt'})
        call=latest(live,'find')
        check('find: real engine evidence and caption',call['evidence']['count']==2 and call['summary'].get('detail')=='2 matches in file contents')
        prior=events(live)
        check('timeline: stable unique ordered sequences',bool(prior) and [row['seq'] for row in prior]==sorted(set(row['seq'] for row in prior)))
        scan(live,'runtime-before-shutdown')
        service.stop('service-1')
    finally:
        service.cleanup()
    first_pid=service.proc.pid
    rows=cli(live,'history','unattributed-activity','--limit','200')['events']
    overview=cli(live,'sessions','list')['sessions']
    turn_origin=next(row.get('origin',row['seq']) for row in prior if row['kind']=='turn_start')
    expected=[{**row,'turnOrigin':turn_origin if row.get('turnId')=='cl-l1-turn' else None} for row in prior]
    check('timeline: complete cold CLI history with correct projected turn origins',rows==expected)
    check('timeline: persisted nonzero count',len(overview)==1 and overview[0]['processExitNonzero']==1)
    turn=overview[0]['timelineTurns'].get('cl-l1-turn',{})
    check('timeline: observed question and complete turn retained',turn.get('questionId')=='cl-l1-question'
          and turn.get('endOrigin',0)>turn.get('origin',0))
    check('timeline: persisted last activity',overview[0]['lastToolActivity']=={'title':call['summary']['title'],'kind':call['summary']['kind']})
    write('runtime-history-before-restart.json',prior)
    write('runtime-overview-before-restart.json',overview)
    restarted=Service(live,1600)
    try:
        check('service: distinct OS process restart',restarted.proc.pid!=first_pid)
        restarted.stop('service-2')
    finally:
        restarted.cleanup()
    after=cli(live,'history','unattributed-activity','--limit','200')['events']
    after_overview=cli(live,'sessions','list')['sessions']
    check('timeline: captions facts call IDs chronology survive service restart',after==rows)
    fields=['id','events','toolCalls','processExitNonzero','lastToolActivity','lastToolCallAt','timelineTurns','requestTurns']
    check('timeline: summary projection survives service restart',
          {k:after_overview[0].get(k) for k in fields}=={k:overview[0].get(k) for k in fields})
    write('runtime-history-after-restart.json',after)
    write('runtime-overview-after-restart.json',after_overview)
    scan(live,'runtime-after-restart')
    check('native runtime evidence contains no sentinels',clean(PROJECTIONS))
    write('native-runtime-projections.json',PROJECTIONS)

status='FAIL'
error=None
started=time.monotonic()
try:
    main()
    status='PASS'
except Exception as exc:
    error={'type':type(exc).__name__,'message':str(exc) if clean(str(exc)) else '<synthetic sentinel-bearing failure details suppressed>'}
    print('QUALIFICATION FAIL '+json.dumps(error),flush=True)
finally:
    write('disk-scan-manifests.json',MANIFESTS)
    write('native-runtime-projections.json',PROJECTIONS if clean(PROJECTIONS) else {'error':'unsafe projection suppressed'})
    write('native-receipt.json',{'unit':'CL-L1','status':status,'sourceSha':SHA,
          'binarySha256':hashlib.sha256(BINARY.read_bytes()).hexdigest() if BINARY.exists() else None,
          'binaryBytes':BINARY.stat().st_size if BINARY.exists() else None,
          'elapsedSeconds':time.monotonic()-started,'checks':CHECKS,'error':error,
          'personalProfilesUsed':False,'actualClipboardAccess':False,
          'scope':'Windows release CLI, loopback MCP, native pipes/ConPTY, synthetic recorder; clean restart only',
          'linux':'NOT RUN','macOS':'HOLD','desktopInstallerAcceptance':'NOT RUN',
          'performanceComparison':'NOT MEASURED','productSourceChanged':False})
raise SystemExit(0 if status=='PASS' else 1)
