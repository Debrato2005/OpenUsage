"""Create a private 1 GiB Claude Code fixture for the native consent benchmark."""
import copy
from datetime import datetime, timedelta, timezone
import hashlib
import json
from pathlib import Path
import sys

from pi_scale import MIB, pad, row
from test_ccusage_formats import ROOT


def hashes(folder):
    result = {}
    for path in sorted(folder.rglob('*.jsonl')):
        with path.open('rb') as stream:
            result[str(path.relative_to(folder))] = hashlib.file_digest(stream, 'sha256').hexdigest()
    return result


def create(folder):
    fixture = ROOT / 'crates/openusage-locallogs/tests/fixtures/home/.claude/projects/-home-dev-alpha/11111111-aaaa-4aaa-8aaa-111111111111.jsonl'
    template = json.loads(fixture.read_text().splitlines()[0])
    stamp = (datetime.now(timezone.utc) - timedelta(days=1)).strftime('%Y-%m-%dT12:00:00.000Z')
    project = folder / '.claude/projects/-synthetic-scale'
    project.mkdir(parents=True)
    usage = template['message']['usage']
    each = sum(usage[field] for field in ('input_tokens', 'output_tokens', 'cache_creation_input_tokens', 'cache_read_input_tokens'))
    for index in range(64):
        identifier = f'00000000-0000-4000-8000-{index:012x}'
        record = copy.deepcopy(template)
        record.update(sessionId=identifier, timestamp=stamp, cwd='/synthetic/scale')
        record['message']['content'] = [{'type': 'text', 'text': 'x' * 64000}]
        path = project / f'{identifier}.jsonl'
        with path.open('xb') as stream:
            for offset in range(256):
                key = f'{index}-{offset}'
                record['uuid'] = key
                record['requestId'] = f'request-{key}'
                record['message']['id'] = f'message-{key}'
                stream.write(row(record))
        pad(path, 16 * MIB)
    size = sum(path.stat().st_size for path in project.iterdir())
    assert size == 1024 * MIB
    return {'bytes': size, 'files': 64, 'records': 64 * 256,
            'tokens': each * 64 * 256, 'sessionTokens': each * 256,
            'date': stamp[:10], 'hashes': hashes(project)}


if __name__ == '__main__':
    folder = Path(sys.argv[2]).resolve(strict=True)
    if sys.argv[1] == 'create':
        print(json.dumps(create(folder)))
    elif sys.argv[1] == 'hashes':
        print(json.dumps(hashes(folder / '.claude/projects/-synthetic-scale')))
    else:
        raise SystemExit('Select create or hashes')
