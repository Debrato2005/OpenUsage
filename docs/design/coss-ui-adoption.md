# coss/ui adoption

Updated: October 18, 2026. The user selected coss/ui for all OpenUsage components and workflows. Adoption proceeds through native verification. Verified slices cover shared buttons, text inputs, the sign-in address field, quota meters, appearance selection, profile management disclosure, and provider form choices. Remaining disclosures, feedback panels, and record views remain in the migration queue.

## Source and licensing

Reviewed repository: [cosscom/ui](https://github.com/cosscom/ui), revision `a597c6306a1c60e9f11e22e38bdf4a15f624b0f2`. Component source comes from `apps/ui/registry/default/ui/`. The upstream [licensing statement](https://github.com/cosscom/ui/blob/a597c6306a1c60e9f11e22e38bdf4a15f624b0f2/LICENSING.md) assigns MIT to `apps/ui/`. Its package manifest also declares MIT. No code from the repository's AGPL directories was copied.

The distribution retains the upstream MIT notice in `apps/desktop/public/licenses/coss-ui-MIT.txt`. Upstream has no separate notice in `apps/ui/`. The retained notice comes from its original MIT component directory, `apps/origin/LICENSE.md`. Source hashes and the reviewed licensing files accompany the verification evidence.

The adapters use Base UI 1.9.0, class-variance-authority 0.7.1, clsx 2.1.1, and tailwind-merge 3.4.0. Installation disabled lifecycle scripts. No registry CLI, generator, hook, service, or remote stylesheet runs in the application.

## Application adaptations

Local component files preserve coss composition and use OpenUsage theme tokens. JetBrains Mono, tabular numerals, readable density, 44-pixel control targets, and visible focus remain mandatory. The token bridge distinguishes muted text from muted surfaces. Explicit themes and the system theme use the same semantic colors.

Button variants distinguish primary, outline, and ghost actions. Existing request ownership and form handlers remain in workflow modules. The Input native branch retains HTML validation, password autocomplete, labels, and focus references. One small adapter property keeps sign-in code typography on the actual input. The Textarea adapter retains selectable read-only address text.

The [coss Meter](https://coss.com/ui/docs/components/meter) presents quota remaining. Its visible label and tabular value sit above the track. Rust supplies the derived percentage. The UI does not calculate usage or infer missing values. The track uses the coss eight-pixel geometry and neutral theme colors. It has no gradient or animated fill. System colors preserve the track and indicator in forced-color mode. Native Windows contrast qualification remains open.

The local spinner uses a CSS border instead of an icon dependency. It preserves the status label without animation. Component transitions and decorative shadows are omitted from this desktop slice. These adaptations retain the user's restrained design and reduced-motion requirements.

## Remaining adoption

The appearance Select and profile/provider Field, Checkbox, and RadioGroup now have native evidence. Security consent remains unchecked. Discovery uses actual radio keyboard selection. Manage profile uses the reviewed coss Collapsible without animation or new dependencies. Active provider operations stay mounted outside it. Forward navigation passed at that milestone. The shared plain-label Button fix now passes strict reverse traversal for all three profile operations and an ordinary privacy button. Full keyboard acceptance remains incomplete. See [the profile review](../reviews/2026-10-18-profile-controls.md) and [the button follow-up](../reviews/2026-10-18-button-hit-testing.md).

Backup/checkpoint Selects, per-password Field contexts, and explicit backup/restore Checkbox consent have five final-artifact and three retained native journeys with explicit source scopes. Choices remain bounded by Rust catalogs. The shared body-portaled picker uses supported fixed positioning and available-width sizing. Both file pickers use nonmodal selection. Backup content status and exact available counts stay visible. Four technical archive fields remain under Details. Ten strict resize states and the full appearance regression passed. Restore comparisons adapt to enlarged text. See [the backup control review](../reviews/2026-10-18-coss-backup-controls.md).

Remaining disclosures adopt coss Collapsible or Accordion with explicit ownership of mounted content. Closing a panel must not silently cancel a provider operation or retain a cleared password. Alerts, empty states, cards, badges, tables, and navigation follow. Shared components remain presentational. Rust keeps provider processes, storage, secrets, and exact arithmetic.

This decision replaces the earlier Primer and React Aria visual recommendation. Those sources informed the comparison. coss/ui is the selected implementation system. Full adoption and full accessibility acceptance remain incomplete.
