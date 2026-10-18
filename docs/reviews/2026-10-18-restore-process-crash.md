# Restore process-crash boundaries

The storage tests now terminate owned child processes on both sides of the production restore COMMIT. They exercise real SQLCipher WAL writes. They do not simulate power loss or establish native-window crash recovery.

The pre-commit child stops inside a test-only SQLite update hook for the actual settings insert. Production code already deleted and replaced the account and profile rows. A small page cache forces dirty pages into the encrypted WAL. The parent observes 29 frames with no commit marker, sends SIGKILL, and reaps its child. Reopening storage returns every previous record column.

The second child completes the production replacement, then stops before it acknowledges success. Its WAL contains 38 frames and one commit marker. After SIGKILL and reopen, every incoming record matches the intentional restore transformations. Homes use fresh unique references, profiles require authentication, and monitoring stays stopped. Observation history stays unchanged. One bounded operation timestamp updates the intended fields.

Both cases preserve the original encrypted checkpoint bytes. The tests authenticate its pages and recover every previous record column. Schema, foreign keys, domain checks, WAL mode, and FULL durability remain valid. These checks establish atomic process-crash outcomes, not filesystem or device durability.

The parent uses an atomic barrier publication, a 15-second deadline, an exact child-test selector, and a private synthetic fixture. Its cleanup always terminates and reaps the owned child. The child entry is ignored by the default test runner because the parent invokes it twice with controlled arguments. No production fault hook, environment switch, or IPC command was added.

OU-BACKUP-010 requires preservation of the previous valid database after interruption. Before COMMIT, previous live records survive. After COMMIT without acknowledgment, complete restored records survive, and the checkpoint preserves previous records. A lost reply cannot establish rollback. SC-002 records that boundary without changing the normative SRS. Full acceptance still needs native interruption, home cleanup, other platforms, and power-loss qualification.

The integrated workspace passed 82 tests. The one ignored child entry ran twice through its parent test. Strict Clippy and formatting passed. The receipt binds storage source, manifests, lockfile, test executable, and logs. Its original executable remains preserved under `.recovery/artifacts/storage-crash/`.

This slice changes only test code and its evidence. The preceding native checkpoint journey still identifies its exact source and executable. Its production storage code remains unchanged. The later source difference adds a module behind `cfg(test, unix)` and the owned-process tests. Native screenshots are historical executed evidence for that preceding milestone, not a new GUI run of this test-only change.
