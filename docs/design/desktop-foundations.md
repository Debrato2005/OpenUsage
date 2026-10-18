# Desktop design foundations

Updated: October 18, 2026. These foundations cover the native workspace, account overview, and backup controls. Provider charts and populated account views need further design and native verification as those features become available.

## Structure and interaction

The overview places storage state first, followed by a compact summary and accounts. Backups use a disclosure below accounts. Recovery messages retain the original database and key. Missing provider readings show “Not available”. Only the unlocked database supplies the profile count.

Privacy stays accessible in narrow windows. Native controls retain keyboard behavior, visible focus, labels, and password-manager support. Errors appear near their controls. A completed backup closes its form, clears passwords, and returns focus to the disclosure button. Its receipt stays visible. No animation or UI dependency was added.

Restore preview compares the current checkpoint with incoming backup counts. The first native capture showed broken column headings at 200% text size. An em-based container query now presents each record category with two labeled values when space is limited. Explicit table roles retain the comparison structure. Five native captures cover desktop, both narrow themes, errors, and enlarged text. Native DOM and keyboard checks passed; screen-reader acceptance remains open. See `../evidence/native-restore-preview/`.

The layout uses one page scroll area, a 208-pixel sidebar, and a 1,280-pixel content limit. At smaller widths, navigation becomes a compact header and summary cells stack. Content wraps instead of concealing overflow. The controls have a minimum height of 44 CSS pixels.

## Typography and tokens

Geist is the single UI family. The unmodified variable WOFF2 is bundled locally with the complete SIL Open Font License. The app makes no font request to a remote server. A preload and `font-display: swap` keep text available during font loading.

| Role | Size | Weight | Line height |
|---|---|---|---|
| Metadata | 12 px | 400 | 1.5 |
| Secondary labels | 13 px | 400–500 | 1.5 |
| Body and controls | 14 px | 400–500 | 1.5 |
| Section headings | 16 px | 600 | 1.3 |
| Summary values | 20 px | 500 | 1.3 |
| Page title | 22 px | 600 | 1.3 |

Sizes use `rem` units. The 14-pixel body serves the desktop information density. Enlarged text and narrow windows need native checks for clipping and control access. Numeric roles use tabular numerals. Future quota, token, time, and money columns must use that same numeric role.

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

Source: [Vercel Geist](https://github.com/vercel/geist-font), revision `e1f671c727f6de7e97c1e1d26dbb44f349c19459`. Original path: `fonts/Geist/webfonts/Geist[wght].woff2`.

Bundled path: `apps/desktop/public/fonts/Geist-Variable.woff2`. SHA-256: `2ffebe993e969069a9789d15164b7715d42491b5835516c5e3b935d5f81b05f1`. The adjacent `OFL.txt` contains the full license and copyright notice.


## Managed sign-in

Sign-in starts with the selected profile name and one consent action. Showing consent contacts no provider. Advanced executable size and checksum remain accessible under a disclosure. The prompt keeps the plain address, selectable code, and Cancel action together. Results receive focus and preserve uncertainty about identity. Credentials already saved by Codex can remain after cancellation.

Native review found an internally clipped URL at enlarged text size. A wrapped read-only field now shows the complete address without scrolling inside the field. Keyboard selection repeats on the next frame only if that field still has focus. Seven actual native captures cover both themes, narrow windows, 200% test-side text, and visible keyboard focus. The harness activates only the identified owned native window before keyboard checks. See `../evidence/native-device-login/`. These checks do not establish screen-reader or full WCAG acceptance.


## Executable selection

Find Codex replaces manual path entry as the first setup action. A single result opens source review. Multiple results use native radio controls and require Review. No results open manual entry with path focus. A later successful search closes that fallback. The source review and initially unchecked approval remain mandatory.

Long paths wrap in the choice list. Size and checksum remain under Executable details. Seven refined native captures cover the empty state, both themes, narrow review, keyboard selection, source refusal, and 200% text. The large-text list uses the page scroll area. See `../evidence/native-provider-discovery/`. The UI explains metadata checks without exposing provider or authentication internals.
