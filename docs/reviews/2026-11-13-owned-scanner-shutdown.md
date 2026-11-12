# Owned scanner shutdown

November 13, 2026 · S-06a · Private synthetic Linux x86_64 input.

Quit now cancels an active local scanner and stops provider work before exit. The native journey completed in 130 ms with both workers active. It preserved all 42 encrypted database tables and the protected key. Tray, Start at login, and sleep/wake behavior remain open.

## Process boundary

The local-log runner adds one-way `ScanCancellation` tokens. Existing public methods remain available. Cancellable methods check the token before launch, during output capture, after parsing, and before the desktop saves a complete result. Consent checks remain separate.

Unix capture uses a nonblocking `UnixStream` pair and one bounded read per loop. Continuous output cannot suppress cancellation or the scan deadline. The runner keeps the owned `Child`, kills only that child, and allows two seconds to reap it. A failure returns `CleanupFailed`. It never kills a process by name or closes a borrowed descriptor. An unrelated process that retains stdout cannot force an unbounded reader-thread join.

The app handles `ExitRequested` through one idempotent coordinator. It signals both worker loops before waiting. Cleanup runs outside the GUI thread and workspace lock. It then requests the final exit. A cleanup failure produces a generic diagnostic and a nonzero exit code. Failed or cancelled reports cannot replace accepted data.

This acceptance covers Linux. The code retains non-cancellable capture on non-Unix platforms and refuses the new cancellable path there before spawning. No supported scanner pin exists on those platforms. Other-platform behavior still needs qualification.

## Executed evidence

| Check | Result |
|---|---|
| Public scanner process test | Cancellation passes for a slow child, an exited child with inherited stdout, and continuous output. A held-output deadline also passes. Pre-cancelled calls start nothing |
| Local-log all-feature checks | Thirty-eight library, thirteen period, fifteen Codex snapshot, and one cancellation test pass |
| Pinned scanner and desktop path | The real pinned scanner check and four desktop local-log checks pass |
| Native Quit | 130 ms with a slow scanner and a pending scheduled provider collection |
| Ownership | Both owned children reaped. External descriptor keeper, its holder, and unrelated sentinel survived |
| Accepted data | All 42 schema-13 tables and the protected key remain unchanged through a read-only encrypted-copy oracle |
| Normal-build guard | Exact fixture override and version are absent from the binary. The reviewed fixture does not execute. Normal Close completes in 136 ms |

The native-test scanner override checks the exact compiled fixture digest. It does not exist in normal builds. The [journey](../../tests/native/owned-shutdown.mjs) uses private homes, D-Bus, Xvfb, and synthetic logs and provider responses. It records source hashes and removes only its owned fixtures. No real provider credentials or histories were read.

The native-test executable has SHA-256 `19620437bd438174e5fdf52117b83172cfc557c446458f9c9f082806a6049bea`. The normal executable has SHA-256 `1711ecd8d556f66ce84de799e6d09a81c12e269c14dec286f7907cf7a4526795`. The read-only oracle has SHA-256 `22484801f2e4824eac5503e181e5e3058633c783038469af4a99feca433fdd5c`.

Private native evidence is in `.recovery/native-runs/2026-11-13-owned-shutdown-r1` and `.recovery/native-runs/2026-11-13-production-scanner-gate-r1`. Metadata-only artifact receipts remain in `.recovery/artifacts`. These results do not establish real-account, packaging, Windows, macOS, tray, or autostart acceptance.
