# Desktop synthetic executable writes

Date: November 10, 2026. R-2 follows a failed workspace check during L-1f11f1. The Pi changes remain separate. All checks use private temporary homes and synthetic providers on Linux.

## Failure and evidence

The full workspace run passed 174 desktop tests and failed the scheduled-collection cooldown test. Its first collection returned `ProviderUnavailable`. The log did not capture the underlying error. A focused run of all 22 provider tests passed with temporary diagnostics. One separate diagnostic with 32 concurrent workers also passed. Neither reproduced the failure, so its cause remains unconfirmed.

Source inspection found a known exposure: desktop fixture constructors still wrote executables with `fs::write`, then launched them during parallel tests. The [R-1 investigation](2026-11-10-executable-fixture-race.md) reproduced the inherited-writer race in Codex tests and added a bounded writer there. A writable handle inherited by another process can outlive the writing thread's handle. The Rust issue describes the same exposure and a file-lock sequence that waits for those handles. [Rust issue 114554](https://github.com/rust-lang/rust/issues/114554).

This source comparison justifies removing the exposure from desktop tests. It does not establish the cause of the original cooldown failure. The search returned the primary Rust issue and reported a DuckDuckGo engine failure. Its cached source supplied the relevant lock sequence.

## Change and verification

Desktop tests now include the existing test-only writer module. Provider fixtures and the activity, quota, login, and collection fixtures use it for executable writes. Executable replacement and copy checks use it too. The shared helper has a five-second lock deadline and no process-spawn retry.

The deterministic inherited-writer test passed in the desktop crate. It observes `ETXTBSY` while a duplicate writer is open, checks the deadline, then executes after that writer closes. All 22 provider tests passed after the change. Doctor and formatting passed. Full workspace checks and rebuilt native provider-lifecycle also passed.

Temporary diagnostic logging and the 32-worker experiment were removed. Production code paths, process rules, credentials, scanner pins, and dependencies are unchanged. This repair adds no new unsafe implementation. It reuses the existing test module.

Renderer checks, other native journeys, hosted CI, real accounts, Windows, and macOS did not run. No automatic retry or interruption occurred. The first failed workspace record remains retained. The repair is separate from the Pi candidate. Pi verification resumes after this local commit.
