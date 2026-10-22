# Proposed amendment: user-curated profile activity

Status: approved by the owner on October 22, 2026. This narrow exception permits the implementation below. The normative SRS file remains unchanged.

The app currently blocks every combined result. OU-ID-009 permits provisional presentation, but OU-HEAT-009 and section 8.2 require distinct identities and compatible scopes. The current official managed interface supplies neither guarantee. Email and profile labels cannot establish them.

Approved exception: add a separate, explicit Show selected-profile sum action. It calculates from saved official daily buckets for the selected profiles and date range. Label it Selected-profile activity, with a visible warning: Account identities and coverage are unverified. Duplicate accounts or overlapping activity may be counted more than once. Keep verified combined totals unavailable until their existing requirements pass.

Rust must sum exact integer strings with checked arithmetic. Never use floating-point token arithmetic. Sum only saved values from the same approved provider method and matching provider dates. Do not claim that unknown scope is compatible. Preserve each profile's source, observation time and binding. Exclude disconnected, archived or missing-reading profiles with a reason. Deduplicate verified identities when available. Do not infer identity from email.

Missing days remain unknown. Show observed subtotals and incomplete coverage. Compute the donut's shares only over those same observed subtotals and label the denominator. Zero-total shares remain unavailable. Do not combine lifetime totals, quotas, costs or unsupported measurements.

Acceptance: exact large integers, duplicate selection refusal, known-identity deduplication, profile isolation, date alignment, missing versus zero, source labels, coverage, keyboard access and native interactions. No provider calls, credential reads or storage migration are needed. The existing sign-in flow, encrypted storage and backup controls remain unchanged.

Affected requirements: OU-ID-004/009/010, OU-HEAT-009/010/021 and section 8.2. This exception authorizes an unverified profile sum only. It does not qualify distinct-account aggregation.

Official source: [Codex app-server account and usage operations](https://learn.chatgpt.com/docs/app-server).
