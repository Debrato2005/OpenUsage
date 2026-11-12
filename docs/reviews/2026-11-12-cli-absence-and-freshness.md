# CLI absence and quota freshness

Date: November 12, 2026. Assignment: R-6 and R-7. Evidence uses private
synthetic Linux data. The tests access no user logs, accounts, or credential stores.

## Result

A report with detected sources and no rows in its selected dates now says
No local usage in this date range. The no-source case keeps its existing message.
Both cases retain an empty JSON `tools` array. Consent, date filtering, token
arithmetic, and the version 2 JSON shape stay intact.

Future quota observations now have `unknown` freshness. Text, default status,
and statusline show Freshness unknown. Current covers precise ages from zero
to less than 600 seconds. Stale starts at 600 seconds. The CLI keeps saved
quota values, source, observation time, and unavailable fields.

This uses the existing desktop treatment of future observations and the exact
cutoff. SC-012 still leaves the normative name for unknown freshness to the owner.
No SRS text changes.

## Executed checks

R-6 failed through the public report command before the correction, then passed.
Its existing no-source, consent, exact-total, and missing-estimate checks still pass.

R-7 has two recorded RED tests. A future observation produced Current instead
of unknown. The old cutoff also produced Current at the ten-minute boundary.
Four status tests now pass, including seven clock cases around observation and
the cutoff. Public JSON, text, default status, and statusline preserve the literal
expected quota fields. Repeated reads leave SQLCipher bytes and the file set unchanged.

The parent review inverted source detection and widened the cutoff in separate
source mutations. Each named regression failed. Exact source restoration passed.

Both native-test binaries built with the pinned toolchain in 156.036 seconds.
The desktop digest is
`cb85f88f076f560afb4da8249d4afd16df7be7a509775e3b2439ff26e42c9a9c`.
The CLI digest is
`a6196aeab22ac3615844cd8cf28f5395e8ea4ca4b60fc2c5ac6c3e75294625f5`.

The enhanced native CLI journey passed in 8.532 seconds. It uses private GNOME
Secret Service, SQLCipher, and the real pinned production scanner. Status reads
the saved total with the app open and after a genuine close. Locked key-store
checks retain exit code 7 and no prompt. Report keeps copied roots at their
original total, reports an empty selected day, and preserves older totals.
After controlled date preparation, CLI reads preserve source hashes and saved
workspace files. Future quota cases use public-command SQLCipher tests.
The native journey does not simulate a future system clock.

Doctor and the selected project checks pass: the affected CLI crate, Clippy,
format, whitespace, and native harness syntax. The full workspace and renderer
milestones belong to the preceding native Codex change. This assignment changes
no renderer or storage implementation, so it does not repeat those milestones.

## Review and scope

The review checks correctness, privacy, maintenance, tests, and requirements.
The report uses detected-source presence only for its empty text. Freshness uses
precise signed duration after the future-time guard. No read starts collection,
changes a key, or writes saved records. Missing measurements keep their states.

The production scanner pin, catalog, and schema stay unchanged. Other platforms,
real-provider acceptance, release qualification, and the broader repository review
remain open. The general clock-recovery policy remains outside this correction.

Private receipts use `.recovery/audit/r6-r7-*`. Native evidence is under
`.recovery/native-runs/r6-r7-20261112/`. These paths stay outside public source.
