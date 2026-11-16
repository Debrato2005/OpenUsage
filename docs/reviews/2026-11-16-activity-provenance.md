# Activity provenance review

Recorded November 16, 2026. Scope: S-04c0, OU-STATE-001/007, and OU-DES-007/009/010.

Lifetime and Peak day previously displayed quota provenance when both readings existed.
Streak and Longest task omitted their activity provenance.
All four existing detail panels now use the selected saved activity observation.
The rows show its source, provider version, observation time, freshness, scope, coverage, and identity.
Missing activity shows no source, time, or identity from quota.
Quota and reset-credit panels keep quota provenance.

The correction adds one activity helper in the existing details module.
It adds no command, storage field, evidence endpoint, or summary text.
Exact counts, measured zero, and missing values retain their existing meanings.
Full S-04 and the historical conflict under SC-031 remain open.

The public renderer regression first failed with deliberately different activity and quota metadata.
The missing-activity regression also failed before the correction.
Both corrected regressions pass in Chromium and WebKit.
An initial test selector matched no case and supplies no RED evidence.
An older keyboard expectation needed the single Coverage row after duplicate activity coverage disappeared.
The focused keyboard and layout checks pass at normal and enlarged text sizes.
All 58 Overview cases pass. The renderer build and 74 IPC checks pass.
The final execution results follow.

The native harness now opens all four panels and compares actual saved activity metadata.
It checks saved activity and quota, provider requests, the protected key, keyboard focus, both themes, and narrow enlarged text.
Final execution and image review follow.

The full renderer suite passes all 542 cases in Chromium and WebKit.
Independent Spec review reports no scoped findings.
Standards review finds no documented violation. Separate activity and quota helpers retain their different scope and missing-value rules.
These are source reviews, not execution evidence.

The native journey passes on the unchanged native-test binary `270c326aa6769707a97fe356ef4ec3a7643a5dfe0b7e5935cd4cfe45577a8acb`.
All four activity panels match actual saved source, time, version, freshness, scope, coverage, and identity.
Activity, quota, provider requests, and the protected key remain unchanged. Keyboard focus returns to each trigger.

Image review rejected the first enlarged-text captures despite passing data assertions.
A controlled probe reproduced stale computed fonts after the harness scaled hidden page content.
A whole-page restoration check then detected 454 stale off-panel values.
The exact WebKit mechanism and DOM replacement remain unproven.
Product CSS is unchanged.

The corrected harness scales only the dialog after it proves that the dialog fills the entire 360 by 850 capture.
It checks every rendered font at the requested scale and after restoration.
All six captures restore 16-pixel headings. Enlarged headings measure 32 pixels, and rows change from 13 to 26 and back to 13.
The final light and dark images pass root inspection. They retain the exact value and all metadata at 200% text.
This is synthetic Linux text-layout evidence, not OS scaling or real-provider qualification.

The frontend, contracts, full renderer, and native journey are separate bounded checks.
Rust source is unchanged. The parent commit's 613 workspace tests and two ignores retain their own scope.
No additional full Rust run is claimed for this UI correction.
