# Synthetic executable startup race

Date: November 10, 2026. R-1 uses private temporary homes and synthetic providers on Linux. No real logs, accounts, or credential homes were inspected.

## Evidence and cause

L-1f10c captured `StartFailed` before the cleanup-fault test reached its intended failure. It did not capture that launch's OS error. R-1 added temporary test-only diagnostics for process and reader-thread creation. In a bounded experiment of at most 15 full-crate samples, the twelfth sample failed. The startup-routing test reported process-spawn error `ExecutableFileBusy`, OS error 26 (`ETXTBSY`). The first eleven samples passed. The experiment stopped at that failure.

Both fixtures wrote a new executable immediately before launch. Other tests could fork while its writable file handle was open. A fork can retain that handle until it executes, even after the writer closes its own copy. Linux rejects execution while a process holds the file open for writing. [Linux execve manual](https://man7.org/linux/man-pages/man2/execve.2.html)

The Rust issue report describes this same race and a file-lock solution. A lock belongs to the open file description shared by inherited descriptors. The writer takes an exclusive lock, closes its descriptor, then opens a read-only descriptor and waits for a shared lock. This waits for inherited writable handles to close. Later forks can inherit only the read-only handle. [Rust issue 114554](https://github.com/rust-lang/rust/issues/114554)

The captured error, fixture source, and OS/Rust sources support this cause for the reproduced startup-routing failure. The shared fixture writer also fixes that exposure in the cleanup test. The original cleanup failure's OS cause and the earlier L-1f10b workspace failure remain unconfirmed.

## Change

`test_executable` applies that lock sequence to the Codex unit tests' executable writes and copies. Each lock wait has a five-second deadline. A deterministic Linux regression holds a duplicate writable descriptor, observes `ETXTBSY`, checks that the wait times out, then closes the descriptor and executes the fixture successfully. A duplicate models the shared open file description; this test does not create a fork race.

Tests remain parallel. There are no process-spawn retries. Temporary diagnostic logging was removed. Production launch, deadlines, ownership, cleanup, and executable validation are unchanged. No dependency or scanner pin changed.

## Verification

Doctor passed. All 46 Codex unit tests passed. Eight fixed post-fix full-crate stress samples also passed. These samples reduce uncertainty but do not prove that every concurrency schedule passes.

The selected full Rust plan passed: formatting, workspace tests, Clippy, and whitespace. The rebuilt native provider-lifecycle journey passed on its first run. It checks environment isolation, cancellation, restore quiescence, and normal-exit cleanup. Renderer tests, other native journeys, hosted CI, real accounts, Windows, and macOS did not run for this slice. No automatic verification retry or interruption occurred.

Private evidence: `.recovery/audit/r1-startup-diagnostic/summary.json`, its `11.log`, `.recovery/audit/r1-spawn-instrumentation.patch`, `.recovery/audit/r1-diagnostic-1794250806867579613.json`, and `.recovery/audit/r1-fixed-fixtures-stress/summary.json`. These records contain the failed sample as well as the passed samples.

Final evidence: `.recovery/audit/r1-verification-complete.json`, `.recovery/native-runs/r1-20261110/summary.json`, and metadata-only artifact receipts under `.recovery/artifacts/`.
