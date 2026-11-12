# Profile removal in the app

Date: November 12, 2026. Assignment: S-08a2. Scope: private synthetic Linux
application and renderer checks. No real account, existing credential home,
provider session, or user log enters these fixtures.

## Result

The profile menu now offers Remove. Its confirmation has separate Archive
History and Delete History choices. An archived profile offers Delete history.
Both choices require explicit approval. [Architecture](../architecture.md#profile-removal)
owns the command and storage boundary. [SC-026](../srs-clarifications.md#sc-026-profile-removal-and-retained-delivery-controls)
owns the retained-data reading. The persistent schema stays at version 13.

Rust caches one approval with the exact profile, home, choice, and revision.
The renderer receives four safe fields. Replaced, cancelled, consumed, stale,
or mismatched approvals cannot authorize another operation. A late preview cannot
replace the current dialog. A changed target requires another review.

Apply checks consent before cleanup. The workspace lock orders profile removal
with notification claim, bounded submission, and completion. Selected monitoring
and provider work stop before storage deletion. Selected accepted caches and
pending previews clear. Other profiles retain their work and saved values.
Cleanup failure preserves the records and consumes the approval.

## Executed checks

The public app archive regression first failed with ProfileRemovalFailed.
The implementation then passed that named test. Six Workspace tests cover
archive and deletion, homes, selected caches, one-use consent, wrong choices,
stale revisions, replacement, cancellation, failed cleanup, and unavailable homes.
Strict command arguments reject missing consent, unknown choices, private paths,
extra scopes, and wrong types. Three renderer contracts check safe previews,
exact selected choices, and matching removal results.

The dialog regression first failed because Remove was absent. The final full
renderer milestone passed in Chromium and WebKit, including build and typed
contracts. It took 414.559 seconds. The five new flow cases run in both engines.
They cover consent, cancellation, focus, stale review, late previews, and retained
quota behavior. The result file records no failed renderer tests.

The full staged workspace milestone passed in 412.749 seconds. Workspace tests
took 295.988 seconds, and Clippy took 115.896 seconds. Format and whitespace
passed. Production Rust and frontend source stayed unchanged during native checks.
The runner drains compiler output and keeps bounded metadata.

## Native removal evidence

The native-test app and nine oracle and fixture artifacts have recorded source and binary
digests. The application digest is
`fb68e3404fe53ad1131eace0e913baeee6b4cfa29ee2b62abf2554f5b37b6283`.
Builds used the shared target. Native runs check these digests before and after
execution. No retained executable copy is needed for normal evidence.

The expanded profile journey passed in 35.695 seconds. It exercises ten-profile
capacity, rename, archive, restore, active and archived deletion, and genuine
restart. Cancelled and stale confirmations preserve database, WAL, and key bytes.
Synthetic home markers remain equal. Actual keyboard actions choose Delete
History, reach consent, cancel with Escape, and return focus.

Root reviewed light and dark native images, including 360-pixel width at 200%
text and a stale error. The dialog scrolls vertically without horizontal overflow.
The populated deletion image also remains readable. These images do not establish
complete assistive-technology or OS scaling acceptance.

The collection journey passed in 223.479 seconds. A selected local subscription
has an explicit zero price. Another profile has an unavailable price. Two owned
provider operations stay pending before deletion. The actual GUI removes the
selected profile. Its worker exits, and its late completion refuses publication.
The other worker stays alive with its completion pending until explicit cancellation.

A read-only encrypted-copy oracle compares all 42 current tables. Only rows
owned by the selected profile disappear. Other records, accounts, delivery
journals, independent local usage, homes, and the database key remain unchanged.
A genuine close and reopen preserves the deletion result. This journey does not
exercise scheduled monitoring. Its historical output used an inaccurate scheduling
flag. The harness now states that scheduled monitoring was not exercised.

All 18 selected native variants passed. These include profiles, collection,
activity and quota history, activity and quota persistence, both reading exports,
application export, backup inspection, login, the full login wait, restore preview,
restore apply, checkpoint recovery, local deletion, and three monitoring modes.
The storage slice supplies populated account and delivery-journal fixtures.

The monitoring removal case passed in 25.924 seconds. Public typed commands seed
accepted pairs and complete actual synthetic sign-in for both profiles. Matching
completion and account validation grant runtime eligibility. Actual GUI Start
reaches a pending scheduled worker. GUI Delete History revokes the selected grant,
reaps that worker, and preserves the other grant. All 42 tables match the scoped
deletion projection. Stable homes, provider markers, the key, and restart match.
Earlier manual receipts and deleted-profile start and refresh calls refuse.
This case does not replay the hidden scheduled request or measure its interval.

The separate monitoring regression passed in 360.344 seconds. Its scheduled
starts were 300.055998288 seconds apart. It checks scheduled and manual publication,
pause, authentication expiry, restart, and normal-exit cleanup. Saved-reading
export passed in 31.082 seconds. These are functional checks, not release timing.

Four independent app guard mutations disable consent, revision checks,
selected-session cleanup, and preview-profile matching. Each named test fails.
Exact restoration passes all six Workspace tests and three profile-removal
contracts. Every frozen artifact and compiled source digest stays unchanged.
The primary review covers correctness, privacy, maintenance, and requirements.
Existing controls and provider cleanup keep one app path. No dependency or schema
change is needed. The prepared outgoing view avoids a fallible read after commit.

## Preserved failures and corrections

The first app test command ended with code 101 without a named diagnostic.
A compile-only diagnostic on unchanged source passed. A corrected deterministic
fixture then produced the named RED result. The initial failure remains unexplained.
No compiler defect is claimed from a generic error counter.

The first native profile run refused the private clipboard helper because the
owned display marker was missing. A private GTK prerequisite check passed.
The corrected runner scopes the marker to its Xvfb child. The next profile run
passed. The clipboard guard itself remains unchanged.

The first affected regression batch stopped at 600.008 seconds during login.
That journey intentionally measures the real 900-second application wait.
The input deadline was too short. Its pending heartbeat and failed receipt remain.
Root stopped only its identified private driver group and bus after bounded runner cleanup.
A fresh output label and 1080-second deadline permit the required real wait.
The corrected journey passed in 914.620 seconds. Its measured application wait
was 901.263 seconds from submit. No application clock or timeout changed.

The first restore-preview run selected a hidden profile-dialog Cancel button.
Its document-wide text selector preceded the visible restore form. A scoped
restore-form selector and a closure check corrected the interaction. A fresh
native run passed in 55.050 seconds. The original failed screenshot and receipt
remain. No restore safety assertion or application behavior changed.

The first monitoring-removal setup expected a new automatic sign-in pair.
Its pre-login seed consumed the manual cooldown. The one-shot automatic request
then refused that cooldown. The fixture now seeds accepted pairs and completes actual synthetic sign-in through
the public typed login commands. It checks matching completion and account validation.
Start and Delete History remain GUI actions. This correction changes no application
retry behavior.
The failed run stopped before scheduled deletion. Its evidence remains private.

The next monitoring fixture reached its pending scheduled worker. It then
incorrectly required mode 0700 on the transient working directory. Rust places
that directory beneath the checked private managed home. Its seven environment
directories are 0700. The corrected test checks these actual privacy boundaries,
canonical paths, ownership, and no links. It still requires exact runtime removal
and unchanged stable home files. Seven isolated filesystem cases passed.

Two concurrent native journals passed, but their Xvfb wrappers exited with code 1.
Both used display `:100`. A controlled private probe reproduced the `-a` allocation
race: both Node clients passed, and one wrapper failed during cleanup. Its stderr
had the same 63-byte excess as the original runs. Original raw output was discarded,
so that attribution remains corroborated rather than a captured original diagnostic.
Atomic `-d` allocation fixed the probe. Fresh removal and export runs passed on
separate displays with recorded Node and wrapper exit codes of zero. No app or
harness source changed for this runner correction. The default monitoring run
already passed its outer process check and remains separate evidence.

## Limits

Logical deletion makes no physical-erasure promise. Retained delivery controls
and account records remain in complete exports and backups. Earlier backups can
restore removed history through the separate restore flow. Tool sign-in and
provider subscriptions remain outside the operation.

Other platforms and real-provider removal remain unverified. WSL removal stays
unavailable. Native notification calls on other platforms can outlive a deadline,
so elapsed time never proves cancellation. No new OS notification-delivery race
experiment ran. Existing dispatcher locking and storage journal tests provide the
recorded ordering evidence. The full repository audit remains incomplete.

Private evidence uses `.recovery/audit/s08a2-*` and
`.recovery/native-runs/s08a2-20261112/`. The current source needs its own hosted
CI result after the authorized push.
