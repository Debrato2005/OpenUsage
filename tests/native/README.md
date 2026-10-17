# Native startup smoke

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

Set `OPENUSAGE_VISUAL_FONT=Geist`, then run `node tests/native/visual.mjs` with the same drivers and display.
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
