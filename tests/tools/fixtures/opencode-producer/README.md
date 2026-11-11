# Pinned OpenCode storage-schema fixture

`pin.json` records fourteen official source files from OpenCode v1.19.5,
commit `810c0eba1a7348036f8bdf61e6632d0c36c6876c`.
The source uses the MIT license. This fixture executes generated table
declarations with Python SQLite and fills them with synthetic message payloads.
It does not execute the OpenCode CLI, SDK, event projector, or message updater.

The source establishes the WAL default, current table names, token fields,
message shapes, and channel filename rules. Full producer execution remains a
separate gate. No real account or credential home takes part.

Set `OPENUSAGE_OPENCODE_PRODUCER_SOURCE` to the exact source checkout.
Set `OPENUSAGE_CCUSAGE_OPENCODE_WAL_CANDIDATE` and its `_SHA256` variable to
the explicit [WAL candidate](../../../../docs/reviews/2026-11-11-opencode-wal-candidate.md).
Run from the project root:

```sh
python3 -m unittest discover -s tests/tools -p test_ccusage_opencode_wal.py -v
```

The optional test checks every source digest before it extracts the three
generated table declarations. It checks completed assistant usage, a user peer,
an unfinished assistant, and an unsupported metadata message. Missing explicit
source inputs skip this source-specific test. WAL capture tests remain separate.
