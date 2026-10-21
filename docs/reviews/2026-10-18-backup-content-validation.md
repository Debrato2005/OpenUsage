> Public source copy: raw acceptance evidence remains private. Historical test statements keep their original scope and do not certify this source snapshot.

# Backup content validation review

Archive authentication establishes integrity of the supplied bytes. OpenUsage also treats the database image as untrusted input. Native inspection validates supported contents in memory before returning record counts. It never opens the image as live storage, migrates it, creates a checkpoint, or applies records.

The storage validator requires the current schema version and compares the complete schema catalog with a reference built from trusted application migrations. It then runs full SQLite integrity and foreign-key checks and validates profile, account, and settings invariants. SQLite documents that [integrity checks do not detect foreign-key violations](https://www.sqlite.org/pragma.html#pragma_integrity_check), so both checks are required.

The isolated connection enables defensive mode, disables trusted schema, views, triggers, and extension loading, and uses query-only access with memory temporary storage. SQL and schema limits reduce parser exposure. A progress callback stops SQL after 60 seconds of elapsed time. These controls follow [SQLite guidance for untrusted databases](https://www.sqlite.org/security.html). They do not impose a global heap limit on other live connections.

The authenticated payload uses a size-bounded, fallible Rust buffer. The validator makes a separate zeroizing copy with 20 readable padding bytes. SQLite borrows this allocation read-only. Rust retains ownership; the connection closes before the buffer is wiped on success or error. The padding and lifetime rules follow the [deserialize contract](https://www.sqlite.org/c3ref/deserialize.html). No plaintext database file is created.

Current accepted settings are UTC and en-US. Other non-corrupt settings return unsupported contents after the remaining records are checked. Future settings writers must expand this guard. Account strings use existing protocol bounds; stored identity confidence is checked for consistency and does not establish a fresh provider identity. Wide counts remain decimal strings at IPC.

The native journey passed with valid profile records, authenticated invalid data, future versions, private staging, and preserved files. Its receipt and nine screenshots are in `../evidence/native-backup-content/`. Rust checks also rejected altered schema objects, unsafe home references, invalid dates, orphaned records, and eleven active profiles.

The 5 GiB archive limit remains mandatory. Inspection currently needs two image-sized Rust buffers plus SQLite resources. Large-history memory use and elapsed-time behavior remain unqualified. This implementation does not complete restore workflow steps 6 through 8 or AT-32. SQLCipher 4 with WAL remains the live storage architecture.

## Current schema export verification

The export verifier now requires JSON with a password and expected schema version. It compares that version with both the authenticated manifest and the database in memory. Its report gives the actual version. The executable regression first failed, then passed schema 4 acceptance and mismatch refusal. Malformed requests also failed without changes to the synthetic files.

The native export passed with explicit consent, matching passwords, private file modes, and authenticated contents. Wrong passwords and tampering failed. Staging stayed empty. The snapshot retained the saved dark theme. The screenshot was reviewed. Application, storage, provider, and embedded assets remain unchanged. Only the independent tool and its harness changed. This check covers appearance and profile count. It does not compare every database record. The earlier thirteen-table restore checks retain their separate scope.

See the current native export receipt (private evidence omitted). Test source: `289bd1a`. The standalone executable regression, strict Clippy, formatting, and script syntax checks passed.
