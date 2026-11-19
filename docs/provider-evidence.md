# Provider evidence

Updated: November 19, 2026. Formal AT-60 acceptance remains open.
No completed real-account comparison summary appears below.
[SRS Section 5B](SRS.md#5b-audit) defines acceptance and publication rules.
[Implementation status](implementation-status.md) owns the current state. [Traceability](acceptance-traceability.md) owns exact executed evidence.

## Required source acceptance

| Stage 1 source | Formal evidence state | Current limited evidence |
|---|---|---|
| Codex app-server | AT-60 not run | Synthetic protocol and native tests. Two authorized real accounts signed in and saved readings on an earlier Linux build |
| Claude Code status line | Adapter and AT-60 open | No implemented quota adapter. S-13 and AT-84 remain open |
| Codex local logs through `ccusage` | Formal real-log comparison open | Pinned scanner with private synthetic logs |
| Claude Code local logs through `ccusage` | Formal real-log comparison open | Pinned scanner with private synthetic logs |

The [source matrix](provider-capabilities.md) describes implemented capabilities.
[SRS Section 5A.4](SRS.md#5a4-stage-1-sources-normative) requires real-account acceptance for all four sources.
An authorized sign-in or saved reading does not establish accuracy against the provider display.
Synthetic fixtures, producer captures, and isolated scanner candidates remain synthetic even when they execute a real pinned binary.

## Redacted summary format

Use this format after an authorized real-account test. A template is not evidence.
Each source and used field needs its own scope. Compare at least five readings at two or more usage levels under OU-AUD-006.
Record units and the largest observed difference. Do not infer a missing provider value or compare incompatible windows.

```markdown
### <source and metric> — <test date>

- Result: <passed, failed, or incomplete>
- App commit and dirty-source state: <commit and state>
- Adapter revision and source level: <revision and level>
- Tool/provider version and executable pin: <version and tracked pin>
- Format/projection version: <version or not applicable>
- Operating system, runtime, and architecture: <tested environment>
- Metric, window, units, and comparison display: <scope>
- Declared tolerance and basis: <values and reference>
- Samples and usage levels: <at least five, at least two levels>
- Largest absolute difference: <value and unit for each tested field>
- Private evidence reference: <opaque receipt reference, no path>
- Limits and omitted fields: <explicit limits>
- Traceability record: <tracked link>

| Sample | Usage level | Adapter value | Provider display | Absolute difference |
|---|---|---|---|---|
| 1 | <level> | <value/unit> | <value/unit> | <value/unit> |
| 2 | <level> | <value/unit> | <value/unit> | <value/unit> |
| 3 | <level> | <value/unit> | <value/unit> | <value/unit> |
| 4 | <level> | <value/unit> | <value/unit> | <value/unit> |
| 5 | <level> | <value/unit> | <value/unit> | <value/unit> |
```

For additional samples, extend the table and calculate the maximum across the complete set.
For local logs, name the selected tool, retained report fields, and provider comparison scope.
If the provider display cannot support that comparison, record the gap and an incomplete result.

## Privacy and validity

Raw captures stay private under OU-AUD-007. Public summaries contain no account identifiers, emails, authorization material, raw responses, prompts, code, or full paths.
An opaque receipt reference locates private evidence without exposing its contents.
Anyone can submit a redacted summary. A summary does not authorize access to another account or publication of private captures.

A used-field or parser change voids its prior field acceptance under OU-AUD-008.
Keep the original summary as a dated record. Record the replacement test and its exact version scope.
A hash does not authenticate a provider or prove an identity.
[Stored reading evidence](data-contracts.md#reading-evidence) and this formal comparison record serve separate purposes.
Historical gaps under SC-031 remain open.

Unofficial adapters remain unimplemented. Future Matched status also requires the independent-source, time, identity, and tolerance checks in OU-AUD-004.
A live provider summary never qualifies another operating system, package installation, hosted leaderboard, or Stage 3 sync.
The [compatibility matrix](compatibility-matrix.md) owns those boundaries.
