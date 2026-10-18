"""Run local checks with temporary files on the workspace drive and two build jobs."""
import argparse
import os
import pathlib
import shutil
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--strict', action='store_true', help='Also check formatting and all-target Clippy')
args = parser.parse_args()
repo = pathlib.Path(__file__).resolve().parents[1]
temporary = repo / '.test-tmp'
if not temporary.resolve().is_relative_to(repo):
    raise RuntimeError('Test temporary directory escaped the workspace')
if shutil.disk_usage(repo).free < 8 * 1024**3:
    raise RuntimeError('Checks require 8 GiB free on the workspace drive')
temporary.mkdir(exist_ok=True)
environment = os.environ.copy()
environment.update(TEMP=str(temporary), TMP=str(temporary), TMPDIR=str(temporary))
environment.setdefault('CARGO_TARGET_DIR', str(repo / 'target'))
print('Test temporary files: ' + str(temporary), flush=True)
print('Build cache: ' + environment['CARGO_TARGET_DIR'], flush=True)
commands = [['test', '--workspace', '--locked', '-j2']]
if args.strict:
    commands.insert(0, ['fmt', '--all', '--', '--check'])
    commands.append(['clippy', '--workspace', '--all-targets', '--locked', '-j2', '--', '-D', 'warnings'])
for command in commands:
    result = subprocess.run(['cargo', *command], cwd=repo, env=environment)
    if result.returncode:
        raise SystemExit(result.returncode)
