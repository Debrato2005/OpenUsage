"""Run the pinned Codex producer with a private local Responses fixture."""
import argparse
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import platform
import queue
import signal
import socket
import subprocess
import tempfile
import threading
import time


ROOT = Path(__file__).resolve().parent.parent.parent
PIN = ROOT / 'crates/openusage-codex/provider-sources/codex-0.163.0-linux-x86_64.json'
BINARY = ROOT / 'apps/desktop/src-tauri/binaries/openusage-codex-x86_64-unknown-linux-gnu'
USAGE = {'input_tokens': 4000, 'input_tokens_details': {'cached_tokens': 1000},
         'output_tokens': 600, 'output_tokens_details': {'reasoning_tokens': 200},
         'total_tokens': 4600}


def sha256(path):
    with Path(path).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def sandbox_command(root, mode):
    return ['bwrap', '--die-with-parent', '--unshare-net', '--unshare-pid',
            '--ro-bind', '/usr', '/usr', '--symlink', 'usr/lib', '/lib',
            '--symlink', 'usr/lib', '/lib64', '--symlink', 'usr/bin', '/bin',
            '--proc', '/proc', '--dev', '/dev', '--tmpfs', '/tmp',
            '--ro-bind', str(BINARY), '/producer/codex',
            '--ro-bind', str(Path(__file__).resolve()), '/harness/codex_producer.py',
            '--bind', str(root), '/work', '--clearenv',
            '--setenv', 'HOME', '/work/home', '--setenv', 'CODEX_HOME', '/work/codex',
            '--setenv', 'XDG_CONFIG_HOME', '/work/xdg/config',
            '--setenv', 'XDG_DATA_HOME', '/work/xdg/data',
            '--setenv', 'XDG_CACHE_HOME', '/work/xdg/cache',
            '--setenv', 'XDG_STATE_HOME', '/work/xdg/state',
            '--setenv', 'PATH', '/usr/bin', '--setenv', 'TZ', 'UTC',
            '--chdir', '/work/project', '/usr/bin/python3',
            '/harness/codex_producer.py', '--inside', mode]


def response_events(response_id, usage):
    completed = {'id': response_id}
    if usage is not None:
        completed['usage'] = usage
    return [
        {'type': 'response.created', 'response': {'id': response_id}},
        {'type': 'response.output_item.done', 'item': {
            'type': 'message', 'role': 'assistant', 'id': 'message-' + response_id,
            'content': [{'type': 'output_text', 'text': 'Synthetic response.'}]}},
        {'type': 'response.completed', 'response': completed},
    ]


class AppServer:
    def __init__(self, root):
        self.events = []
        self.inbox = queue.Queue()
        self.next_id = 0
        self.stderr = (root / 'app-server.stderr').open('ab')
        self.process = subprocess.Popen(
            ['/producer/codex', 'app-server', '--listen', 'stdio://'],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=self.stderr,
            text=True, bufsize=1)
        self.reader = threading.Thread(target=self._read, daemon=True)
        self.reader.start()

    def _read(self):
        for line in self.process.stdout:
            self.inbox.put(json.loads(line))
        self.inbox.put(None)

    def send(self, value):
        self.process.stdin.write(json.dumps(value) + '\n')
        self.process.stdin.flush()

    def until(self, predicate, timeout=20):
        deadline = time.monotonic() + timeout
        while True:
            event = self.inbox.get(timeout=max(0, deadline - time.monotonic()))
            if event is None:
                raise RuntimeError('The app server stopped before the expected event.')
            self.events.append(event)
            if predicate(event):
                return event

    def call(self, method, params):
        self.next_id += 1
        request_id = self.next_id
        self.send({'id': request_id, 'method': method, 'params': params})
        event = self.until(lambda row: row.get('id') == request_id)
        if 'error' in event:
            raise RuntimeError(json.dumps(event))
        return event['result']

    def turn(self, thread_id, text):
        result = self.call('turn/start', {'threadId': thread_id,
                           'input': [{'type': 'text', 'text': text}]})
        completed = self.until(lambda row: row.get('method') == 'turn/completed'
                               and row['params']['threadId'] == thread_id)
        if completed['params']['turn']['status'] != 'completed':
            raise RuntimeError(json.dumps(completed))
        return result['turn']['id']

    def close(self):
        self.process.stdin.close()
        try:
            self.process.wait(timeout=8)
        except subprocess.TimeoutExpired:
            self.process.kill()
            self.process.wait(timeout=3)
            raise RuntimeError('The app server did not stop after input closed.')
        finally:
            self.reader.join(timeout=1)
            self.process.stdout.close()
            self.stderr.close()


