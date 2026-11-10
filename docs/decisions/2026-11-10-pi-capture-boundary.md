# Pi capture and publication boundary

L-1f11j, November 10, 2026. Status: captured-record boundary selected for local
usage under [SC-025](../srs-clarifications.md#sc-025-captured-local-records-and-producer-completion).
Producer-completion publication is a tested protocol experiment, not an
application feature or an enablement prerequisite. Pi remains disabled pending
its remaining candidate integration and release checks.

## Decision

A successful Pi snapshot describes complete, validated records captured from
consented roots. It does not say that Pi finished every writer operation, that
all generated usage reached disk, or that the source became idle. The required
daily and session reports use the same captured entries. A failed or changed
read returns unavailable and preserves accepted history.

The [L-1f11i investigation](../reviews/2026-11-10-pi-producer-qualification.md)
showed an empty report after a header and 17 tokens after a complete usage record.
Pi then completed its buffered write with 137 tokens. The observations are
compatible: the scanner read all complete records visible in each capture.
The earlier investigation incorrectly promoted the absence of producer-completion
proof into a failure of ordinary capture. SC-025 corrects that interpretation
without changing the SRS, recorded results, or production behavior.

The pinned producer exposes no completion record for its buffered operations.
A stronger guarantee therefore needs producer cooperation. OpenUsage will not
copy raw logs into a new store, inspect other processes, delay scans to guess
completion, or add an unapproved product requirement to obtain that guarantee.
OU-LOG-003 still permits storage of summaries only.

## Evaluated alternatives

| Approach | What it can establish | Limit |
|---|---|---|
| Existing bounded reader and one captured entry set | Validated on-disk observations for the required reports | Does not prove writer completion |
| Reader-only advisory lock | Excludes participating lock holders | The unchanged producer ignores it |
| Atomic replacement alone | Readers see the previous or replacement file | Does not prove that the publisher finished its logical operation |
| Producer publication plus a completion record and shared lock | A participating publisher accepted the exact named bytes | Requires a new producer interface and reader integration |

Linux [rename](https://man7.org/linux/man-pages/man2/rename.2.html) atomically
replaces a destination on the same mounted filesystem. Linux
[flock](https://man7.org/linux/man-pages/man2/flock.2.html) is advisory on the local
filesystem used here, so an unrelated writer can ignore it. File synchronization
alone does not persist the directory entry; the
[fsync](https://man7.org/linux/man-pages/man2/fsync.2.html) contract also requires
directory synchronization for that step. These rules inform the experiment.
They do not establish portability, crash durability, or complete producer support.

## Protocol experiment

The reference code lives only in `tests/tools/pi_publication_probe.py`.
It uses one private session file, a stable lock identity supplied by the fixture
owner, and a versioned completion record containing the file's SHA-256 digest.
It is not a reusable storage service or production implementation.

The producer fixture prepares a complete file outside the directory selected
for scanning. It must finish successfully before publication. An exclusive
lease covers the write. Before replacement, the publisher checks that the
previous committed revision still matches its expected base. It then replaces
the data file, synchronizes the directory, and publishes the matching completion
record through another atomic replacement.

A reader acquires a shared lease without waiting. It checks the completion
record, file membership, and byte digest before and after the actual candidate
scanner runs. A busy lease, changed revision, unknown record, missing file, or
unmatched digest returns unavailable. No age threshold or automatic retry is
part of this protocol.

The probe is deliberately narrow. It assumes a private, owned publication
directory and trusted participating publishers. A checksum is not proof of
publisher identity. Its stale-base check does not protect against every
nonparticipating writer racing the final replacement. It does not support
multiple sessions, multiple roots, automatic migration, or crash recovery.
Normal Pi log capture does not depend on this optional interface.

## Executed evidence

Ten protocol tests passed across 21 controlled cases on synthetic Linux.
The first reader-only-lock regression failed at both paused-write points.
Adding completion-record checks made those cases return unavailable.

| Controlled case | Observed outcome |
|---|---|
| Unchanged producer ignores a reader lock | Raw scanner still returns the captured prefix |
| Source has no matching completion record | Protocol reader returns unavailable |
| Participating writer pauses after header or usage | Reader returns unavailable while the exclusive lease is held |
| Publication completes | Reader accepts the full 137-token file |
| Private writer terminates or preparation fails | Previous 120-token publication remains intact |
| Publisher process exits after data replacement | Unmatched completion record prevents acceptance |
| Publisher process exits after the completion record | Full 137-token publication remains readable |
| Stale writer tries to replace a newer revision | Replacement is refused and 203 tokens remain |
| Data changes during collection | The result is discarded |
| Missing, repeated, unknown, extra, linked, or replaced control data | Reader returns unavailable |
| Empty input, measured zero, and absent categories | Source bytes and distinct report states remain intact |

The producer runs from the unchanged pinned source in private offline namespaces.
The scanner is the unchanged observation candidate. The earlier producer fixture
owns their pins and runtime scope. All source and candidate digests still match.
The experiment copies only synthetic, producer-owned fixture files. No raw-log
copying path was added to OpenUsage.

The reviewed verification plan passed staged whitespace and all Python tool
tests. The current native build passed local-log consent with the production
scanner and private synthetic input. No Rust or renderer source changed, so
the plan selected neither workspace Rust tests nor renderer checks. The
publication experiment ran on Linux tmpfs. No application data boundary changed.

The only failing check was the intended reader-only-lock regression. Later
controlled publisher exits are fault cases, not interrupted verification runs.
No unexpected timeout, interruption, or automatic retry occurred.
Full Rust, renderer, other native journeys, installed Pi, real logs, power-loss
recovery, network filesystems, Windows, macOS, and hosted CI did not run.

## Next boundary

L-1f11k will exercise the real Pi candidate through the grouped Rust scanner
and storage path under SC-025. It will cover valid prefixes, zero, absent
measurements, completed writes, and rejected incomplete records. Pi enablement,
production pin changes, and the optional producer-publication interface remain
outside that slice until their own checks and decisions permit them.
