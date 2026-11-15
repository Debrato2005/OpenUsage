# Keys crate

Keeps the 32-byte database key in the OS credential store (SRS Section 12.2): Secret Service on Linux, Keychain on macOS and the Windows credential store. The desktop app and the CLI share this one copy.

The key lives under the service `app.openusage.desktop` and the account `primary-database-key-v1`, as `v1:` and 64 lowercase hexadecimal digits. The `native-test` feature uses the separate service `app.openusage.desktop.native-test`.

On Linux each vault owns a one-worker Tokio runtime for its Secret Service connection, and each call has a 5-second limit. Removal checks the exact service, account and value before it deletes the one entry. A locked or missing store gives `Unavailable`, and a malformed or duplicate entry gives `InvalidKey`.

`cargo test -p openusage-keys` checks removal and malformed keys against a private mock store. The `persistence`, `local-data-deletion` and `cli-status` native journeys use a private GNOME Secret Service.

## Leaderboard credential storage

`BoardVault` in `openusage-keys` owns one fixed OS credential slot for public leaderboard enrollment. It uses the existing application service name and the account `leaderboard-device-session-v1`. The database key keeps `primary-database-key-v1`. Native tests use the existing separate service namespace.

`BoardCredential` holds between one and 4,096 payload bytes in zeroized memory. Its Debug output is redacted. Deliberate Rust access uses `expose()`. The store adds a fixed version prefix. Its caller must use the [enrollment codec](../openusage-board/README.md#enrollment-credential-format) to bind the configured service origin, device ID, login, and service token. This low-level boundary does not define that schema. The [session coordinator](../openusage-board/README.md#serialized-enrollment) owns typed persistence.

`protect` refuses an occupied slot. `replace` updates only the exact expected selected item. Removal compares the exact expected bytes before deleting the selected item. Missing items count as already removed. On Linux, duplicate, locked, malformed, and unavailable entries refuse without an unlock prompt. Each service operation retains its five-second bound. The store has no plaintext fallback or network operation.

The OS store does not supply a transaction across processes. Callers use the guard below. Duplicates refuse safely. Settings integration remains open. Database removal does not revoke public leaderboard membership or delete this separate credential.

### Acceptance

The private Linux harness starts its own D-Bus and GNOME Secret Service. Synthetic credentials survive a daemon restart. Tests cover occupied slots, expected-value mismatches, malformed and duplicate items, locked collections, unavailable service, and isolation from the database key. No existing user keyring or live provider is read.

Build the ignored test with `native-test` through the shared Cargo environment. Select its executable from Cargo's JSON artifact output. Set `OPENUSAGE_VAULT_TEST_BINARY` to that absolute path and `OPENUSAGE_NATIVE_EVIDENCE` to a new private directory. Run `timeout 120 node tests/native/board-vault.mjs`. The harness supplies the isolated environment and invokes each phase. Direct execution against a normal user environment refuses.

The test proves the Linux storage boundary only. The separate CLI journey checks typed persistence and online commands. Settings, hosted enrollment, and other platforms retain separate acceptance gates.

## Credential coordination

BoardGuard::for_current_user uses /run/user/<uid>/app.openusage.desktop-leaderboard/enrollment.lock on Linux.
The path depends only on the Linux user ID.
It never depends on HOME, XDG_DATA_HOME, or the service origin.
The OS runtime directory must already exist, belong to that user, and use mode 0700.
The guard creates its application directory and persistent empty lock file.
Missing or unsafe runtime directories refuse without a fallback.
Directories use mode 0700. The empty regular file uses mode 0600.
Links, multiple file links, shared writes, and changed inodes refuse.
Never remove the file to clear a busy guard.

Callers hold the guard across the complete operation, including authorization and network calls.
This guard coordinates participating OpenUsage processes.
Other programs under the same user can still change the credential store.
BoardVault::replace compares exact expected bytes before it updates the selected OS item.
It preserves that item and never creates a delete gap.
Missing, mismatched, malformed, duplicate, locked, and unavailable items refuse.
Each Linux store call keeps its five-second bound.