def inside(mode):
    root = Path('/work')
    evidence = {'mode': mode, 'requests': [], 'threads': {}, 'turns': {}, 'checkpoints': {}}
    evidence['sandbox'] = {'networkNamespace': os.readlink('/proc/self/ns/net'),
                           'interfaces': socket.if_nameindex(),
                           'routes': Path('/proc/net/route').read_text(),
                           'home': os.environ['HOME'], 'codexHome': os.environ['CODEX_HOME']}
    if [name for _, name in socket.if_nameindex()] != ['lo']:
        raise RuntimeError('The fixture requires an isolated loopback interface.')
    if len(Path('/proc/net/route').read_text().splitlines()) != 1:
        raise RuntimeError('The fixture must not have an outbound route.')
    sequence = []

    class Handler(BaseHTTPRequestHandler):
        def do_POST(self):
            body = self.rfile.read(int(self.headers['Content-Length']))
            request = json.loads(body)
            evidence['requests'].append({'path': self.path,
                                         'bodySha256': hashlib.sha256(body).hexdigest(),
                                         'model': request.get('model'),
                                         'authorizationPresent': 'Authorization' in self.headers})
            if self.path != '/v1/responses' or not sequence:
                self.send_error(400)
                return
            response_id, usage = sequence.pop(0)
            evidence['requests'][-1].update(responseId=response_id, suppliedUsage=usage)
            payload = ''.join('event: ' + row['type'] + '\ndata: ' + json.dumps(row)
                              + '\n\n' for row in response_events(response_id, usage)).encode()
            self.send_response(200)
            self.send_header('Content-Type', 'text/event-stream')
            self.send_header('Content-Length', str(len(payload)))
            self.end_headers()
            self.wfile.write(payload)

        def log_message(self, *args):
            pass

    server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
    server_thread = threading.Thread(target=server.serve_forever, daemon=True)
    server_thread.start()
    (root / 'codex/config.toml').write_text(
        'model = "gpt-5"\nmodel_provider = "synthetic"\n'
        'approval_policy = "never"\nsandbox_mode = "read-only"\n'
        '[model_providers.synthetic]\nname = "Synthetic local fixture"\n'
        f'base_url = "http://127.0.0.1:{server.server_port}/v1"\n'
        'wire_api = "responses"\nrequires_openai_auth = false\n'
        'request_max_retries = 0\nstream_max_retries = 0\nstream_idle_timeout_ms = 5000\n')
    app = None
    try:
        def initialize():
            current = AppServer(root)
            evidence['initialize'] = current.call('initialize', {
                'clientInfo': {'name': 'openusage_synthetic_test', 'version': '1.0.0'},
                'capabilities': {'experimentalApi': True}})
            current.send({'method': 'initialized', 'params': {}})
            return current

        app = initialize()
        start_params = {'model': 'gpt-5', 'modelProvider': 'synthetic',
                        'cwd': '/work/project', 'approvalPolicy': 'never',
                        'sandbox': 'read-only', 'baseInstructions': 'Reply with synthetic text.'}
        first = app.call('thread/start', start_params)['thread']['id']
        evidence['threads']['first'] = first
        sequence.append(('response-first', USAGE))
        evidence['turns']['first'] = app.turn(first, 'Synthetic first turn.')
        if mode == 'contract':
            evidence['firstProcessEvents'] = app.events
            app.close()
            app = None
            evidence['checkpoints']['beforeResume'] = {
                str(path.relative_to(root)): path.read_text()
                for path in sorted((root / 'codex/sessions').rglob('*.jsonl'))}
            app = initialize()
            resumed = app.call('thread/resume', {'threadId': first, 'excludeTurns': True})
            evidence['threads']['resumed'] = resumed['thread']['id']
            half = {'input_tokens': 2000, 'input_tokens_details': {'cached_tokens': 500},
                    'output_tokens': 300, 'output_tokens_details': {'reasoning_tokens': 100},
                    'total_tokens': 2300}
            sequence.append(('response-resumed', half))
            evidence['turns']['resumed'] = app.turn(first, 'Synthetic resumed turn.')
            second = app.call('thread/start', start_params)['thread']['id']
            evidence['threads']['second'] = second
            sequence.append(('response-second', USAGE))
            evidence['turns']['second'] = app.turn(second, 'Synthetic equal-usage thread.')
            fork = app.call('thread/fork', {'threadId': first, 'excludeTurns': True})
            evidence['forkResponse'] = fork['thread']
            child = fork['thread']['id']
            evidence['threads']['fork'] = child
            quarter = {'input_tokens': 1000, 'input_tokens_details': {'cached_tokens': 250},
                       'output_tokens': 150, 'output_tokens_details': {'reasoning_tokens': 50},
                       'total_tokens': 1150}
            sequence.append(('response-fork', quarter))
            evidence['turns']['fork'] = app.turn(child, 'Synthetic fork turn.')
            sequence.append(('response-missing', None))
            evidence['turns']['missing'] = app.turn(child, 'Synthetic missing usage.')
            zero = {'input_tokens': 0, 'input_tokens_details': {'cached_tokens': 0},
                    'output_tokens': 0, 'output_tokens_details': {'reasoning_tokens': 0},
                    'total_tokens': 0}
            sequence.append(('response-zero', zero))
            evidence['turns']['zero'] = app.turn(child, 'Synthetic measured zero.')
        evidence['pendingResponses'] = len(sequence)
        evidence['status'] = 'completed'
    except BaseException as error:
        evidence['status'] = 'failed'
        evidence['error'] = repr(error)
        raise
    finally:
        try:
            if app:
                evidence['events'] = app.events
                app.close()
        finally:
            server.shutdown()
            server.server_close()
            (root / 'evidence.json').write_text(json.dumps(evidence, indent=2) + '\n')


