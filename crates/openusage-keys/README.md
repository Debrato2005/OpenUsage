# Keys crate

Keeps the 32-byte database key in the OS credential store (SRS Section 12.2): Secret Service on Linux, Keychain on macOS and the Windows credential store. The desktop app and the CLI share this one copy.

The key lives under the service `app.openusage.desktop` and the account `primary-database-key-v1`, as `v1:` and 64 lowercase hexadecimal digits. The `native-test` feature uses the separate service `app.openusage.desktop.native-test`.

On Linux each vault owns a one-worker Tokio runtime for its Secret Service connection, and each call has a 5-second limit. Removal checks the exact service, account and value before it deletes the one entry. A locked or missing store gives `Unavailable`, and a malformed or duplicate entry gives `InvalidKey`.

`cargo test -p openusage-keys` checks removal and malformed keys against a private mock store. The `persistence`, `local-data-deletion` and `cli-status` native journeys use a private GNOME Secret Service.
