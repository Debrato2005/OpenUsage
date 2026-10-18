# Native executable approval and connection checks

Updated: October 18, 2026. The application now reviews a local executable before explicit approval and runs bounded account-status checks in selected managed profiles. Usage collection stays stopped.

## Source and approval

Production approval requires the exact reviewed Codex 0.163.0 Linux x86_64 artifact. Its archive digest matches official release metadata. Sigstore verifies the executable signature, Fulcio chain, certificate transparency, and Rekor signed timestamp. Verification checks the exact OpenAI release workflow and issuer. The upstream annotated tag is unsigned. The signature evidence and independent artifact audit remain separate from application execution.

Inspection reads bounded file metadata and a streamed checksum. It executes no process. Unknown source bytes remain inspectable, but approval fails before execution. The user must select an initially unchecked confirmation. Approval lasts for the application session. The runtime checks the approved digest and filesystem identity before each version and app-server launch.

The static synthetic provider has one reviewed digest, enabled only in test configurations. Its UI and diagnostics identify controlled test data. The default production build rejects that fixture, including an explicit approval request, without executing it.

## Native behavior

Rust owns two worker slots, duplicate-profile refusal, timeouts, cancellation, and child cleanup. Each check uses the selected private home with a cleared environment. Restore and profile changes stop owned checks before changing records or home mappings. Completed results pass current home, approval, and generation checks before entering IPC.

Diagnostics expose authentication state and provisional identity confidence. They omit email, credentials, process identifiers, home paths, and provider frames. SQLCipher records remain unchanged by these checks. Login, verified identity, quota collection, and combined totals remain incomplete.

The disclosure sits between accounts and backups. Check and Cancel controls refer to their profile labels. Errors and results receive focus. Shared typography, surfaces, spacing, wrapping, and focus tokens cover both themes. Native captures cover desktop, 360-pixel windows, and 200% test-side text scaling. Screen-reader and full WCAG acceptance remain open.

## Executed verification

The integrated workspace passed 88 Rust tests, strict Clippy, and formatting. One child entry remains ignored by default and runs twice through its parent process-crash test. Renderer verification passed 18 contract checks and 12 Chromium/WebKit checks. Both native debug configurations compiled.

`NATIVE-PROVIDER-LIFECYCLE-001` passed source review, approval, tamper and protocol rejection, capacity, real cancellation, restore quiescence, fresh-home binding, and native exit cleanup. Five captures passed layout checks and visual review. `NATIVE-PRODUCTION-SOURCE-GATE-001` passed fixture refusal on the default configuration.

`NATIVE-OFFICIAL-PROVIDER-001` passed on the default build with the signed upstream executable. Actual GUI approval and two fresh-profile checks returned authentication required, unknown identity, and stopped monitoring. Database, WAL, and protected-key hashes stayed unchanged. Three native captures passed visual review. Debug checks took about 17 seconds, including repeated full-file digests. These lab timings do not qualify release performance.

`NATIVE-CHECKPOINT-WM-REGRESSION-001` passed on that same default binary. A pending authenticated archive preview became stale after genuine native close and restart. Refusal preserved reopened records, key, and homes. Confirmed checkpoint recovery then restored the original three home mappings and retained imported homes. Both native closes recorded database and sidecar state. Independent SQLCipher readers verified encrypted pages and selected semantic fields.

## Test failures and resolution

An initial backup wait matched the provider approval receipt before export completed. The runner now waits for the authenticated backup receipt, one archive, and enabled controls.

WebKitGTK 2.52.6 force-terminates its launched application when the last automation window disappears. WebDriver window deletion therefore cannot qualify native exit cleanup. The runner now sends WM_DELETE_WINDOW to the sole visible OpenUsage window owned by the identified application PID. Pending children and process groups disappear before driver cleanup. The production cleanup hook required no change. Historical preview and restore receipts retain their original bytes. Their WebDriver closure observations support restart and persistence, while this new regression establishes genuine native close behavior. Neither run proves forensic RAM erasure. See the [pinned WebKit implementation](https://github.com/WebKit/WebKit/blob/webkitgtk-2.52.6/Source/WebDriver/glib/SessionHostGlib.cpp) and [Tauri lifecycle events](https://docs.rs/tauri/2.12.4/tauri/enum.RunEvent.html).

## Remaining limits

Qualification covers Linux debug builds and a single reviewed provider version and target. Windows, macOS, signed application packaging, release performance, authenticated live identity, and monitoring remain open. Path and descriptor checks precede a pathname-based spawn. They do not provide atomic descriptor execution or isolation from a hostile process with the same user identity. Positive ownership tests used the host UID namespace because the sandbox maps root-owned ancestors to an untrusted overflow UID. The fail-closed policy remains unchanged.
