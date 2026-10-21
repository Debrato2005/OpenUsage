# Completed remote branch cleanup

Completed October 22, 2026, after the owner's approval: "review then delete after published". GitHub now contains only `main`.

The [historical candidate list](branch-cleanup-candidates.tsv) records the 219 deleted temporary, recovery and archive branches and their reviewed tips. No active collaboration branch was identified in this review.

## Work preservation

Before preservation, 41 non-main branches were ancestors of main. Another 178 branches contained 260 commits outside main. Review found 234 matching patches already on main, 17 source-empty commits after privacy filtering, and nine snapshots, diagnostics or evolved fixture/lock changes. No missing production feature was identified. This source review does not prove runtime equivalence.

Local merge `6c6e0168e1b2ea16e93a596ac6e451d7f06a0808` records the 84 independent historical tips as ancestry. It leaves the current source tree unchanged. Independent checks verified all 607 prior commit objects and all 219 branch tips remain reachable from main. No existing commit was rewritten. The merge does not add obsolete experimental code to the current application.

The normal root checkout now contains the complete reviewed development history. Additional publication repositories and archive tags are not needed for routine work. Original private recovery data remains local.

## Executed sequence

1. The owner published `main` at `af70f601b864ed542e70e61c73ef93b90e811543` with ordinary Git.
2. A fresh fetch and remote read confirmed that published main preserved all 219 candidate tips. Every candidate still matched its reviewed tip. No changed or additional branch was included.
3. The owner's conditional approval covered the reviewed list after publication. The deletion hook also checked Git's advertised tips against the list to catch changes after preflight.
4. One atomic operation deleted only those 219 approved branches. No source commit was pushed, existing commit rewritten, main updated or tag deleted.
5. A fresh remote read confirmed exactly one branch, `main`, with the same hash. The remote default branch remains main. Stale local tracking references were pruned.

No further remote action is approved by this record. The candidate list is historical evidence, not an automatic cleanup instruction. Future deletions need fresh review and approval. The local pre-push hook refuses deletion when the advertised tip is absent from either local main or fetched origin/main.

## Verification scope

Gitleaks scanned all selected local and fetched remote branch histories with no findings before the ancestry merge. Full final-main checks include the preservation merge. TruffleHog findings are reviewed synthetic URL rejection fixtures, with credential validation disabled. No exposed credential requiring rotation was confirmed. These tools do not prove the absence of all secrets.

Existing Git author names and email addresses remain in history. They are not credentials. This cleanup does not change author identities or rewrite their commits.

The SRS and application security model are unchanged. No application build or platform release qualification is implied by this Git cleanup.
