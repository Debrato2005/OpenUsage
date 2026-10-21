> Public source copy: raw acceptance evidence remains private. Historical test statements keep their original scope and do not certify this source snapshot.

# Native button focus

October 18, 2026. Plain text labels in the shared coss Button now keep pointer hits on the native button. This corrects reverse Tab after a centered label click in WebKitGTK. The label and accessible name stay intact. Custom renderers and mixed-content children stay unchanged.

The investigation reproduced the old failure on its original executable. Clicking the text trapped reverse Tab at Manage profile. Clicking the padding, or reaching Manage through forward Tab, reached the selected Cancel. A reversible comparison used the same button, label span, original text node, and running provider process. Only the span's pointer-events value changed. The none value passed, while auto failed. Storage hashes and the original DOM returned unchanged. These observations match the internal focus-navigation starting-node mechanism in [WebKit bug 316027](https://bugs.webkit.org/show_bug.cgi?id=316027) and [PR 74300](https://github.com/WebKit/WebKit/pull/74300). They do not establish a shipped engine fix.

The product fix changes one shared component. It wraps a default-render string label in a span with pointer-events disabled. It adds no key remapping, focus assignment, DOM reordering, clipping, dependency, or security exception. The stricter native harness clicks the actual label center and sends physical Shift+Tab.

Five native journeys passed on the rebuilt immutable executables. All three profile operations reached their exact Cancel controls through strict reverse Tab. Actual keyboard cancellation reaped the owned process. An ordinary privacy-button check reached Overview with visible focus. The full quota regression retained all 38 cases. The visual regression captured 48 images across both themes, narrow windows, enlarged text, and password-error states. Encrypted authenticated export and all three production fixture refusals also passed. There are 71 new captures across these journeys.

The visual review opened fourteen current visual captures and bound the other 34 to previously reviewed identical images. The separate profile, quota, and export reviews cover their captures. The frontend check passed 24 contracts and 18 renderer journeys. All Rust and Cargo files match the preceding fresh 117-test qualification, strict Clippy, and formatting. One owned child helper remains intentionally ignored.

The artifact audit (private evidence omitted) binds source, harnesses, executable hashes, receipts, screenshots, and historical diagnostics. All 163 source/harness hashes match before and after execution. Historical failed receipts retain their original executable scope. These are Linux debug builds with labeled synthetic provider data. SQLCipher 4, WAL, credential isolation, and source approval stay unchanged.

Full AT-36 acceptance remains open. Mixed-content controls, complete workflows, screen readers, other platforms, and release performance need separate qualification. Live OAuth, verified provider identity, and accepted quota history remain incomplete. Monthly limits and spend-control state are the next separate implementation slice.
