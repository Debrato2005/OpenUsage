# Linux key-store transport review

Date: October 18, 2026. Scope: the proposed native key-store integration, before acceptance.

The review rejected `dbus-secret-service` 4.1.0. Its session code uses `fastrand` for the private Diffie-Hellman exponent and the encryption IV. The library documents that its generator is not cryptographically secure. Switching between the Rust and OpenSSL crypto features does not replace that generator.

Sources: [upstream session code](https://docs.rs/dbus-secret-service/4.1.0/src/dbus_secret_service/session.rs.html) and [fastrand documentation](https://docs.rs/fastrand/latest/fastrand/).

The replacement is pinned `secret-service` 5.2.0 with `rt-tokio-crypto-rust`. Its [session code](https://raw.githubusercontent.com/open-source-cooperative/secret-service-rs/v5.2.0/src/session.rs) and [secret formatting code](https://raw.githubusercontent.com/open-source-cooperative/secret-service-rs/v5.2.0/src/util.rs) use OS randomness through `getrandom` for both values. The adapter requests encrypted sessions, checks locked items and collections, and bounds each application wait to five seconds. An OS permission prompt can still appear during explicit setup. The deadline does not guarantee prompt dismissal.

Two further fixes remove stale key-store availability state and distinguish a database recovery fault from an incorrect passphrase. Rust checks cover both paths. Key protection and read-back precede database creation. Missing keys and missing databases require recovery without replacement.

Owned Rust key and passphrase buffers use zeroization. This does not prove that every IPC or library allocation is wiped. Native restart and recovery acceptance requires separate execution evidence.
