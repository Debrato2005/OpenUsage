# Desktop design foundations

Updated: October 20, 2026. These foundations cover the native workspace, account overview, backup controls, and provisional quota preview. The user selected coss/ui for all components. The [adoption record](coss-ui-adoption.md) controls the incremental migration. Charts and accepted history need further native qualification.

## Structure and interaction

The overview places storage state first, followed by a compact summary and accounts. Backups use a disclosure below accounts. Recovery messages retain the original database and key. Missing provider readings show “Not available”. Only the unlocked database supplies the profile count.

Privacy stays accessible in narrow windows. Native controls retain keyboard behavior, visible focus, labels, and password-manager support. Errors appear near their controls. A completed backup closes its form, clears passwords, and returns focus to the disclosure button. Its receipt stays visible. The original foundation slice added no animation or UI dependency. The later coss adoption adds the reviewed shared controls.

Restore preview compares the current checkpoint with incoming backup counts. The first native capture showed broken column headings at 200% text size. An em-based container query now presents each record category with two labeled values when space is limited. Explicit table roles retain the comparison structure. Five native captures cover desktop, both narrow themes, errors, and enlarged text. Native DOM and keyboard checks passed; screen-reader acceptance remains open. See `../evidence/native-restore-preview/`.

The layout uses one page scroll area, a 204-pixel sidebar, and a 1,400-pixel content limit. At smaller widths, navigation becomes a compact header and summary cells stack. Content wraps instead of concealing overflow. Primary buttons and text fields have a minimum height of 48 CSS pixels. Navigation retains the same minimum height; compact secondary controls remain at least 44 CSS pixels.

## Typography and tokens

DM Sans serves interface text and exact values, following the user's latest font choice. It replaces Geist Sans and JetBrains Mono. The supplied reference guides the sidebar, paired overview panels, summary strip, and account table. Missing readings remain explicit. The October 21 readability update enlarges shared text and controls, following the user's request for a larger, softer interface. It changes the existing tokens without adding panels, animation, or dependencies.

| Role | Family | Size | Weight | Line height |
|---|---|---|---|---|
| Metadata | DM Sans | 13 px | 400 | 1.5 |
| Supporting text | DM Sans | 14 px | 400–500 | 1.5–1.55 |
| Body and controls | DM Sans | 16 px | 400–500 | 1.5 |
| Section headings | DM Sans | 18 px | 600 | 1.3 |
| Page title | DM Sans | 30 px | 500 | 1.3 |
| Exact overview total | DM Sans | 32 px | 600 | 1.25 |

Sizes use `rem` units. Numeric roles use tabular digits, no ligatures, and zero letter spacing. Dates also use tabular digits. OpenUsage bundles and preloads the variable font. `font-display: swap` keeps text available during loading. Synthetic font weights remain disabled. The app makes no font request to a remote server.

