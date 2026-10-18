"""Qualify release CLI session projections with public source fixtures and fresh profiles."""
import argparse
import hashlib
import json
import os
import pathlib
import subprocess
import tempfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--binary', required=True)
parser.add_argument('--output', required=True)
args = parser.parse_args()
repo = pathlib.Path(__file__).resolve().parents[1]
binary = pathlib.Path(args.binary).resolve()
output = (repo / args.output).resolve()
assert output.is_relative_to(repo / 'audit') and not output.exists(), 'Use a new workspace audit receipt'
temporary_root = repo / '.test-tmp'
assert temporary_root.resolve().is_relative_to(repo), 'Temporary root escaped the workspace'
temporary_root.mkdir(exist_ok=True)
environment = os.environ.copy()
environment.update(TEMP=str(temporary_root), TMP=str(temporary_root), TMPDIR=str(temporary_root))
flags = {'creationflags': 0x08000000} if os.name == 'nt' else {}
corpus_paths = [repo / 'fixtures/upstream/phase1-6-closure' / (name + '.json') for name in ['session-content', 'session-model-selection']]
content, selection = [json.loads(path.read_text(encoding='utf-8')) for path in corpus_paths]
checks = []
api_only = []

def cli(profile, *command, expect_error=None):
    result = subprocess.run([str(binary), '--data-dir', str(profile), *command], capture_output=True, text=True, encoding='utf-8', env=environment, timeout=30, **flags)
    if expect_error is not None:
        assert result.returncode != 0 and expect_error in result.stderr, (command, result)
        return None
    assert result.returncode == 0, (command, result.stderr)
    return json.loads(result.stdout)

def projection(summary):
    return {key: summary[key] for key in ['title', 'titleSource', 'autoTitle', 'origin', 'selectedModel', 'conversationId', 'chatIds'] if key in summary}

with tempfile.TemporaryDirectory(prefix='cos-rust-public-session-', dir=temporary_root) as temporary:
    root = pathlib.Path(temporary)
    assert root.resolve().parent == temporary_root.resolve(), 'Public profile escaped its owned parent'
    for case in content['exports']:
        profile = root / case['name']
        cli(profile, 'info')
        session_id = 'session-export-public'
        cli(profile, 'sessions', 'create', case['title'], '--id', session_id)
        session = profile / 'sessions' / session_id
        assets = session / 'assets'
        assets.mkdir(exist_ok=True)
        for name, text in case['overflow'].items():
            assert name == hashlib.sha256(text.encode()).hexdigest()[:32] + '.txt'
            (assets / name).write_text(text, encoding='utf-8', newline='')
        (session / 'events.jsonl').write_text(''.join(json.dumps(event, ensure_ascii=False) + '\n' for event in case['events']), encoding='utf-8', newline='')
        for expected in case['outcomes']:
            command = ['export', session_id, '--format', 'md', '--scope', expected['scope'], '--turn', 'turn-public']
            if 'error' in expected:
                cli(profile, *command, expect_error=expected['error'])
            else:
                assert cli(profile, *command)['text'] == expected['text'], (case['name'], expected['scope'])
            checks.append({'name': 'source export text projection', 'case': case['name'], 'scope': expected['scope'], 'status': 'PASS'})

    profile = root / 'selection'
    cli(profile, 'info')
    cli(profile, 'config', 'set', 'sessions.record', 'true')
    session_id = selection['id']
    cli(profile, 'sessions', 'create', '--id', session_id, '--conversation', 'conversation-public')
    current_conversation = 'conversation-public'
    retired = set()
    for index, step in enumerate(selection['selection']):
        operation = step['operation']
        kind = operation[0]
        if kind == 'observe':
            source = operation[1]
            if 'nonFinite' in source:
                api_only.append({'step': index, 'reason': 'Non-finite Numbers are covered by the native API corpus; JSON cannot represent them'})
                continue
            if source['conversationId'] != current_conversation and source['conversationId'] not in retired:
                api_only.append({'step': index, 'reason': 'The store API names a session explicitly; an unknown recorder conversation would name a different session'})
                continue
            event = {'kind': 'model_selection', 'model': source['model'], 'time': source['observedAt']}
            if 'reasoningEffort' in source:
                event['reasoningEffort'] = source['reasoningEffort']
            payload = root / 'public-observation.json'
            payload.write_text(json.dumps([event]), encoding='utf-8')
            cli(profile, 'observe', source['conversationId'], str(payload))
        elif kind == 'rebind':
            source = operation[1]
            cli(profile, 'rebind', session_id, source['from'], source['to'])
            retired.add(current_conversation)
            current_conversation = source['to']
        else:
            assert kind == 'restart'
            # Every CLI invocation already opens a fresh process/profile; read it once more.
        rows = cli(profile, 'sessions', 'list')['sessions']
        assert len(rows) == 1, rows
        assert projection(rows[0]) == projection(step['expected']), (index, rows[0], step['expected'])
        checks.append({'name': 'source selection metadata projection', 'step': index, 'operation': kind, 'status': 'PASS'})

receipt = {'status': 'PASS', 'sourceCommit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=repo, text=True).strip(), 'scope': 'Actual release CLI using new marked public profiles; export text and named metadata fields only', 'binarySha256': hashlib.sha256(binary.read_bytes()).hexdigest(), 'binaryBytes': binary.stat().st_size, 'corpora': {path.relative_to(repo).as_posix(): hashlib.sha256(path.read_bytes()).hexdigest() for path in corpus_paths}, 'checks': checks, 'selectionApiOnlyCases': api_only, 'nativeApiCorpusSteps': 17, 'projectionFields': ['title', 'titleSource', 'autoTitle', 'origin', 'selectedModel', 'conversationId', 'chatIds'], 'nativeClipboardDialog': 'NOT RUN', 'personalProfilesUsed': False, 'fullApplicationReady': False}
output.parent.mkdir(parents=True, exist_ok=True)
with output.open('x', encoding='utf-8', newline='\n') as stream:
    stream.write(json.dumps(receipt, indent=2) + '\n')
print(json.dumps({'status': 'PASS', 'checks': len(checks), 'apiOnlyCases': len(api_only), 'receipt': str(output)}, indent=2))
