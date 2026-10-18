"""Execute an exact Git snapshot in the task-owned disposable Linux container."""
import argparse, hashlib, json, pathlib, re, shutil, subprocess, tempfile, uuid

p=argparse.ArgumentParser()
p.add_argument('--container',default='cos-rust-qualify-20261017')
p.add_argument('--source',default='HEAD')
p.add_argument('--output',required=True)
p.add_argument('--strict',action='store_true',help='Also run formatting and strict all-target Clippy')
p.add_argument('--lean-build',action=argparse.BooleanOptionalAction,default=True,help='Use the default storage-conscious cache; --no-lean-build explicitly requests full debug symbols/incremental state')
a=p.parse_args()
repo=pathlib.Path(__file__).resolve().parents[1]

def run(*args):
    return subprocess.check_output(args,cwd=repo,text=True,encoding='utf-8').strip()

source=run('git','rev-parse',a.source+'^{commit}')
container=json.loads(run('docker','inspect',a.container))[0]
assert container['Config']['Labels'].get('com.cos-rust.task')=='phase1-6-closure','Refusing an unrelated container'
assert container['State']['Running'],'Start the owned qualification container first'
image='rust@sha256:8514999d4786ef12efe89239e86b3d0a021b94b9d35108c8efe6c79ca7dc1a65'
assert container['Config']['Image']==image,'Qualification image identity changed'
output=(repo/a.output).resolve()
assert output.is_relative_to(repo/'audit'),'Receipts must stay inside the workspace audit directory'
output.parent.mkdir(parents=True,exist_ok=True)
log=output.with_suffix('.log')
if output.exists() or log.exists():
    raise FileExistsError('Preserve historical qualification receipts; choose a new output path')

# Host archives belong to the workspace drive, rather than the Windows system Temp.
# Keep the Docker cache bounded by using one profile; never prune other containers.
temporary_root=repo/'.cache'/'qualification'
temporary_root.mkdir(parents=True,exist_ok=True)
assert temporary_root.resolve().is_relative_to(repo),'Qualification temporary directory escaped the workspace'
host_free=shutil.disk_usage(temporary_root).free
if host_free < 8*1024**3:
    raise RuntimeError('Qualification requires 8 GiB free on the workspace drive')
system_free=shutil.disk_usage(pathlib.Path.home().anchor).free
if system_free < 12*1024**3:
    raise RuntimeError('Qualification requires 12 GiB free on the system drive for Docker and OS headroom')
build_env={'CARGO_TARGET_DIR':'/work/target'}
if a.lean_build:
    build_env.update(CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0')
else:
    build_env.update(CARGO_INCREMENTAL='1',CARGO_PROFILE_DEV_DEBUG='2',CARGO_PROFILE_TEST_DEBUG='2')
docker_exec=['docker','exec']
for key,value in build_env.items():
    docker_exec.extend(['-e',key+'='+value])
token=uuid.uuid4().hex
checkout='/work/checkouts/'+source[:12]+'-'+token
archive='/tmp/cos-rust-source-'+token+'.tar'
with tempfile.TemporaryDirectory(prefix='cos-rust-public-linux-',dir=temporary_root) as temporary:
    local=pathlib.Path(temporary)/'source.tar'
    subprocess.run(['git','archive','--format=tar','--output',str(local),source],cwd=repo,check=True)
    subprocess.run(['docker','cp',str(local),a.container+':'+archive],check=True)
    subprocess.run(['docker','exec',a.container,'mkdir','-p',checkout],check=True)
    subprocess.run(['docker','exec',a.container,'tar','xf',archive,'-C',checkout],check=True)
    print('Running actual Linux tests for '+source,flush=True)
    checks={}
    with log.open('x',encoding='utf-8',newline='\n') as stream:
        result=subprocess.run([*docker_exec,'-w',checkout,a.container,'/usr/local/cargo/bin/cargo','test','--workspace','--locked','-j2'],stdout=stream,stderr=subprocess.STDOUT)
        checks['workspaceTests']=result.returncode
        if a.strict:
            for name,command in [('format',['fmt','--all','--','--check']),('clippy',['clippy','--workspace','--all-targets','--locked','-j2','--','-D','warnings'])]:
                checks[name]=subprocess.run([*docker_exec,'-w',checkout,a.container,'/usr/local/cargo/bin/cargo',*command],stdout=stream,stderr=subprocess.STDOUT).returncode
    subprocess.run(['docker','exec',a.container,'rm','--',archive],check=True)
text=log.read_text(encoding='utf-8')
rows=re.findall(r'test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored',text)
counts=dict(zip(['passed','failed','ignored'],[sum(int(row[i]) for row in rows) for i in range(3)]))
receipt={
    'status':'PASS' if all(code==0 for code in checks.values()) and counts['passed']>0 and counts['failed']==0 and counts['ignored']==0 else 'FAIL',
    'sourceCommit':source,'scope':'Actual native Linux execution of an isolated Git archive; host working-tree changes excluded',
    'rustImage':image,'rustc':run('docker','exec',a.container,'/usr/local/cargo/bin/rustc','--version'),'checks':checks,'checkout':checkout,
    'kernel':run('docker','exec',a.container,'uname','-srmo'),'tests':counts,
    'log':log.relative_to(repo).as_posix(),'logSha256':hashlib.sha256(log.read_bytes()).hexdigest(),
    'storage':{'hostTemporaryDirectory':str(temporary_root),'workspaceFreeBefore':host_free,'systemFreeBefore':system_free,'buildEnvironment':build_env,'linuxCacheUsage':run('docker','exec',a.container,'du','-sh','/work/target')},
    'personalProfilesUsed':False,'macOS':'HOLD by user request'
}
output.write_text(json.dumps(receipt,indent=2)+'\n',encoding='utf-8',newline='\n')
print(json.dumps(receipt,indent=2),flush=True)
raise SystemExit(0 if receipt['status']=='PASS' else 1)
