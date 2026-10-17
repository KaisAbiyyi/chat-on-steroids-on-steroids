"""Qualify host OpenSSH against our disposable Linux container, with private keys/config only."""
import argparse, json, os, pathlib, shutil, subprocess, tempfile, time, uuid
p=argparse.ArgumentParser();p.add_argument('--container',default='cos-rust-qualify-20261017');p.add_argument('--output',default='audit/2026-10-17/ssh-qualification.json');a=p.parse_args()
repo=pathlib.Path(__file__).resolve().parents[1]
def run(args,**kw):
    result=subprocess.run(args,capture_output=True,**kw)
    if result.returncode:
        print(result.stdout.decode('utf-8',errors='replace') if isinstance(result.stdout,bytes) else result.stdout or '')
        print(result.stderr.decode('utf-8',errors='replace') if isinstance(result.stderr,bytes) else result.stderr or '')
        result.check_returncode()
    return result
inspect=json.loads(run(['docker','inspect',a.container],text=True).stdout)[0]
assert inspect['Config']['Labels'].get('com.cos-rust.task')=='phase1-6-closure','Not an owned qualification container'
docker=pathlib.Path(shutil.which('docker')).as_posix();keygen=shutil.which('ssh-keygen');assert keygen
server='/tmp/cos-rust-ssh-public-'+uuid.uuid4().hex
with tempfile.TemporaryDirectory(prefix='cos-rust-public-ssh-') as temporary:
    local=pathlib.Path(temporary)
    for name in ['client','unauthorized']:
        run([keygen,'-q','-t','ed25519','-N','','-f',str(local/name)])
    setup=f'''set -eu
mkdir -p {server}/project /run/sshd
id cosfixture >/dev/null 2>&1 || useradd -m -s /bin/sh cosfixture
printf 'cosfixture:public-fixture-password-unused\\n' | chpasswd
ssh-keygen -q -t ed25519 -N '' -f {server}/host_key
printf 'public SSH fixture\\n' > {server}/project/public.txt
printf 'public outside fixture\\n' > {server}/outside.txt
ln -s ../outside.txt {server}/project/escape.txt
chown -R cosfixture:cosfixture {server}/project
cat > {server}/sshd_config <<'CONFIG'
Port 2222
ListenAddress 127.0.0.1
HostKey {server}/host_key
AuthorizedKeysFile {server}/client.pub
PasswordAuthentication no
KbdInteractiveAuthentication no
PubkeyAuthentication yes
UsePAM no
PermitRootLogin no
StrictModes no
AllowUsers cosfixture
PidFile {server}/sshd.pid
Subsystem sftp /usr/lib/openssh/sftp-server
LogLevel ERROR
CONFIG
/usr/sbin/sshd -t -f {server}/sshd_config
'''
    run(['docker','exec','-i',a.container,'bash','-s'],input=setup.encode('utf-8'))
    run(['docker','cp',str(local/'client.pub'),f'{a.container}:{server}/client.pub'])
    hostkey=run(['docker','exec',a.container,'cat',server+'/host_key.pub'],text=True).stdout.strip()
    (local/'known_hosts').write_text('cos-rust-public-ssh '+hostkey+'\n',encoding='utf-8')
    (local/'wrong_hosts').write_text('cos-rust-public-ssh '+(local/'unauthorized.pub').read_text()+'\n',encoding='utf-8')
    def config(name,identity,known):
        file=local/name
        file.write_text(f'''Host cos-rust-public-ssh
  HostName 127.0.0.1
  User cosfixture
  Port 2222
  HostKeyAlias cos-rust-public-ssh
  IdentityFile "{(local/identity).as_posix()}"
  IdentitiesOnly yes
  UserKnownHostsFile "{(local/known).as_posix()}"
  GlobalKnownHostsFile none
  StrictHostKeyChecking yes
  ProxyCommand "{docker}" exec -i {a.container} /bin/nc 127.0.0.1 2222
''',encoding='utf-8')
        return str(file)
    good=config('good_config','client','known_hosts');bad_host=config('bad_host_config','client','wrong_hosts');bad_auth=config('bad_auth_config','unauthorized','known_hosts')
    run(['docker','exec','-d',a.container,'/usr/sbin/sshd','-D','-e','-f',server+'/sshd_config'])
    try:
        result=run(['cargo','run','--locked','-j','2','-p','cos-tools','--example','qualify_ssh','--',good,bad_host,bad_auth,server+'/project',str(local)],cwd=repo,text=True)
        receipt=json.loads(result.stdout.strip().splitlines()[-1]);receipt.update({'date':'2026-10-17','clientPlatform':os.name,'cargoCommand':'cargo run --locked -j 2 -p cos-tools --example qualify_ssh','externalServer':'NOT RUN; this is an actual isolated loopback SSH server','containerImage':inspect['Image']})
        out=repo/a.output;out.parent.mkdir(parents=True,exist_ok=True);out.write_text(json.dumps(receipt,indent=2)+'\n',encoding='utf-8');print(json.dumps(receipt))
    except subprocess.CalledProcessError as e:
        print(e.stdout or '');print(e.stderr or '');raise
    finally:
        pid_result=subprocess.run(['docker','exec',a.container,'cat',server+'/sshd.pid'],capture_output=True,text=True)
        if pid_result.returncode==0:
            pid=str(int(pid_result.stdout.strip()))
            command=subprocess.run(['docker','exec',a.container,'cat',f'/proc/{pid}/cmdline'],capture_output=True)
            if server.encode() in command.stdout:subprocess.run(['docker','exec',a.container,'kill',pid],check=False,capture_output=True)
