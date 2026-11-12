# Native startup smoke

See [implementation status](../../docs/implementation-status.md#failing-or-stale-tests) for which journeys currently pass.

Run `node tests/native/smoke.mjs` after the native binary build.

This Linux test uses the real Tauri binary, `tauri-driver`, and `WebKitWebDriver`.
It needs Node 22 and a working display, such as a separate Xvfb session.
It adds no application test plugin or npm package.
Windows and macOS remain untested.

The default binary is `target/debug/openusage-desktop`.
Set `OPENUSAGE_NATIVE_BINARY` to test another binary.
If that binary comes from another checkout, also set `OPENUSAGE_NATIVE_SOURCE_ROOT`.
Set `TAURI_DRIVER` or `WEBKIT_WEBDRIVER` when a driver is outside `PATH`.

The driver receives fresh XDG directories and a synthetic `CODEX_HOME`.
The test leaves `HOME` unchanged.
All native tests use a private D-Bus configuration without service activation directories.
The smoke starts no Secret Service.
The persistence test starts only its isolated GNOME Secret Service.
The test removes only its temporary fixture and closes its own session and processes.

`NATIVE-SMOKE-001` checks real bootstrap IPC, command rejection, setup guidance, missing metrics, and keyboard access to privacy details.
It does not establish provider login, encrypted storage, or AT-01 acceptance.

Each run prints its evidence directory.
The JSON record includes the binary hash, source commit, platform, fixture type, expected result, actual result, and pass/fail status.
A successful run also saves a native screenshot.
Set `OPENUSAGE_NATIVE_EVIDENCE` to select the evidence directory.

This test uses the [official external-driver setup](https://v2.tauri.app/develop/tests/webdriver/manual-setup/).

## Native Pi candidate

This Linux experiment uses the disabled Pi candidate through the normal consent,
worker, grouped storage, and typed view paths. Build the renderer first. Use the
shared Cargo target and the pinned toolchain.

```sh
bun run build
cargo build --locked -p openusage-desktop --features pi-native-qualification
cargo build --locked --manifest-path tests/native/backup-verifier/Cargo.toml
```

Set `OPENUSAGE_NATIVE_BINARY` to the absolute desktop path. Set
`OPENUSAGE_NATIVE_PI_SCANNER` to the bounded release candidate with the digest in
`tests/tools/fixtures/pi-producer/pin.json`. Set `OPENUSAGE_LOGIN_VERIFIER` to the
built `openusage-native-activity-verifier`. Set
`OPENUSAGE_LOGIN_VERIFIER_SHA256` to that verifier's checked digest. Set
`OPENUSAGE_NATIVE_EVIDENCE` to a private output directory. Use the driver variables
above when needed.

Run `xvfb-run -a node tests/native/pi-qualification.mjs` with a 240-second deadline.
The journey creates private logs and a password workspace. It checks copies,
exact growth, unavailable details, the pinned zone, and scanner provenance.
Malformed input and a wrong binary digest must preserve saved data. Withdrawal
must prevent a later source change from entering storage. The independent
SQLCipher oracle checks saved records after native shutdown. Source hashes must
stay unchanged by scans.

Rebuild with only `--features native-test`. Run the same journey with
`--expect-disabled` and a separate evidence directory. This checks that the
ordinary build neither offers nor grants Pi. The catalog and production scanner
pin stay unchanged. These results cover synthetic Linux input only. They do not
qualify real histories, other platforms, or a production release.

Set `OPENUSAGE_LOGIN_VERIFIER_SHA256` to the checked digest for a newly built
activity verifier. Existing journeys keep their historical pin when it is unset.
An invalid pin or a mismatch refuses execution.

## Native Codex candidate

This Linux build tests the combined Codex snapshot through consent, worker scans,
grouped storage, and saved views. Build the renderer first. Use the shared Cargo
target and the pinned toolchain.

```sh
bun run build
cargo build --locked -p openusage-desktop --features codex-native-qualification
cargo build --locked --manifest-path tests/native/backup-verifier/Cargo.toml
```

Set `OPENUSAGE_NATIVE_BINARY` to the absolute desktop path. Set
`OPENUSAGE_NATIVE_CODEX_SCANNER` to the combined candidate whose digest appears
in `tests/tools/fixtures/codex-snapshot-candidate/pin.json`. Set
`OPENUSAGE_CODEX_CAPTURE_RECEIPT` to the private synthetic producer receipt named
by `tests/tools/fixtures/codex-producer/capture.json`. The journey checks its
hash before it copies the three captured source files. It reads no real logs.

Set `OPENUSAGE_LOGIN_VERIFIER` to the built activity verifier. Set
`OPENUSAGE_LOGIN_VERIFIER_SHA256` to its checked digest. Set
`OPENUSAGE_NATIVE_EVIDENCE` to a private output directory. Use the driver
variables above when needed.

Run `xvfb-run -a node tests/native/codex-qualification.mjs` with a 360-second
deadline. The journey uses a private home, bus, and password workspace. It checks
measured zero, copied files, the full fork, missing attribution, the pinned zone,
and scanner provenance. It also checks refused input, withdrawal, wrong digests,
and genuine reopen. Independent SQLCipher reads check exact saved records.
Authenticated backup, full JSON export, restore checkpoints, and applied restore
must retain every local field. The journey retains metadata and screenshots.
The capture dates are November 11, 2026. Overview checks require them in the
last 30 days.

Rebuild with only `--features native-test`. Run the same journey with `--ordinary`
and a separate evidence directory. Codex must remain available with the production
scanner. The ordinary build must ignore the candidate override. These experiments
do not change the production pin or qualify real histories, other platforms,
or a release.


## Native summary details

After the renderer build, build the desktop with `--features native-test` in the shared Cargo target. Run `node tests/native/summary-details.mjs` on a private Xvfb display with `OPENUSAGE_NATIVE_BINARY` and `OPENUSAGE_NATIVE_EVIDENCE` set to absolute paths. Use the driver variables above when needed. Restore the production build afterward and run `provider-source-gate.mjs` against it.

`NATIVE-SUMMARY-DETAILS-001` creates a private home, D-Bus session and SQLCipher workspace. An approved synthetic provider supplies one account's saved readings. The journey checks summary disclosure with real keyboard input, focus, source details, missing-value behavior, both themes and narrow text layout. It verifies unchanged saved quota, protected key and synthetic provider-request evidence. It saves screenshots and source hashes, then removes its private fixture. It does not qualify real accounts, OS text scaling, screen readers, other platforms or the complete AT-85 review.

## Native persistence

Build the application with the `native-test` Cargo feature before `node tests/native/persistence.mjs`.
This feature uses the separate `app.openusage.desktop.native-test` credential namespace.
The test checks that the production namespace stays empty on its private bus.

The persistence test needs `dbus-daemon`, `gdbus`, Python 3, `gnome-keyring-daemon`, and `secret-tool`.
Set `OPENUSAGE_SECRET_SERVICE_SDK` to a verified package directory when GNOME tools are outside the system installation.
The SDK layout must contain `runtime/usr/bin` and `runtime/usr/lib/x86_64-linux-gnu`.

The test creates a private D-Bus session and synthetic XDG directories.
It starts only the GNOME secrets component and unlocks its temporary keyring with a synthetic passphrase.
It leaves `HOME` unchanged and never queries the user's Secret Service.

`NATIVE-STORAGE-001` checks workspace creation, saved appearance, application and keyring restarts, passphrase fallback, wrong-passphrase preservation, and missing-key recovery.
It also checks the native database, active WAL, wrapped-key file, private file modes, and rejection by unkeyed SQLite.
It does not establish power-loss recovery, live provider capability, or release qualification.

The private bus uses the [documented D-Bus configuration](https://dbus.freedesktop.org/doc/dbus-daemon.1.html).

After the renderer build, build the test binary with `cargo build -p openusage-desktop --features native-test`.
Use Node 22 for this command from the repository root:

```sh
xvfb-run -a env \
  GDK_BACKEND=x11 \
  GIO_USE_PROXY_RESOLVER=dummy \
  TAURI_DRIVER=/tmp/openusage-native-tools-01a14ad0/bin/tauri-driver \
  WEBKIT_WEBDRIVER=/usr/bin/WebKitWebDriver \
  OPENUSAGE_SECRET_SERVICE_SDK=/tmp/openusage-secret-service-sdk-01a14ad0 \
  node tests/native/persistence.mjs
```

These task-local SDK paths must exist and match their package integrity records.
The GLib resolver setting applies only to this local test process after a proxy lookup failure.
For a preserved binary from another checkout, also set `OPENUSAGE_NATIVE_BINARY` and `OPENUSAGE_NATIVE_SOURCE_ROOT`.

## Native backup export

Build the standalone verifier from the repository root:

```sh
cargo build --locked --manifest-path tests/native/backup-verifier/Cargo.toml
```

Then run the native backup test with Node 22:

```sh
xvfb-run -a env \
  GDK_BACKEND=x11 \
  GIO_USE_PROXY_RESOLVER=dummy \
  TAURI_DRIVER=/tmp/openusage-native-tools-01a14ad0/bin/tauri-driver \
  WEBKIT_WEBDRIVER=/usr/bin/WebKitWebDriver \
  OPENUSAGE_BACKUP_VERIFIER="$PWD/tests/native/backup-verifier/target/debug/openusage-native-backup-verifier" \
  node tests/native/backup.mjs
```

The fixture sets its Downloads directory inside its temporary XDG configuration.
The test does not use the user's Downloads directory or credential store.
The password goes to the verifier through standard input.
The verifier uses the production archive codec and reads the SQLite snapshot in memory.
It writes no plaintext records to disk.

`NATIVE-BACKUP-001` checks explicit records consent, password confirmation, native export, private file modes, and the saved receipt.
It checks schema and appearance in the authenticated records, wrong-password rejection, and tamper rejection.
It also checks locked-workspace rejection, malformed arguments, and the absence of pending files after success.
The evidence directory contains the encrypted archive and a native screenshot.
This test does not establish restore acceptance, power-loss recovery, or live provider capability.

## Native visual checks

Set `OPENUSAGE_VISUAL_FONT="DM Sans"`, then run `node tests/native/visual.mjs` with the same drivers and display.
The test also needs the private Secret Service tools from the persistence test.
Use a `native-test` binary and set `OPENUSAGE_NATIVE_SOURCE_ROOT` when it comes from another checkout.

`NATIVE-VISUAL-001` saves light and dark native images for setup, passphrase fallback, ready, recovery, and backup states.
It checks the loaded font, keyboard focus outline, horizontal overflow, and unavailable provider metrics.
The form checks require the correct error focus, field association, and semantic error color.
The native window must supply the recorded 1280×850 and 360×800 viewports.
The test enlarges computed font sizes to 150% inside the renderer, then removes those test styles.
This checks text layout. It does not establish operating-system zoom or display scaling.

The Unicode byte-limit case enters 600 `é` characters through native WebDriver input.
Their 1200 UTF-8 bytes exceed the Rust limit while their 600 UTF-16 units fit the HTML limit.
This environment left emoji fields empty during a separate native input diagnosis.
The visual test does not establish emoji input acceptance.

Timing records describe individual synthetic lab launches.
They include driver creation and polling where stated.
They do not establish a percentile, release performance, or complete accessibility acceptance.

## Native profile lifecycle

Build the `native-test` binary first. The profile test needs Python 3 with PyGObject and GTK 3.
Run it on a private Xvfb display:

```sh
xvfb-run -a env \
  GDK_BACKEND=x11 \
  GIO_USE_PROXY_RESOLVER=dummy \
  TAURI_DRIVER=/tmp/openusage-native-tools-01a14ad0/bin/tauri-driver \
  WEBKIT_WEBDRIVER=/usr/bin/WebKitWebDriver \
  OPENUSAGE_NATIVE_PRIVATE_XVFB=1 \
  node tests/native/profiles.mjs
```

The 100-emoji label uses the [GTK clipboard](https://docs.gtk.org/gtk3/method.Clipboard.set_text.html) and native Control+V on that private display.
The helper changes no renderer values through JavaScript.
It closes its clipboard owner after the real input receives the label.

`NATIVE-PROFILES-001` creates ten profiles through the GUI and checks the refusal of an eleventh.
It checks rename, archive, restore, restart, and slot reuse through the GUI.
Direct native IPC checks metadata, locked access, malformed input, and capacity enforcement.
The test checks SQLCipher persistence and private managed homes without provider login.
An explicit synthetic marker checks local data retention across archive and restore.
Separate file and symlink faults affect only synthetic paths and must preserve their markers.
This test does not establish provider collection, remote history, or other-platform acceptance.

## Native backup inspection

Run `node tests/native/backup-inspection.mjs` with the same private display, driver, binary, and source settings.
This test uses passphrase fallback and starts no Secret Service.
Build the standalone package above first, then set `OPENUSAGE_BACKUP_FIXTURE_GENERATOR` to its `target/debug/openusage-native-backup-fixtures` binary.

`NATIVE-BACKUP-CONTENT-001` extends the archive checks in `NATIVE-BACKUP-INSPECTION-001`.
It creates three profiles through the GUI, archives one, exports a backup, and checks two active profiles, one archived profile, and no account records.
It checks the empty catalog, wrong password, authenticated metadata, and recovery after a malformed archive.
The GUI checks cleared passwords, error association, focus, and both themes at 360 pixels.
It checks focus after pointer and keyboard appearance saves.
Direct IPC checks malformed requests, tampering, truncation, trailing bytes, symlinks, hardlinks, and unsafe file modes.
The test checks that inspection preserves the source archive and encrypted workspace, then removes all temporary fixtures.
The evidence directory keeps the encrypted archive and native images.

The fixture generator authenticates the exported snapshot and changes records only in memory.
It writes encrypted fixtures for non-SQLite content, a future manifest, a future database schema, and a broken account relation.
Invalid or unsupported contents must retain authenticated archive metadata, show a useful warning, and omit record counts.
The receipt must state that restore is unavailable for every result.
This test does not establish restore or other-platform acceptance.

## Native restore preview

Run `node tests/native/restore-preview.mjs` with the same private display, drivers, and source settings.
Build the standalone package first. Set `OPENUSAGE_CHECKPOINT_VERIFIER` to its `target/debug/openusage-native-checkpoint-verifier` binary.
Keep `OPENUSAGE_BACKUP_FIXTURE_GENERATOR` set for invalid-content cases.

`NATIVE-RESTORE-PREVIEW-001` exports GUI state A, changes profiles and appearance to state B, then prepares a restore preview.
The preview must follow publication of a private encrypted checkpoint containing exact state B.
The Rust verifier unwraps only the synthetic fixture key, opens the checkpoint read-only, and checks its schema and records.
It runs no migration, changes no journal mode, returns no key, and writes no plaintext database.
The test checks unchanged checkpoint bytes, live records, wrapped key, and synthetic home markers.

The journey checks cancel, invalidation after a live write, failed replacement, and restart after a normal native window close.
It waits for the owned desktop process to exit before driver cleanup.
The layout checks include both narrow themes and test-side text scaling to 200%.
This is a CSS text-layout check, not operating-system zoom or display scaling.
The test does not establish apply, undo, interrupted-commit recovery, or memory cleanup after forced termination.

## Confirmed native restore

Run `node tests/native/restore-apply.mjs` with the private display and drivers above.
Set `OPENUSAGE_NATIVE_SOURCE_ROOT` to the frozen build source.
Set `OPENUSAGE_LIVE_VERIFIER` and `OPENUSAGE_CHECKPOINT_VERIFIER` to their standalone Rust binaries.
Keep evidence outside temporary executor storage with `OPENUSAGE_NATIVE_EVIDENCE`.

`NATIVE-RESTORE-APPLY-001` exports one active and one archived GUI profile with Light appearance.
It changes that workspace to three active profiles with Dark appearance.
The test checks stale previews, strict confirmation arguments, checkpoint faults, and unsafe home permissions.
Each refusal must preserve the live records, key, and existing homes.
The GUI requires an unchecked confirmation before Restore records becomes available.
After confirmation, the test checks selected restored fields, appearance, and a normal restart.
Those fields are profile IDs, labels, archival flags, environment, connection state, and account count.
It does not compare every column or timestamp.
A changed source archive must not replace the retained authenticated contents.

The live verifier opens SQLCipher read-only with WAL support and no migrations.
It unwraps only the private synthetic fixture key through production code.
It checks fresh, private, empty homes for active and archived imported profiles.
It checks unchanged database, WAL, and wrapped-key bytes and reports shared-memory changes separately.
After normal exit, it checks a byte-identical private encrypted copy with the existing WAL, when present.
SQLite can create empty WAL and shared-memory sidecars on that derived copy. The receipt records this.
The original closed database, WAL, and key remain unchanged. The verifier never uses immutable mode.
Existing synthetic home markers must stay intact.
Native screenshots and debug GUI timing do not establish release performance or cross-platform acceptance.
The historical recovery audit records preserved receipt hashes and missing temporary executables.

## Native checkpoint recovery

`restore-checkpoint.mjs` is the next native journey after confirmed backup restore.
Use the same drivers, source root, and verifier settings.
The test creates GUI states A and B, restores A, then restarts OpenUsage.
It lists durable checkpoint metadata and recovers B with explicit confirmation.
The preview must save a new encrypted checkpoint of A before replacement.

The test checks stale previews, corrupted checkpoints, and a replaced original home.
Each refusal must preserve current records, key, and home directories.
After recovery, selected semantic fields and the three original home mappings must match B.
Existing synthetic markers and the two imported homes must stay intact.
The read-only oracle requires explicit existing-home mode and exact expected mappings for this check.
Its default mode continues to require fresh, private, empty homes for a backup import.
This journey does not establish forced-interruption recovery or cross-platform acceptance.

The recovery journey also captures the catalog, comparison, and confirmation at 360 pixels with doubled text.
It checks whole label words, comparison reflow, keyboard access to the unchecked confirmation, and horizontal overflow.
The runner restores all temporary inline text styles before the actual restore.
This is test-side CSS scaling, not operating-system zoom or full accessibility acceptance.

## Native provider lifecycle

`provider-lifecycle.mjs` uses an owned synthetic executable copied into a private fixture.
It does not use a real Codex executable or inspect existing credential homes.
The executable supports only version output, initialization, and a read-only account response.
Its central execution log detects any process launch before explicit executable approval.
The reviewed fixture has one fixed native-test digest and an explicit test-fixture classification.
Unknown digests must fail approval before any execution. The default production build must reject the test fixture.
Behavior modes come from adjacent private JSON. They do not change the executable bytes.
The log records controlled environment values and unexpected variable names, without recording other values.

The journey checks executable changes, malformed responses, two active slots, duplicate rejection, and real GUI cancellation.
A restore must stop the old owned check before it changes home mappings.
The next check must use the fresh restored home.
Normal window close must stop and reap the registered provider child and process group.
This fixture does not establish live authentication, official executable provenance, monitoring, or account identity.

## Native device-code sign-in

Run `node tests/native/device-login.mjs` with the private display and drivers above.
Set the frozen native-test executable, source root, and persistent evidence directory.
The journey uses only `fixtures/synthetic-login-provider.py` and fresh synthetic homes.
The existing read-only provider fixture stays unchanged.

Sign in starts only the selected profile. Approving Codex in Settings is the consent.
The test follows keyboard focus from Sign in to the selectable code and the Open sign-in page button.
It captures the actual prompt in both themes, at a narrow width, and with doubled text.
A completed sign-in shows the connected account, not a result panel. Errors and cancellations keep their result.
The journey never opens a browser or performs real OAuth. `browser-login` checks the sign-in page request.

Early completion must precede a fresh account check.
Mismatched completion, malformed challenges, and unsupported account states must fail.
Cancel must work before the challenge arrives and must not affect a newer attempt.
A restore and genuine X11 window close must stop and reap the owned login process.
Other homes, archived homes, and application records must stay intact.
Identity remains provisional or unknown, and monitoring never runs. A failed reading can pause the app-wide summary.

The native application wait limit remains 900 seconds.
Short timeout behavior is checked in the Rust core, not this native journey.
The production source-gate journey rejects both synthetic fixtures before execution.
These checks do not establish live login, token persistence, verified identity, or release qualification.

The X11 helper verifies the exact process ID and visible OpenUsage window before it acts.
Under Xvfb without a window manager, native automation can update `activeElement` while `document.hasFocus()` stays false.
That state does not apply the focused field's CSS outline.
The login journey raises and focuses only its owned native window before keyboard input.
It checks document focus and records the X11 receipt.
No DOM focus or style override substitutes for this setup.
Normal close separately sends `WM_DELETE_WINDOW`, then waits for the application and provider to exit.

## Native local log consent

Run `node tests/native/local-log-consent.mjs` with the private display and drivers above. Set `OPENUSAGE_LOGIN_VERIFIER` to the activity verifier.

`NATIVE-LOCAL-LOG-CONSENT-001` copies the synthetic Claude Code and Codex logs from `crates/openusage-locallogs/tests/fixtures/home` into a private home, and names it with `OPENUSAGE_NATIVE_LOG_HOME`. A native-test build reads only that variable for detection, never the real home. `OPENUSAGE_NATIVE_CCUSAGE` names the provisioned pinned `ccusage`, and both the journey and the app check its digest.
The journey moves the copied Claude Code log days to the two days before today in UTC, so that the 30-day Overview includes them. The welcome screen must list both tools, selected. The journey unselects Codex with the keyboard and creates a password workspace. The app must grant only Claude Code and scan only its folder. With no account, Overview must show the This device section with the exact Claude Code total and no Codex card (AT-91). The saved local usage must hold the two Claude Code sessions with their project, start and end times, and no reasoning count, and the three Claude Code hours (SC-021). The verifier then checks the saved sources and the exact daily totals.
A restart must ask no second consent, and a second scan must give the same totals. The synthetic logs must stay unchanged. This journey reads no real logs and does not establish other platforms.

## Native local-history performance

Build the desktop with `--release --features native-test` after the renderer
build. Set `OPENUSAGE_NATIVE_BINARY` and `OPENUSAGE_NATIVE_EVIDENCE` to absolute
paths. Use the private display and driver settings above. Limit the runner
to four CPU affinity slots, then run `node tests/native/local-log-performance.mjs`.

The benchmark creates three private 1 GiB Claude Code histories. It measures
from the setup action that grants consent to two animation frames after the
saved total appears. It checks all 64 saved sessions and unchanged source
hashes. Each sample must finish within 60 seconds. The machine and memory
scope remain explicit in the [performance report](../../docs/performance-report.md).

## Native CLI status

Build both native binaries in the shared Cargo target.

```sh
cargo build --locked -p openusage-desktop -p openusage-cli --features openusage-desktop/native-test,openusage-cli/native-test
```

Set `OPENUSAGE_NATIVE_CLI` to the CLI binary. Set `OPENUSAGE_NATIVE_BINARY` to
the desktop binary. Set `OPENUSAGE_NATIVE_EVIDENCE` to a private output directory.
Use the private display, driver settings, and `OPENUSAGE_SECRET_SERVICE_SDK`
above. Run `node tests/native/cli-status.mjs`. The `native-test` CLI reads the
separate credential namespace.

`NATIVE-CLI-STATUS-001` starts a private GNOME Secret Service. The native-test app creates a key-store workspace, gets consent for the synthetic Claude Code and Codex logs, and scans them with the real pinned `ccusage`. The journey moves the copied Claude Code days to today and yesterday in UTC. The CLI must give the exact saved total for today in `status --json` and `statusline`, first with the app open and then after a genuine close. After the close, the database, its WAL and the file set must stay unchanged. With the Secret Service gone, both commands must give `locked` with exit code 7 and no prompt. Version 2 JSON keeps legacy scope metadata and unavailable measurements explicit. The journey also checks `report` consent, `--yes`, and copied Claude roots with the pinned scanner. Report must leave the saved workspace unchanged.

The report check also moves private source days before today. One selected day
must show no usage, while the longer range retains the original totals. JSON
keeps an empty `tools` array for the empty day. The scan must preserve source
hashes and saved workspace files. Future quota and subsecond boundary cases use
public-command tests with private SQLCipher workspaces. The native journey
checks report text and actual CLI access to the isolated key store.

## Native share images

Run `node tests/native/share-images.mjs` with the private display and drivers above.

`NATIVE-SHARE-IMAGES-001` creates a password workspace with synthetic Claude Code and Codex logs in a private home, and scans them with the real pinned `ccusage`. It opens Share from This device on Overview. A preview must save nothing. Save PNG and Save SVG must each write one file in the private Downloads/OpenUsage folder. The PNG must be 2400 by 1260 pixels, and the SVG must hold the exact total and no path, project name or password text.

The native share journey also selects one saved folder and checks its exact total. Unknown and undated scopes must fail. The selected image must contain no folder path or project name.
