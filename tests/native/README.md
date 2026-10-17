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
Startup must not access the OS credential store.
Future storage tests need a separate credential namespace first.
The test removes only its temporary fixture and closes its own session and processes.

`NATIVE-SMOKE-001` checks real bootstrap IPC, command rejection, setup guidance, missing metrics, and keyboard access to privacy details.
It does not establish provider login, encrypted storage, or AT-01 acceptance.

Each run prints its evidence directory.
The JSON record includes the binary hash, source commit, platform, fixture type, expected result, actual result, and pass/fail status.
A successful run also saves a native screenshot.
Set `OPENUSAGE_NATIVE_EVIDENCE` to select the evidence directory.

This test uses the [official external-driver setup](https://v2.tauri.app/develop/tests/webdriver/manual-setup/).
