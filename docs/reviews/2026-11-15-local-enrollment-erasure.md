# Local enrollment erasure review

Recorded November 15, 2026. Scope: L-5a5, SC-029, OU-DATA-016/017/028, and the affected OU-BOARD-010 boundary.
The normative SRS remains unchanged.

## Desktop deletion boundary

The desktop prepares its leaderboard eraser after workspace and path checks.
It holds the shared user guard through the complete deletion operation.
It writes and syncs the existing version 1 intent before erasing a credential.
The intent format contains no leaderboard token or payload.
Erasure failure keeps the intent and blocks normal workspace access.
Explicit resume repeats erasure before deleting managed files and the database key.

A competing enrollment refuses before the intent and preserves saved data.
A store connection failure before the intent also preserves saved data.
Linux local deletion now needs an available, unlocked credential store, including password-protected workspaces.
Unknown storage cannot prove that all local credentials are gone.
A missing private runtime directory gives a separate coordination error.
Restore the login session or store access, then confirm deletion again.

The confirmation names local leaderboard credentials among the data it removes.
It states that public leaderboard data remains.
Complete local deletion does not issue public leave or revoke other devices.
Public departure remains a separate explicit action.
Other platforms do not create leaderboard credentials through current product callers.
Their existing deletion path remains, without new platform qualification.


## Native erasure acceptance

The Linux journey uses the compiled native-test app, private homes, D-Bus, GNOME Secret Service, and SQLCipher.
A typed Rust fixture creates active or pending records through EnrollmentSession.
The password workspace holds an active credential.
The OS-store workspace holds a pending credential.
Both records disappear after confirmed local deletion.

The journey checks competing processes, unavailable-store refusal, restart, renewed consent, and fresh setup.
It preserves unrelated credentials, external files, backup archives, and display calibration.
A seeded durable intent proves restart recovery without automatic deletion.
It does not establish crash or power-loss behavior.
The changed confirmation passed visual inspection in the actual native app.
Sixteen targeted renderer checks cover both browser engines, scope text, consent, errors, and accessibility.
The native vault journey also checks malformed and duplicate erasure of the selected slot, locked absence, and store absence.

Both independent source reviewers found no remaining concrete spec or standards violation.
They ran no tests or builds.
The local eraser owns no HTTP client and calls no public departure API.
That is a static source finding. Runtime network tracing was not collected.
No physical secure-erasure claim is made.


## Research

The official [Secret Service API Draft](https://specifications.freedesktop.org/secret-service/latest/org.freedesktop.Secret.Service.html) separates locked and unlocked search results.
Its [item interface](https://specifications.freedesktop.org/secret-service/latest/org.freedesktop.Secret.Item.html) defines deletion separately from secret retrieval.
The implementation uses metadata and deletion without retrieving the leaderboard payload.
Both source pages were read completely on November 15, 2026.
The pages support the API contract, not qualification of every credential service implementation.
Private GNOME execution supplies the recorded Linux acceptance.


## Remaining gates

Settings enrollment, other-platform enrollment, and hosted acceptance remain open.
Recovery after all local credentials are lost remains a separate service gate.
The current service can refuse a new device when the account reaches its device cap.
A fresh authorization path for public departure must work without requiring another enrollment slot.
This correction does not deploy or publish a service.

## Verification

The final milestone passes the full renderer suite, Rust workspace tests, workspace Clippy, formatting, and native syntax.
Native-feature Clippy also passes for the desktop, board, and keys packages.
Eleven focused deletion tests and sixteen focused renderer checks pass.
The native-feature keys suite passes thirteen unit and five process tests.
The separate vault and actual desktop journeys pass within the private Linux scope above.
The native app artifact has SHA-256 `1ad16ad0b123f00d7331f666fc3ee7e7296c1641a1ac6b991da2e2bfb14fe400`.
Normal verification preserves receipts and removes generated build output after the push.
