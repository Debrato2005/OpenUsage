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
