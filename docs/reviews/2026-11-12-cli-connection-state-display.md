# CLI connection-state display

Date: November 12, 2026. Assignment: R-8. Evidence covers private synthetic
Linux data. The tests access no user logs, accounts, or credential stores.

## Result

Text and statusline now retain known connection failures with saved quota.
They show Authentication required, Unauthorized, Offline, Unsupported, or Error.
A valid last accepted observation time appears with the connection label.
Missing quota remains unavailable and has no invented observation time.

JSON stays unchanged. Its profile connection state and quota clock freshness
remain independent. Stale and Freshness unknown marks still appear. Connected
output keeps its existing format. No metric state, schema, identity rule, or
SRS text changes.

The CLI uses one helper for both output forms. The helper requires a parsed
observation time without control characters before it prints that field.
It starts no process, changes no source record, and requests no provider reading.

## Executed evidence

Two public-command RED cycles cover Offline and the other four failure states.
The first fixture attempt assumed quota acceptance marked a profile Connected.
The native coordinator owns that step, so the fixture now records Connected
explicitly. A private diagnostic pattern also needed corrected escaping.
Its incomplete receipt stays marked unverified. The corrected RED records the
named output assertion before the production correction.

Seven status tests passed. New cases compare JSON quota fields before and after
connection changes. They preserve values, source, observation time, nulls, and
clock freshness. Controls retain future unknown and stale marks and keep missing
quota unavailable. Repeated reads preserve SQLCipher bytes and the file set.

Two source mutations omit the connection label and observation time separately.
Each named regression fails. Exact source restoration passes. The final source
hash is in the private mutation receipt.

Both native-test binaries built with the pinned toolchain in 157.947 seconds.
The desktop digest is
`d9f14403210729d690da5b5a751fe6d34d74a25614ffde0b9f02ef59b6110f7b`.
The CLI digest is
`f938287f3b0abc31e2790d93882ab7e56caef1faa74be7d96224ad1c97c980b5`.

The enhanced native CLI journey passed in 9.485 seconds. It uses private GNOME
Secret Service, SQLCipher, synthetic logs, and the real pinned scanner. It checks
saved totals with the app open and after close, report consent, copied roots,
empty selected dates, unchanged source hashes, and locked exit code 7.
It also creates one private profile through the native app without a sign-in.
After a genuine close, the real CLI shows Authentication required and unavailable
quota. JSON keeps null source and observation time with no windows. CLI reads
preserve the saved workspace files.

Doctor and the affected project checks pass: CLI tests, Clippy, format,
whitespace, and native syntax. The selected project run took 35.990 seconds.
The full workspace and renderer milestones remain scoped to the earlier native
Codex change. This correction changes no renderer or storage implementation.

## Review and limits

The review checks correctness, privacy, maintenance, tests, and requirements.
The shared helper keeps both output forms consistent. The connection label
never replaces a saved value or its clock evidence. The tests use public command
dispatch and public storage setup operations.

Saved-quota connection states use public-command SQLCipher tests. The native
case covers Authentication required without a sign-in. No native network outage,
real provider account, other platform, or general clock recovery is qualified.
The broader repository review and production activation remain open.

Private receipts use `.recovery/audit/r6-r7-r8-*` and `.recovery/audit/r8-*`.
Native evidence is under `.recovery/native-runs/r8-20261112/`. These paths stay
outside the public source snapshot.
