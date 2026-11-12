#!/usr/bin/python3
"""Private cancellation fixture. No network, credentials, or real tool logs."""
import array
import json
import os
from pathlib import Path
import socket
import signal
import subprocess
import sys
import time


def record(folder, kind, **fields):
    path = folder / 'synthetic-scanner-events.jsonl'
    descriptor = os.open(path, os.O_WRONLY | os.O_APPEND | os.O_CREAT, 0o600)
    with os.fdopen(descriptor, 'a', encoding='utf8') as output:
        output.write(json.dumps({'kind': kind, 'pid': os.getpid(), **fields}) + '\n')


if sys.argv[1:2] == ['--hold']:
    time.sleep(300)
    raise SystemExit(0)

if sys.argv[1:2] == ['--pipe-keeper']:
    folder = Path(sys.argv[2])
    server = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    server.bind(str(folder / 'keeper.sock'))
    server.listen(1)
    record(folder, 'keeper-ready')
    connection, _ = server.accept()
    _, ancillary, _, _ = connection.recvmsg(1, socket.CMSG_SPACE(array.array('i').itemsize))
    descriptors = array.array('i')
    for level, kind, data in ancillary:
        if level == socket.SOL_SOCKET and kind == socket.SCM_RIGHTS:
            descriptors.frombytes(data[:len(data) - len(data) % descriptors.itemsize])
    assert len(descriptors) == 1
    try:
        holder = subprocess.Popen([sys.executable, __file__, '--hold'], stdin=subprocess.DEVNULL,
                                  stdout=descriptors[0], stderr=subprocess.DEVNULL)
    finally:
        os.close(descriptors[0])
    def stop_keeper(_signal, _frame):
        raise SystemExit(0)
    signal.signal(signal.SIGTERM, stop_keeper)
    record(folder, 'pipe-held', holderPid=holder.pid)
    connection.sendall(b'1')
    connection.close()
    try:
        holder.wait(timeout=300)
    finally:
        holder.terminate()
        holder.wait(timeout=5)
    raise SystemExit(0)

folder = Path(os.environ['CLAUDE_CONFIG_DIR'])
assert folder.is_absolute() and ',' not in str(folder)
assert not set(os.environ) - {'HOME', 'CLAUDE_CONFIG_DIR', 'TZ', 'NO_COLOR', 'LC_CTYPE'}
mode = (folder / 'synthetic-scanner-mode').read_text(encoding='utf8').strip()
assert mode in {'seed', 'slow', 'held-exit', 'flood'}
record(folder, 'start', mode=mode, arguments=sys.argv[1:])
if mode in {'slow', 'held-exit'}:
    connection = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    connection.connect(str(folder / 'keeper.sock'))
    connection.sendmsg([b'1'], [(socket.SOL_SOCKET, socket.SCM_RIGHTS, array.array('i', [1]))])
    assert connection.recv(1) == b'1'
    connection.close()
    record(folder, 'inherited-pipe-ready')
    if mode == 'held-exit':
        raise SystemExit(0)
    time.sleep(120)
elif mode == 'flood':
    os.write(1, b'x' * 65536)
    record(folder, 'flood-started')
    while True:
        os.write(1, b'x' * 65536)
        time.sleep(0.001)
else:
    report = 'session' if 'session' in sys.argv else 'blocks' if 'blocks' in sys.argv else 'daily'
    sys.stdout.buffer.write((folder / ('synthetic-scanner-' + report + '.json')).read_bytes())
    sys.stdout.buffer.flush()