[Vercel typography guidance](https://vercel.com/geist/typography) informed the original type hierarchy. The current sizes prioritize the user's readability feedback. Taste informed the restrained weights and spacing. The user's desktop requirements control the layout.

CSS tokens define typography, spacing, surfaces, borders, radii, focus, errors, and interaction states. Spacing follows a 4-pixel base. Light and dark themes use neutral surfaces. Green identifies an open encrypted workspace. It does not decorate unrelated panels. Panels use borders without shadows.

## Reviewed guidance and scope

Reviewed copies remain outside the repository. No installation script from those repositories ran. UI/UX Pro Max supplies hierarchy, density, and accessibility guidance. Taste supplies visual restraint. Vercel guidance supplies semantic controls, focus, form feedback, local font loading, and React state practices.

| Guidance | Reviewed revision |
|---|---|
| [UI/UX Pro Max](https://github.com/nextlevelbuilder/ui-ux-pro-max-skill) | `56ea9cf174ed86786187daa9de0e6247cb0747e4` |
| [Taste minimalist skill](https://github.com/Leonxlnx/taste-skill) | `f624f37a8c650b9cc550253104925efe4c8de246` |
| [Vercel agent skills](https://github.com/vercel-labs/agent-skills) | `063bee94c3f4df8453406c830b0a7df0f2860278` |

The UI/UX search did not return an exact desktop account workflow. Its landing-page layouts and unrelated monitoring examples were not applied. Taste recommendations for editorial typography, large spacing, gradients, and reveal motion conflict with this desktop task. The user’s restrained desktop requirements control those choices.

[Linear documentation](https://linear.app/docs) informed the separation of navigation and work areas. [Raycast guidance](https://developers.raycast.com/basics/prepare-an-extension-for-store) informed functional action names and concise descriptions. The app does not copy their branding or layouts. The [Vercel web guidelines](https://github.com/vercel-labs/web-interface-guidelines) inform the accessibility review. Automated checks do not establish full WCAG 2.2 AA conformance or screen-reader acceptance.

## Font provenance

The font comes from the [official DM Fonts repository](https://github.com/googlefonts/dm-fonts/tree/4412393b7d2de9fe7a92064c2dce9b5af5d7fd26/Sans), revision `4412393b7d2de9fe7a92064c2dce9b5af5d7fd26`. OpenUsage bundles the original variable TTF unchanged. It supports normal weights from 100 to 1000. The interface uses 400, 500, and 600. The adjacent `DM-Sans-OFL.txt` retains its full copyright and SIL Open Font License 1.1.

This change adds no font dependency. Previous evidence retains the font choice tested at that time.

Original path: `Sans/fonts/variable/DMSans[opsz,wght].ttf`. Bundled path: `apps/desktop/public/fonts/DMSans-Variable.ttf` (252,888 bytes). SHA-256: `a023f465d2f26d8f56c6e3cac6aece08b7a66eb75b4d75eeef9df838c53c81f6`.

License path: `apps/desktop/public/fonts/DM-Sans-OFL.txt` (4,482 bytes). SHA-256: `9af36190332437f5ecd09974de43c1f7c77a310a996cdd8ceb25628b458840e1`.

The rebuilt native app passed 24 reviewed visual captures with DM Sans. Visible exact totals stay on one scrollable line at narrow widths. The renderer also verifies keyboard scrolling at normal and doubled text size. Monitoring remains a separate acceptance boundary. See `../evidence/native-dm-sans/`.

## Managed sign-in

Sign-in starts with the selected profile name and one consent action. Showing consent contacts no provider. Advanced executable size and checksum remain accessible under a disclosure. The prompt keeps the plain address, selectable code, and Cancel action together. Results receive focus and preserve uncertainty about identity. Credentials already saved by Codex can remain after cancellation.

Native review found an internally clipped URL at enlarged text size. A wrapped read-only field now shows the complete address without scrolling inside the field. Keyboard selection repeats on the next frame only if that field still has focus. Seven actual native captures cover both themes, narrow windows, 200% test-side text, and visible keyboard focus. The harness activates only the identified owned native window before keyboard checks. See `../evidence/native-device-login/`. These checks do not establish screen-reader or full WCAG acceptance.


## Executable selection

The primary Add action prepares the pinned official companion automatically. If it is absent, Rust checks supported installation locations. A present invalid companion fails closed. Account sign-in retains explicit consent. Find, manual paths, and technical source review remain in Settings for troubleshooting. See the [source review and decision](../reviews/2026-10-21-onboarding-sources.md).

Long paths wrap in the choice list. Size and checksum remain under Executable details. Seven refined native captures cover the empty state, both themes, narrow review, keyboard selection, source refusal, and 200% text. The large-text list uses the page scroll area. See `../evidence/native-provider-discovery/`. The UI explains metadata checks without exposing provider or authentication internals.

## Quota preview and coss controls

The first coss slice replaces common controls across existing screens and adds a labeled remaining-capacity meter. Profile actions precede the preview. Remaining capacity and its tabular percentage sit above an eight-pixel track. Used percentage and expected reset remain below. Missing data stays explicit. Limit and source details use progressive disclosure.

Native review fixed the percentage wrapping at 200% text. Seven final quota captures passed visual and geometry checks. Native sign-in and backup-content regressions checked the visible coss control wrappers, password clearing, selected read-only text, and error/focus states. These checks qualify the first shared-control slice. Selection controls, disclosures, feedback components, and record views still need migration. See `../evidence/native-quota-preview/` and `../evidence/native-quota-verification/`.

## Compact quota context

The current quota view replaces the long introductory paragraph with one title, visible fixture provenance, identity uncertainty, saved status, and observation time. About this preview retains the monitoring, history, derived-value, and reset explanations. The result uses a separator instead of another bordered card. Related windows share columns on wider screens and stack on narrow screens. Their details use an em-based wrap threshold so enlarged reset dates have room.


## Backup and recovery controls

Backup and checkpoint choices now use coss Select. Password inputs each have their own Field context and explicit label. Backup and restore consent remain separate, initially unchecked actions. Native checks cover their real keyboard and pointer behavior.

The shared picker uses supported fixed positioning for body portals and bounds its popup to the available width. Native investigation reproduced stale document extent with absolute positioning even without scroll locking. Both file pickers now pass desktop-to-narrow open-menu resizing. The full appearance regression also passed. No global overflow clipping conceals content.

At enlarged text size, comparison row labels use the full row width. Labeled values stack when two columns would crowd their text. Whole-word and overflow checks remain strict. Five final-artifact and three retained native journeys map to 46 captures. Their tested/current source scopes remain explicit. Strict profile reverse traversal and full accessibility acceptance remain open. See [the review](../reviews/2026-10-18-coss-backup-controls.md).


## Reference workspace layout — October 20, 2026

The reference layout now uses four views: Overview, Accounts, Usage, and Settings. The overview contains two panels, a compact activity strip, and an account table. Connection approval, appearance, and encrypted backups belong in Settings. Account creation and detailed readings use disclosures.

At that milestone, JetBrains Mono served all interface roles. The later typography refinement below replaces this choice. Shared coss controls retain their keyboard behavior and focus states. Light and dark themes use the existing tokens. The sidebar becomes a compact navigation grid in narrow windows. Tables can scroll horizontally without hiding columns.

The overview shows one selected profile and one quota bucket at a time. Rust supplies quota percentages, reset state, freshness, and exact count strings. Unknown totals stay unavailable. Identity uncertainty and synthetic data labels remain visible. No combined donut or date filter appears without supported data.

Provider operations stay mounted under Accounts when navigation or disclosures change. Hidden views are inert. The read-only Usage view loads only when visible. The overview refreshes saved data while visible, without executing a provider.

The first native reference journey passed 21 reviewed captures. It checked real navigation, three GUI-created profiles, saved quota/activity, and completion of a background read after panel closure and navigation. Native backup-content passed eleven captures. Production checkpoint recovery passed seven captures; production fixture refusal also passed. These Linux checks used synthetic provider data. Native visual review found repeated Accounts headings and notices; the next refinement removes them. Secondary-page enlarged captures show their initial viewport, not every control below the fold.

The workspace passed 190 Rust checks, strict Clippy, 34 contracts, and 116 renderer checks in Chromium and WebKit. Two parallel native startups timed out before creating sessions. An isolated retry opened the app, then exposed a quoting fault in the test theme helper. The corrected helper and unchanged application passed. Startup contention remains a hypothesis. Failed runs remain retained. See `../evidence/native-reference-workspace/`.


### Native refinement and text reflow

The follow-up removes the repeated Accounts heading and exposes capacity guidance with the creation form. One provenance notice above the overview retains synthetic-source and unverified-identity labels. Missing activity values and their action stay together.

Three scrolled enlarged-text captures exposed a real account-row fault. The automated journey returned success, but native review found a one-character-wide sign-in status. Both renderer browsers reproduced seven lines for the word Sign-in. Mobile rows now use a wrapping flex layout: status and action share space when they fit and use separate lines when they need more room. The same test then passed with one line. The earlier automated result is retained and explicitly fails visual qualification.

The corrected native artifact passed 24 reviewed captures, eleven backup-content captures, and seven production recovery captures. Default production refused all four synthetic providers. The final full check passed 34 contracts and 116 renderer tests. Rust and dependencies remain identical to the baseline that passed 190 workspace tests and strict Clippy. Native source, assets, executable, tools, and fixture hashes remained unchanged during each journey.

These are Linux journeys with clearly labeled synthetic data and test-side font scaling. They do not establish live-provider authentication, OS text scaling, release-platform performance, full accessibility acceptance, or Windows/macOS behavior. Existing inner scrolling keeps exact oversized usage values accessible. See `../evidence/native-reference-refinement/`.


### Native typography refinement

The native visual journey passed 24 reviewed captures across four views, both themes, 360-pixel windows, and test-side 200 percent text. Runtime checks confirm loaded Geist interface text and JetBrains Mono values. Account status and actions retain whole words. Long exact usage values retain their existing local horizontal scroll. This does not qualify OS text scaling, screen readers, Windows, macOS, or live-provider data. See `../evidence/native-typography-login/`.

### One refresh action

The primary account action is Refresh readings. Manage profile discloses individual reads and less frequent actions. Active Cancel controls remain outside that disclosure and immediately precede its trigger. Combined completion reloads both saved panels without moving focus away from its result. Individual read completion retains its existing saved-panel focus. Receipt checks still require matching profile, binding, and run identifiers.

The current pairing retains Geist for interface text and JetBrains Mono for exact values. Normal body text uses 14 pixels with 1.5 line height. Supporting text uses 13 pixels. Page headings use 28 pixels at weight 500. These sizes use relative units. No new font, decorative motion, or component dependency was added.

Native review exposed the original Cancel document-order fault and competing saved-cache focus. Both renderer browsers reproduced these failures before the corrections. The rebuilt native app passes physical reverse Tab to Cancel and keeps combined completion focused. Its visual journey passed 24 reviewed captures with loaded-font assertions, both themes, narrow windows, and enlarged text. The strict disclosure journey adds eight reviewed captures. These remain synthetic Linux checks with test-side text scaling. See `../evidence/native-combined-collection/`.

## Monitoring controls

Start and Pause stay under Manage profile. One workspace status poll updates all controls. A committed reading revision refreshes saved values without moving focus. Pause remains available during a scheduled read or a status-load failure. Restart shows saved readings with disabled Start and sign-in guidance. Eight reviewed native captures cover normal and enlarged text, both themes, keyboard Pause, and retained focus. See `../evidence/native-runtime-monitoring/`.

The October 21 native check measured 16-pixel body/button text, 48-pixel primary controls, 10-pixel control corners, and 16-pixel panel corners. The Accounts view reflowed at 360 pixels without page overflow. Both native themes were inspected. See [the startup and readability evidence](../evidence/native-startup-snapshot/). Overview simplification and full accessibility acceptance remain open.
