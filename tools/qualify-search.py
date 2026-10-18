"""Compare an actual service executable with captured source find-handler projections."""
import argparse
import hashlib
import json
import os
import pathlib
import queue
import re
import subprocess
import tempfile
import threading
import urllib.request

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--binary', required=True)
parser.add_argument('--output', required=True)
args = parser.parse_args()
repo = pathlib.Path(__file__).resolve().parents[1]
binary = pathlib.Path(args.binary).resolve()
output = (repo / args.output).resolve()
assert output.is_relative_to(repo / 'audit'), 'Receipt must be inside workspace audit'
assert not output.exists(), 'Preserve prior receipts; select a new output path'
temporary_root = repo / '.test-tmp'
assert temporary_root.resolve().is_relative_to(repo), 'Temporary directory escaped the workspace'
temporary_root.mkdir(exist_ok=True)
corpus_path = repo / 'fixtures/upstream/phase1-6-closure/search-tool.json'
corpus = json.loads(corpus_path.read_text(encoding='utf-8'))
environment = os.environ.copy()
environment.update(TEMP=str(temporary_root), TMP=str(temporary_root), TMPDIR=str(temporary_root))
flags = {'creationflags': 0x08000000} if os.name == 'nt' else {}
checks = []
serial = 0

def normalize(result):
    for item in result.get('content', []):
        if item.get('type') == 'text':
            item['text'] = re.sub(r'elapsed_ms: \d+', 'elapsed_ms: <elapsed>', item['text'])
    return result

with tempfile.TemporaryDirectory(prefix='cos-rust-public-search-', dir=temporary_root) as temporary:
    root = pathlib.Path(temporary)
    profile = root / 'profile'
    projects = {name: root / name for name in ['left', 'right']}
    for directory in projects.values():
        directory.mkdir()
    for path, descriptor in corpus['files'].items():
        name, relative = path.lstrip('/').split('/', 1)
        target = projects[name] / relative
        if 'zeroBytes' in descriptor:
            with target.open('xb') as stream:
                stream.truncate(descriptor['zeroBytes'])
        else:
            target.write_text(descriptor['text'], encoding='utf-8', newline='')
    # This is a filesystem input, not a normalization of the service's output.
    order = [entry.name for entry in projects['left'].iterdir()]
    variant = next(row for row in corpus['fileOrderVariants'] if row['leftEntries'] == order)

    def cli(*command):
        return json.loads(subprocess.check_output([str(binary), '--data-dir', str(profile), *command], env=environment, text=True, encoding='utf-8', **flags))

    cli('info')
    for key, value in [('roots', [{'name': name, 'path': str(path)} for name, path in projects.items()]), ('capabilities.command', False), ('capabilities.read', False), ('capabilities.browse', False), ('capabilities.metadata', False), ('multiAgent.allowUnattributedCalls', True)]:
        cli('config', 'set', key, json.dumps(value))
    with (root / 'service-stderr.log').open('w', encoding='utf-8') as errors:
        process = subprocess.Popen([str(binary), '--data-dir', str(profile), 'serve', '--duration-ms', '10000'], env=environment, stdout=subprocess.PIPE, stderr=errors, text=True, encoding='utf-8', **flags)
        try:
            lines = queue.Queue()
            threading.Thread(target=lambda: lines.put(process.stdout.readline()), daemon=True).start()
            ready = json.loads(lines.get(timeout=10))
            assert ready['event'] == 'ready'

            def rpc(method, params):
                global serial
                serial += 1
                params = dict(params, _meta={'io.modelcontextprotocol/protocolVersion': '2026-07-28', 'io.modelcontextprotocol/clientInfo': {'name': 'public search qualification', 'version': '1'}, 'io.modelcontextprotocol/clientCapabilities': {}})
                headers = {'Accept': 'application/json, text/event-stream', 'Content-Type': 'application/json', 'MCP-Protocol-Version': '2026-07-28', 'Mcp-Method': method, 'x-request-id': 'wfr_public_search_qualification/suffix'}
                if method == 'tools/call':
                    headers['Mcp-Name'] = params['name']
                request = urllib.request.Request(ready['urls']['core'], json.dumps({'jsonrpc': '2.0', 'id': serial, 'method': method, 'params': params}).encode(), headers)
                with urllib.request.urlopen(request, timeout=10) as response:
                    result = json.loads(response.read())
                assert 'error' not in result, result
                return result['result']

            names = [tool['name'] for tool in rpc('tools/list', {})['tools']]
            assert 'find' in names and 'exec_command' not in names and 'read' not in names, names
            checks.append({'name': 'discovery retains independent search authority', 'status': 'PASS'})
            for row in variant['cases']:
                wire = rpc('tools/call', {'name': 'find', 'arguments': row['args']})
                assert wire.pop('resultType') == 'complete', wire
                assert wire.pop('_meta') == {'io.modelcontextprotocol/serverInfo': {'name': 'chat-on-steroids-core', 'version': '2.1.39'}}, wire
                actual = normalize(wire)
                assert actual == row['expected'], (row, actual)
                checks.append({'name': 'source registered find projection', 'args': row['args'], 'status': 'PASS'})
            nested = rpc('tools/call', {'name': 'exec', 'arguments': {'code': "text((await tools.find({query:'needle'})).content[0].text);"}})
            assert nested.get('isError') is not True, nested
            assert len(nested['content']) == 1, nested
            actual = normalize({'content': [nested['content'][0]]})
            assert actual == variant['cases'][0]['expected'], actual
            checks.append({'name': 'isolated code worker calls native find', 'status': 'PASS'})
            process.wait(timeout=15)
            assert process.returncode == 0, (root / 'service-stderr.log').read_text(encoding='utf-8')
            checks.append({'name': 'service settles and exits normally', 'status': 'PASS'})
        finally:
            if process.poll() is None:
                process.kill()
                process.wait(timeout=5)
            process.stdout.close()

receipt = {'status': 'PASS', 'sourceCommit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=repo, text=True).strip(), 'scope': 'Actual service executable, loopback MCP and isolated code worker using disposable public fixtures', 'binarySha256': hashlib.sha256(binary.read_bytes()).hexdigest(), 'binaryBytes': binary.stat().st_size, 'corpus': corpus_path.relative_to(repo).as_posix(), 'corpusSha256': hashlib.sha256(corpus_path.read_bytes()).hexdigest(), 'nativeLeftEntryOrder': order, 'checks': checks, 'personalProfilesUsed': False, 'fullApplicationReady': False}
output.parent.mkdir(parents=True, exist_ok=True)
with output.open('x', encoding='utf-8', newline='\n') as stream:
    stream.write(json.dumps(receipt, indent=2) + '\n')
print(json.dumps({'status': 'PASS', 'checks': len(checks), 'receipt': str(output)}, indent=2))
