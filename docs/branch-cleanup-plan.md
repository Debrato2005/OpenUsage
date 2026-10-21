# Remote branch cleanup awaiting approval

Reviewed October 22, 2026. No remote branch was changed or deleted.

Keep `main`. The [exact candidate list](branch-cleanup-candidates.tsv) contains 219 temporary, recovery and archive branches with their reviewed tips. No active collaboration branch was identified in this review.

## Work preservation

Before preservation, 41 non-main branches were ancestors of main. Another 178 branches contained 260 commits outside main. Review found 234 matching patches already on main, 17 source-empty commits after privacy filtering, and nine snapshots, diagnostics or evolved fixture/lock changes. No missing production feature was identified. This source review does not prove runtime equivalence.

Local merge `6c6e0168e1b2ea16e93a596ac6e451d7f06a0808` records the 84 independent historical tips as ancestry. It leaves the current source tree unchanged. Independent checks verified all 607 prior commit objects and all 219 branch tips remain reachable from main. No existing commit was rewritten. The merge does not add obsolete experimental code to the current application.

The normal root checkout now contains the complete reviewed development history. Additional publication repositories and archive tags are not needed for routine work. Original private recovery data remains local.

## Required order

1. The owner reviews and pushes current `main` with ordinary `git push`. Agents must not push.
2. Fetch the remote again. Confirm remote main contains every candidate tip and each candidate still matches this list. Re-review any changed branch.
3. Obtain the owner's explicit approval for the exact 219 remote deletions.
4. Delete only those approved branches. Preserve `main`, tags and any newly created or changed collaboration branch.
5. Read the remote branch list again and confirm the approved result.

Deletion is not approved by this document. Until the owner pushes main and approves the cleanup, GitHub retains the existing branches. The local pre-push hook also refuses branch deletion when its advertised tip is absent from either local main or the fetched origin/main.

## Verification scope

Gitleaks scanned all selected local and fetched remote branch histories with no findings before the ancestry merge. Full final-main checks include the preservation merge. TruffleHog findings are reviewed synthetic URL rejection fixtures, with credential validation disabled. No exposed credential requiring rotation was confirmed. These tools do not prove the absence of all secrets.

Existing Git author names and email addresses remain in history. They are not credentials. This cleanup does not change author identities or rewrite their commits.

The SRS and application security model are unchanged. No application build or platform release qualification is implied by this Git cleanup.