def run(output, mode='basic'):
    output = Path(output).resolve()
    if output.exists():
        raise FileExistsError('The evidence path already exists.')
    pin = json.loads(PIN.read_text())
    before = sha256(BINARY)
    if before != pin['executableSha256'] or BINARY.stat().st_size != pin['executableBytes']:
        raise ValueError('The producer differs from its source pin.')
    receipt = {'producerSha256Before': before, 'pinSha256': sha256(PIN),
               'hostNetworkNamespace': os.readlink('/proc/self/ns/net'),
               'harnessSha256Before': sha256(__file__), 'pythonVersion': platform.python_version(),
               'platform': platform.platform(),
               'producingCommit': subprocess.check_output(
                   ['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True, timeout=5).strip(),
               'dirtySource': subprocess.check_output(
                   ['git', 'status', '--porcelain'], cwd=ROOT, text=True, timeout=5).splitlines()}
    with tempfile.TemporaryDirectory(prefix='openusage-codex-producer-') as directory:
        root = Path(directory)
        for name in ['home', 'codex', 'project', 'xdg/config', 'xdg/cache', 'xdg/data', 'xdg/state']:
            (root / name).mkdir(parents=True, mode=0o700)
        command = sandbox_command(root, mode)
        receipt['command'] = command
        started = time.monotonic()
        process = subprocess.Popen(command, stdin=subprocess.DEVNULL,
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                   start_new_session=True)
        try:
            stdout, stderr = process.communicate(timeout=75)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            stdout, stderr = process.communicate(timeout=5)
            receipt['timeout'] = True
        receipt.update(returncode=process.returncode, elapsedSeconds=time.monotonic() - started,
                       stdout=stdout.decode(errors='replace'), stderr=stderr.decode(errors='replace'))
        for relative in ['evidence.json', 'app-server.stderr']:
            path = root / relative
            if path.exists():
                receipt[relative] = path.read_text()
        receipt['rollouts'] = {str(path.relative_to(root)): path.read_text()
                               for path in sorted((root / 'codex/sessions').rglob('*.jsonl'))}
    receipt['producerSha256After'] = sha256(BINARY)
    receipt['pinSha256After'] = sha256(PIN)
    receipt['harnessSha256After'] = sha256(__file__)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(receipt, indent=2) + '\n')
    if receipt['producerSha256After'] != before or receipt['pinSha256After'] != receipt['pinSha256']:
        raise AssertionError('The producer or source pin changed.')
    if receipt['harnessSha256After'] != receipt['harnessSha256Before']:
        raise AssertionError('The producer harness changed during execution.')
    if receipt['returncode'] != 0 or receipt.get('timeout'):
        raise RuntimeError('The producer fixture failed. Read the saved evidence.')
    evidence = json.loads(receipt['evidence.json'])
    if evidence['sandbox']['networkNamespace'] == receipt['hostNetworkNamespace']:
        raise AssertionError('The producer shared the host network namespace.')
    return receipt


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--inside', choices=['basic', 'contract'])
    parser.add_argument('--mode', choices=['basic', 'contract'], default='basic')
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    if args.inside:
        inside(args.inside)
    elif args.output:
        result = run(args.output, args.mode)
        print(json.dumps({'returncode': result['returncode'], 'rollouts': len(result['rollouts']),
                          'output': str(args.output)}))
    else:
        parser.error('Specify --output.')
