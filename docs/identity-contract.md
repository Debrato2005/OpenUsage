# Identity contract

Status: **Draft. Owner approval pending.** Reviewed November 12, 2026, against
source `a1438a6`. This document describes current identity limits and the gates
for future attribution. It does not approve a provider identity interface or
enable verified aggregation. [SRS Section 4](SRS.md#4-profile-identity-and-authentication)
and Section 8 remain normative.

## Identity and local references

| Reference | Meaning | Identity limit |
|---|---|---|
| Profile UUID | One local profile | Two profiles can use the same provider account |
| Managed home reference | One approved credential environment | A separate home does not prove a separate account |
| Binding UUID | One accepted local association with a home and account description | It is not a provider-issued account key |
| Collection-run UUID | One accepted method result | It identifies an observation, not a person or account |
| Email and plan | Descriptive fields from the authenticated account response | Equal or different values do not prove account equality or distinction |
| Verified identity key | A stable account/workspace key from an approved official interface | The current collector supplies none |
| Log account key | An identifier reported in local logs | It cannot establish verified provider identity |

[Data contracts](data-contracts.md#meaning-of-identifiers-and-provenance) owns
record formats and provenance. [ProfileBinding](../crates/openusage-storage/src/activity.rs)
keeps local profile, home, and binding references together. Its
`ProfileIdentityEvidence` holds email and optional plan. The
[profile writer](../crates/openusage-storage/src/profiles.rs) creates native
profiles without assigning an `account_id`.

The [base schema](../crates/openusage-storage/src/schema.sql) can represent
verified accounts. That schema alone establishes no identity source.
[Association records](../crates/openusage-storage/src/schema_v2.sql) and the
[current collection schema](../crates/openusage-storage/src/schema_v3.sql)
restrict accepted bindings and remote readings to provisional identity with a
null verified key. Descriptive association records are separate from the planned
redacted reading-evidence contract in SRS Section 5B.

## Confidence and accepted readings

| SRS confidence | Required evidence | Current implementation |
|---|---|---|
| `verified` | Stable provider identity under an approved official contract | No accepted collector path |
| `provisional` | Authenticated descriptive account evidence without a stable key | Accepted Codex quota and activity readings |
| `unknown` | No usable identity evidence | The account parser can return Unknown. Collection refuses new attribution |
| `conflict` | Current evidence contradicts the accepted association | Typed failures and explicit association review protect the old binding. There is no stored four-state confidence enum |

The [account parser](../crates/openusage-codex/src/lib.rs), `parse_account`,
recognizes the pinned ChatGPT account shape. A present email gives provisional
confidence. A missing email gives unknown confidence. Storage also requires
bounded, nonblank descriptive evidence. An unsupported account type cannot
become a ChatGPT-plan usage account.

The [session collector](../crates/openusage-codex/src/session.rs) requires
ChatGPT authentication, `requiresOpenaiAuth`, and provisional account evidence
before measurement requests. It compares authentication, confidence, email,
plan, and the authentication requirement before and after collection.
A mismatch refuses the collection. Equal descriptions detect no change. They
do not prove stable identity, and cannot detect every switch between accounts
that return the same description.

The pinned response can include workspace routing fields. The parser checks
routing presence for startup handling. It does not promote `chatgptAccountId`,
`backendOrigin`, or `accountRoutingOverride` to verified identity or a network
target. [SC-006](srs-clarifications.md#sc-006-initial-account-snapshot-before-measurements)
owns the narrow startup-notification rule. Later `account/updated` notifications
invalidate the read, even when their descriptive fields match.

## Binding and reassociation

The [native coordinator](../apps/desktop/src-tauri/src/provider.rs) retains
executable approval, generation, request ownership, and physical-home checks.
These checks protect the selected local operation. They do not certify a
provider account or metric scope.

The [collection writer](../apps/desktop/src-tauri/src/workspace/collection_storage.rs)
compares new evidence with the saved binding and description. The first accepted
reading can create a binding. Changed email, plan, or home requires a separate
review before a replacement binding. This conservative rule can request review
for a plan change without claiming that the account changed.

[Association review](../apps/desktop/src-tauri/src/workspace/activity_storage.rs)
shows the previous and current email and plan, plus whether the home changed.
The [review fence](../apps/desktop/src-tauri/src/provider/activity.rs) expires
after 300 seconds. It binds approval to the operation, generation, physical
home, and reviewed description. The native command also checks the exact old
binding and explicit consent. A subsequent read must match the reviewed
new description. Expired or invalid reviews require a new review.

The [storage transaction](../crates/openusage-storage/src/activity.rs) compares
the expected previous binding before it creates the new association. It clears
current quota and activity caches for that profile. Old accepted observations
retain their original binding and description. They never move to the new
association. A failed transaction preserves the prior state.

The [combined writer](../crates/openusage-storage/src/collection.rs) accepts
supported quota and activity results in one transaction. Each method keeps its
own run UUID. When both capabilities are unsupported, no new association forms.
A failed supported pair cannot publish half of the pair.

Disconnect preserves history. Archive and Delete History have separate consent
and retention rules in the [profile-removal boundary](architecture.md#profile-removal).
Neither operation proves identity or cancels a provider subscription.

## Aggregation and duplicates

The current official collector supplies provisional identity and unknown
application scope. Verified combined totals therefore remain Not available.
The [combined heatmap](../apps/desktop/src-tauri/src/workspace/activity_storage/heatmap.rs)
returns no included profiles and gives exclusion reasons. The
[dashboard projection](../apps/desktop/src-tauri/src/workspace/activity_storage/heatmap/aggregation.rs)
also requires both identity and scope proof for its verified view.
Its live inputs supply neither proof.

The projection contains duplicate-key handling for future qualified inputs.
It chooses the newest accepted compatible source, then the lexicographically
smaller profile UUID for equal times. Synthetic tests of that algorithm do not
qualify a live identity source. Email, profile names, home paths, plans, and
matching usage never become deduplication keys.

[SC-010](srs-clarifications.md#sc-010-approved-unverified-selected-profile-activity)
permits the separate Profile sum view. It sums observed daily values from the
same pinned provider contract and matching provider dates with exact arithmetic.
Its Unverified mark and detail notice retain possible overlap, unknown identity,
unknown scope, and missing coverage. Known duplicate keys and incompatible
contracts are excluded. Unknown duplicates can still count twice.
[SC-015](srs-clarifications.md#sc-015-all-accounts-as-overviews-opening-view)
records the approved all-accounts opening state. Neither exception enables
verified distinct-account totals.

## Local logs and other sources

Current local usage does not link logs to provider profiles. Unknown account
stays unknown. The [session parser](../crates/openusage-locallogs/src/session.rs)
retains only supported report metadata. The data contract maps current local
records. The planned `log_accounts` mapping and the owner's OQ-1 decision remain
open. Folder names, email, matching time, and matching counts cannot supply
that mapping. Session quota movement still waits for it.

Under OU-ID-013, an Unofficial or local-log account ID cannot become a verified
identity key. SRS Section 5D can use a shared provider-issued ID for source
association only within its rules. Cross-source duplicate reconciliation remains
unbuilt. Provider and local token values stay separate.

The planned Claude Code status-line quota adapter has no account ID under
OU-SRC-015. Its required identity-unverified result does not become implemented
support through this document. The leaderboard's GitHub identity and random
device ID also cannot establish provider account identity.

## Gates before verified aggregation

These gates remain open. This draft does not approve an identifier, migration,
provider scope, or acceptance result.

1. Record the approved official method, provider version, stable key, key scope,
   provider namespace, and behavior across reconnects, account switches,
   workspaces, and plans.
2. Establish compatible metric populations and provider-day definitions.
   Retain unknown scope when the provider gives no guarantee.
3. Add reviewed storage and interface mappings for the approved confidence
   states. Preserve old bindings, observations, backups, and exports.
4. Test duplicate selection, conflicting identities, missing keys, stale work,
   account switches, and incompatible scopes with synthetic fixtures.
5. Run the live identity-contract test in OU-ID-012 and AT-45. Record private
   evidence and a redacted result. Obtain owner approval before activation.

AT-60 and source-audit qualification remain separate gates. The
[acceptance log](acceptance-traceability.md#evidence-log) owns executed results.
Existing source tests include association rollback and immutable history in
`activity.rs`, explicit changed-identity review in Workspace `collection_tests.rs`,
and duplicate selection in the dashboard projection. Native journeys cover
[activity persistence](../tests/native/activity-persistence.mjs) and
[combined collection](../tests/native/collection.mjs) with private synthetic data.
This document cites their scope. It does not record a new execution of them or
qualify real accounts, Windows, macOS, or WSL.
