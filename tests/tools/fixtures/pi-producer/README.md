# Pinned Pi session writer fixture

This fixture executes the unchanged session writer from Pi
`0d1793c2bc7139d1df4547fa3c0aeacb233e0534`.
It requires Linux, bubblewrap, Node 22.23.2, and the explicit local Pi scanner
candidate. Controlled scanner schedules also require x86_64 and permission to
trace a child process. Tests make no download and use no real account.

## Inputs

Get the [pinned source archive](https://codeload.github.com/earendil-works/pi/tar.gz/0d1793c2bc7139d1df4547fa3c0aeacb233e0534).
Its SHA-256 is
`ceabe66cf9ebc8780c912074dca7d97706627494e24f6f21b1c9040cf653c23f`.
The source's `package-lock.json` pins six required packages: cross-spawn 7.0.6,
path-key 3.1.1, shebang-command 2.0.0, shebang-regex 3.0.0, which 2.0.2, and
isexe 2.0.0. Place their registry contents in that source's `node_modules`.
Check each lockfile integrity value. Run no dependency lifecycle script.
`pin.json` checks 48 source and dependency files before execution.

Set `OPENUSAGE_PI_PRODUCER_SOURCE` to this source directory.
Set `OPENUSAGE_CCUSAGE_PI_OBSERVATION_CANDIDATE` and its `_SHA256` variable to
the observation candidate recorded in `pin.json`. Use the project root:

```sh
python3 -m unittest discover -s tests/tools -p test_ccusage_pi_producer.py
```

The fixture refuses changed source or candidate bytes. An absent explicit input
skips these optional local checks. It does not establish qualification elsewhere.

## Execution boundary

`loader.mjs` resolves the writer's two value imports to the original
`getCurrentSystemMessage` and `uuidv7` implementations. This avoids loading
unrelated provider modules. Other writer imports remain unchanged.
`producer.mjs` controls the synthetic clock and calls the real public session
methods. An optional schedule pauses after an unchanged filesystem write.
It resumes only after the scanner finishes. No writer source is patched.

The writer runs with isolated mounts, a private home, and no network.
`pi_scan_schedule.py` traces only its own scanner child. It pauses at a selected
source open or read, executes one producer operation, then resumes collection.
It changes no syscall result. Every process has a deadline and cleanup.

The recorded reports normalize temporary paths, the random branch ID, and only
float cost precision. Every count remains exact. OpenUsage keeps these costs
unavailable. The two paused-writer cases remain failed completeness evidence.
See the [review](../../../../docs/reviews/2026-11-10-pi-producer-qualification.md).
