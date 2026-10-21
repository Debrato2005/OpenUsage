# Native Codex executable discovery

Updated: October 18, 2026. Find Codex now checks bounded installation locations before manual path entry. Discovery does not run a program, download files, or approve execution.

## Workflow and trust

A single result opens source review directly. Multiple results require a selection and Review. An empty result opens manual entry and focuses its path field. A successful search closes that fallback. The existing checksum review and initially unchecked confirmation still control approval. Unknown source bytes remain inspectable, but approval fails before execution.

The typed IPC returns at most 16 unique canonical paths and advisory location categories. Rust checks at most 64 PATH entries, fixed system paths, and three pinned npm vendor paths. It rejects relative entries, excessive symlink hops, unsafe ownership, writable ancestors, unsafe file modes, and unsupported filesystems. It does not enumerate directories or execute npm wrappers.

The boundary step uses only lstat and readlink for known credential roots and their ancestors. It records lexical and physical prefixes for HOME/.codex, CODEX_HOME, and managed profiles. Candidate probing excludes those prefixes before lookup. Missing roots need no creation. Malformed, inaccessible, changed, or ambiguous roots return a safe refusal with manual entry available. No development check used an existing credential home.

Independent review found two scope defects before integration. A configured root alias initially left its direct physical path discoverable. Physical exclusions now cover default and configured aliases. Another alias containing child/.. could inspect a child before returning to the protected root. Boundary resolution now refuses parent-directory targets before traversal. Synthetic lookup spies cover both defects, missing suffixes, changed boundaries, ownership, and bounds.

Boundary metadata checks detect ordinary replacement. They do not form an atomic namespace snapshot against a hostile process with the same user identity. Candidate source review and pre-launch identity checks remain mandatory. The official standalone install can reside inside ~/.codex. That location remains excluded, with manual review available.

## Executed verification

The integrated workspace passed 100 Rust tests, strict workspace Clippy, and formatting. One ignored child entry runs twice through its process-crash parent. Renderer checks passed 22 synthetic contracts and 16 Chromium/WebKit checks. Native-test and default production debug configurations compiled.

The native journey passed empty, unique, multiple, deduplicated symlink, and fixed npm location cases. It refused unknown-source approval. Synthetic default, configured, physical, and managed credential locations stayed excluded. Exact executable and protected marker bytes stayed unchanged. Database, WAL, and protected-key hashes stayed unchanged around discovery. No execution marker appeared. These observations complement the injected lookup tests. They do not prove a syscall-level no-read trace.

Actual keyboard input moved from results to a radio choice, selected it, and opened the matching review. Approval remained unchecked. Seven native captures cover desktop, dark/light narrow views, visible keyboard focus, and 200% test-side text. Root review found consistent typography, wrapped paths, clear refusal, and no horizontal overflow. Large-text results use the page scroll area.

The first native run exposed redundant manual controls after an empty search followed by a successful search. A one-line state change closes manual entry when results appear. The refined native journey checks this transition. The production source gate still rejects both synthetic executable fixtures before execution.

The signed official read-only regression also passed through manual entry, explicit approval, and fresh managed-home checks. It reported authentication required, unknown identity, and stopped monitoring. Storage hashes stayed unchanged. That receipt binds the preceding production binary. The final UI differs only in the Find-result state that closes manual entry. No provider, source-verification, permission, or Rust code changed after that official run. The final native discovery and production source-gate receipts bind their own rebuilt binaries.

## Evidence and remaining work

See `../evidence/native-provider-discovery/`, `../evidence/native-discovery-source-gate/`, `../evidence/native-discovery-official-check/`, and `../evidence/native-discovery-verification/` for receipts and source hashes. The independent review covers the final boundary implementation.

SQLCipher 4 with WAL remains mandatory. The SRS is unchanged. SC-003 retains its approved narrow challenge-display scope. Discovery adds no login, credential migration, verified identity, monitoring, or quota claims. Native tests used synthetic HOME and CODEX_HOME before launch. Fixed-location emptiness is a visible environment assumption in this runner. Other hosts, native screen readers, release performance, Windows, and macOS remain unqualified.
