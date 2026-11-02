OpenUsage shows what you have used and what you have left across your AI coding tools.
Use Linux commands directly. Check the executor and working directory first.
Never push, publish, create pull requests or call GitHub write APIs. Commit locally only.
Before edits, check Git status and preserve unrelated work. Read docs/implementation-status.md and the SRS IDs that the task names, not the entire history. docs/README.md maps every document.
docs/SRS.md is normative, and only the owner changes it. Record conflicts in docs/srs-clarifications.md. The user controls execution.
Follow SRS Section 2.3. Every decision improves user, developer and agent experience together and breaks nothing. Add no taglines, helper text or secondary labels; if a screen needs them, fix the layout.
Use one bounded assignment with acceptance criteria and a stopping condition. One primary agent is the default. Delegate only independent, authorized work with separate ownership. Report and stop after the local commit.
Use scripts/engineering.py doctor and verify; follow docs/engineering-workflow.md. Use deadlines, targeted checks and no automatic retries. Run full renderer/workspace checks at applicable milestones. Native acceptance and affected security-critical checks remain mandatory.
Keep secrets, provider sessions, processes, storage and exact arithmetic in Rust with narrow typed IPC. Never inspect existing credential homes, browser cookies or other tools' logs without explicit approval. Test with private temporary homes and synthetic data.
Missing metrics are not zero. Email is not verified identity. Time alone does not confirm a reset. Claim only executed evidence, with its real/synthetic and platform scope.
One fact has one home. Update the doc that owns it in the same change, as docs/README.md lists. Treat the records listed there as history, not instructions.
Use the project root, ordinary Git commits and one shared Cargo target. Preserve secret-check hooks and review staged paths. Never publish all refs or create publication mirrors. Keep unfinished work explicitly unverified.
Follow docs/artifact-retention.md and reuse scripts/storage.py. Normal verification retains metadata, not binaries. Preserve source, history, active work, credentials, databases, backups and unique evidence. Remove only integrated clean worktrees after preserving unique work.
