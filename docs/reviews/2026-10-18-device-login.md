# Native managed device-code sign-in

Updated: October 18, 2026. The actual Tauri application now supports explicit sign-in for a selected managed profile. Native qualification uses labeled synthetic challenges. Real OAuth and authenticated live identity remain unqualified.

## Consent and ownership

The user chooses Sign in, reviews the named profile, and chooses Get sign-in code. Showing consent executes nothing. The request binds the current executable approval before any process starts. Rust owns the provider login identifier, child process, managed home, cancellation, and deadlines. Checks and sign-in share two worker slots.

Prompt and completion waits run outside application locks. Publication requires the current profile, home, storage revision, approval, and operation generation. Old attempt tokens cannot cancel a newer login. Restore, profile changes, approval replacement, and genuine native exit stop owned operations before changing their bindings.

Only a matching successful completion followed by a fresh ChatGPT account response permits success. Early completion events remain bounded until the start reply arrives. Account updates, null identifiers, mismatched events, and cancel acknowledgments do not prove authentication. Results retain unknown or provisional identity confidence. They create no account claims or usage readings in SQLCipher.

The URL must exactly match the documented OpenAI device page. The human code must contain 1–64 visible ASCII characters and cannot be blank. The typed frontend contract rejects additional payload fields. Provider identifiers, tokens, credential files, database keys, and raw frames remain outside the renderer.

Cancel sends the matching provider request, then stops and reaps the owned process even if the provider does not respond. Cleanup errors retain priority. Cancellation does not promise credential deletion or logout. The UI explains that Codex can retain credentials already saved in the selected profile. OpenUsage waits at most 15 minutes after the challenge. Codex supplies no code expiry time.

## Interface and executed checks

The consent view uses one next action and Back. It adds no redundant checkbox. The prompt shows the plain address, selectable code, and visible Cancel action. Results receive focus. Terminal states clear challenge fields. Executable size and checksum sit under an advanced disclosure. Account rows use full-width status text with bounded line lengths.

The native journey passed consent, keyboard navigation, successful and early completion, invalid challenges, incorrect events, cancellation, stale attempts, restore quiescence, remapped homes, and genuine window close. Other and archived synthetic homes remained unchanged. Sign-in preserved database, WAL, and protected-key hashes. All owned synthetic providers were reaped.

Seven actual WebKit captures cover desktop, narrow light/dark views, keyboard focus, and 200% test-side text scaling. Root visual review found readable fields, clear profile binding, consistent typography, and no page or field clipping. This does not qualify OS zoom, screen readers, or full WCAG conformance.

A single-line URL field initially hid part of the address. A wrapped read-only field now keeps the full value visible at enlarged text sizes. Native WebKit reset selection after keyboard focus. Selection now repeats on the next frame only while that field retains focus. A further focus-ring failure came from the inactive Xvfb window. The harness now focuses only the sole visible OpenUsage window owned by the identified application PID. The unchanged product binary then passed full selection and visible-outline assertions.

The official regression also exposed a bootstrap race in its runner. It now waits for enabled setup fields after bridge readiness.

## Evidence and limits

The integrated workspace passed 95 Rust tests, strict Clippy, and formatting. One ignored child entry runs twice through its parent process-crash test. Renderer checks passed 21 synthetic contracts and 16 Chromium/WebKit checks. Native-test and default production debug configurations compiled. The production configuration refuses both synthetic executables before execution. The signed official Codex read-only regression uses a fresh managed home and performs no login or inference.

See `../evidence/native-device-login/`, `../evidence/native-device-login-source-gate/`, `../evidence/native-device-login-official-check/`, and `../evidence/native-device-login-verification/` for receipts, hashes, and logs.

The user approved SC-003 on October 18, 2026. It permits transient challenge display under its narrow controls. The normative SRS remains unchanged. Real sign-in qualification remains unexecuted. Browser opening, authenticated provider credential persistence, the full native 15-minute timeout, Windows/macOS, release performance, monitoring, quotas, and verified identity remain open. Core tests qualify bounded timeout behavior with synthetic deadlines. No existing Codex credential home was inspected.
