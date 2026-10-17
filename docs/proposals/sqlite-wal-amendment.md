# Proposed SRS amendment: standard SQLite with WAL

Status: **DRAFT FOR USER APPROVAL. NOT APPLIED.**

The current SRS still requires SQLCipher. Its SHA-256 is `667005e9d26549d7f0da99918a9e455595feb7fc407d4d3319ac5ee1c083277c`.

This proposal restores the user's stated SQLite decision. It keeps the limit of ten active profiles and the Rust storage boundary.

Approval must explicitly accept this consequence: **OpenUsage will not encrypt the live database, WAL, or shared-memory file at rest.**
Local account, usage, billing, and other application records can be readable to someone who can read those files.
OS file permissions limit access. They do not provide encryption or protect a copied database.
Encrypted backups do not encrypt the live database. Provider credentials remain outside SQLite.

The changes below form one amendment. They do not authorize implementation before approval.

## 1. Database selection

Replace the Section 2 main-database row with:

> | Main database | Standard SQLite 3 through `rusqlite` | Local transactions with OS file access controls |

Replace OU-DATA-001 with:

> **OU-DATA-001:** Standard SQLite shall be the only application-record database. SQLCipher shall not be a required runtime dependency.

Preserve OU-DATA-002 through OU-DATA-026 unchanged. This includes local-only WAL, verified foreign keys, one Rust writer, transactional migrations, and recoverable prior data.
It also preserves FULL synchronization, the 5000 ms busy timeout, prepared statements, consistent snapshots, and WAL-aware backup handling.

## 2. Replace Section 12.2

Replace the section title with `### 12.2 Local database protection and access`.

Use these exact replacement requirements:

> **OU-CRYPT-001:** OpenUsage shall not encrypt its primary SQLite database, WAL, or shared-memory file. Product text shall state this limit.
>
> **OU-CRYPT-002:** Rust shall keep database files and sidecars in the local application-data directory. OpenUsage shall restrict access to the current user.
>
> On Linux and macOS, new managed directories shall use mode 0700. New database files and sidecars shall use mode 0600.
>
> On Windows, managed database files and directories shall use access controls that exclude other ordinary users.
>
> **OU-CRYPT-003:** The primary database shall not require a database key, OS credential-store entry, or application unlock passphrase.
>
> **OU-CRYPT-009:** Backup and retained-evidence passwords and derived encryption keys shall not be saved to plain files or logs. Rust shall control cryptographic operations.
>
> **OU-CRYPT-010:** Monitoring shall not start until Rust opens the database, checks its schema, and completes required migrations.
>
> **OU-CRYPT-011:** A missing, unreadable, corrupt, or incompatible database shall not cause an automatic database reset or replacement.
>
> **OU-CRYPT-012:** The app shall show recovery guidance when the existing database cannot be opened or its schema cannot be read.
>
> **OU-CRYPT-013:** Opening an existing database shall check that SQLite can read its schema before migration. A newer unsupported schema shall remain unchanged.
>
> **OU-CRYPT-014:** Exports shall be treated as potentially sensitive and shall require an explicit save action.

Retire OU-CRYPT-004 through OU-CRYPT-008 and OU-CRYPT-015 through OU-CRYPT-017. Keep their identifiers reserved and mark them superseded by this amendment.
Those requirements select database-key stores or wrap a database key. A standard SQLite database has no such key.

Replace the section's security-limit paragraph with:

> **Security limit:** OpenUsage uses OS file access controls for its live database. It does not provide application-managed database encryption at rest.
> OS disk encryption remains an independent user or system control. OpenUsage does not claim to encrypt provider-owned credential files or OS swap.

## 3. Preserve encrypted backups and credential isolation

Preserve OU-BACKUP-001 through OU-BACKUP-018 and Section 18.4 unchanged.
Backups still require a user password, Argon2id, XChaCha20-Poly1305, unique randomness, authenticated chunks, authentication before restore, and a reversible pre-restore checkpoint.

Preserve OU-DATA-014 and OU-SEC-004 through OU-SEC-010 unchanged. Provider tokens, cookies, and Codex auth-file contents remain outside SQLite and renderer payloads.
Each profile retains its isolated provider-managed credential environment.

Replace OU-SEC-003 with:

> **OU-SEC-003:** The frontend shall not receive tokens, auth-file contents, browser cookies, or backup and retained-evidence encryption keys.

Replace OU-SEC-017 with:

> **OU-SEC-017:** Rust shall keep required temporary database files and recovery checkpoints inside the restricted local application-data directory.
> Those files shall use the same user-only access controls as the primary database. The app shall remove temporary files after their purpose ends.
> Backup archives shall reach the user-selected destination only in encrypted, authenticated form.

Preserve OU-SEC-013 and OU-SEC-018 unchanged.
Preserve OU-IMPORT-011 and the optional encrypted evidence reference in `receipt_imports` unchanged.
This amendment does not authorize plaintext retained receipts or a weaker evidence-encryption mechanism.

## 4. Align workflows, tests, and supporting text

Use these exact replacements:

| Location | Replacement text |
|---|---|
| OU-NFR-001 measurement cell | `30 cold-process launches with an existing readable database; no application database unlock step` |
| Section 18.1, step 1 | `1. **Load:** Open and check the local database. Complete required migrations. Load cached state. Show prior data with its age.` |
| Section 20 storage-crate comment | `# Standard SQLite, WAL, and migrations` |

Replace these complete rows in Sections 19 and 22:

```text
| AT-33 | Start without an OS key store; deny database access or corrupt its schema | A readable database needs no key store; access or schema failure preserves files and shows recovery guidance |
| AT-50 | Test standard SQLite with WAL and power-loss faults | Accepted transactions and prior valid history remain recoverable; product text makes no live-database encryption claim |
| Database | Plain SQLite baseline | Standard SQLite with WAL and user-only file access |
| Encryption | Encrypted backup | Encrypted authenticated backup; live database has OS file access controls |
```

In Section 14.1, replace only `lost encryption keys` with `lost backup or retained-evidence passwords and keys; copied plaintext database files`.
Preserve every other threat and the existing limit for a compromised OS account.

In Section 23, remove the SQLCipher API reference. Renumber the remaining references without changing their content.
Preserve the SQLite WAL reference.

AT-32, AT-40, AT-41, AT-42, AT-51, and AT-53 remain mandatory and unchanged.
AT-42 must examine the database through SQLite, so a plaintext database cannot hide accidental credential storage behind encryption.
Native acceptance still requires executed evidence. Neither this proposal nor existing SQLCipher unit tests satisfy native acceptance.

## 5. Approval and implementation boundary

Required decision: approve or reject this entire amendment, including the explicit plaintext-at-rest consequence.
Until approval, `docs/SRS.md` remains unchanged and SQLCipher remains normative.

After approval, update the SRS and clarification log together. Then replace the SQLCipher dependency and database unlock states.
Keep existing database detection explicit. Never open an encrypted SQLCipher file as a new blank standard SQLite database.
Any existing encrypted user database needs a separate, reversible conversion plan before conversion.
