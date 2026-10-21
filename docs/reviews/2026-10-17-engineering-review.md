# Engineering review

Review date: October 17, 2026. Source scope: the paused renderer shell. Three independent agents reviewed provider contracts, security, and native testing. The coordinator checked the fix and the sources cited below.

## Fixed boundary defect

`apps/desktop/src/contracts.ts` accepted array enums through `String()`. An array containing `ready` also bypassed the ready-state metadata requirement. The public-wrapper regression failed before the fix and passed after strict string guards. Current severity is low because the shell has no sensitive native operation. Native input validation remains mandatory.

The review found no raw HTML sink, arbitrary process call, or filesystem operation in the renderer. This observation does not assess future Rust authorization, encryption, provider isolation, or release security.

## Provider decisions and limits

The source review used Codex tag `rust-v0.163.0`. It is a research target, not a tested supported version.

The [quota response schema](https://github.com/openai/codex/blob/rust-v0.163.0/codex-rs/app-server-protocol/schema/typescript/v2/GetAccountRateLimitsResponse.ts) includes nullable `accountId`. Its description associates the ID with the snapshot but does not promise stability. Stable account identity remains unverified. Email alone cannot establish it. Keep verified aggregate totals unavailable until the approved contract and authorized live checks establish distinct identities.

The [window schema](https://github.com/openai/codex/blob/rust-v0.163.0/codex-rs/app-server-protocol/schema/typescript/v2/RateLimitWindow.ts) supplies usage percentage, nullable duration, and nullable reset time. It supplies no window-generation field. The quota response also warns against inferring ordinary-use recovery from percentages or reset timestamps. Preserve SRS section 6.2. See [the clarification record](../srs-clarifications.md).

The [official app-server documentation](https://learn.chatgpt.com/docs/app-server) documents `account/usage/read` and nullable summary and daily values. It does not establish a Codex/Work split or complete date coverage in the reviewed example. Preserve unknown scope and missing values. No live usage or authentication test ran.

The [versioned usage parameters](https://github.com/openai/codex/blob/rust-v0.163.0/codex-rs/app-server-protocol/schema/typescript/v2/GetAccountTokenUsageParams.ts) also accept `threadId`. Supplying it changes the result to estimated thread usage. Omit it for account activity. The [Rust summary and daily types](https://github.com/openai/codex/blob/rust-v0.163.0/codex-rs/app-server-protocol/src/protocol/v2/account.rs#L495) use signed 64-bit integers. The future Rust adapter must reject negative counts and send exact decimal strings across IPC. JavaScript numeric parsing cannot establish exactness for large token counts.

## First native test

The [official Tauri testing guide](https://v2.tauri.app/develop/tests/webdriver/) recommends WebdriverIO with `@wdio/tauri-service`. Its embedded driver supports all three desktop platforms. Enable test plugins only in test builds. Direct `tauri-driver` supports Windows and Linux. Renderer browser tests do not establish native acceptance.

Use a separate `NATIVE-SMOKE-001` ID. AT-01 requires five connected plan fixtures, so a startup check cannot satisfy it.

1. Start a test build with temporary application storage and a synthetic `CODEX_HOME`.
2. Check that the native bootstrap command succeeds and reports setup required, monitoring stopped, and provider unconfigured.
3. Check setup guidance, unavailable metrics, and keyboard access to privacy details.
4. Close the native session in cleanup, including after a failed assertion.
5. Record the binary hash, build commit, platform, fixture type, expected state, actual state, and result.

When an external driver launches the app, isolate the driver's environment. Client-only environment changes do not isolate the launched app. Test builds also need a separate credential-store namespace before storage tests can run.

The current executor has Node 22.23.2 available through its explicit installation path. Rust, native app source, `tauri-driver`, `WebKitWebDriver`, and `pkg-config` are absent. Native tests and storage/performance benchmarks remain blocked. No measured performance claim is possible.
