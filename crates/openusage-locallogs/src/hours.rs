//! Strict parser for `ccusage blocks --json -n 1` from the pinned version (OU-LOG-017, SC-021).
//! With one-hour blocks, each block covers one whole UTC hour. Only Claude Code reports blocks.

use crate::report::{ReportError, counts, valid_model};
use serde::Deserialize;

const MAX_HOURS: usize = 200_000;

/// One UTC hour of one tool. Token counts are exact integers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HourRow {
    pub tool: &'static str,
    /// The start of the hour, such as `2026-11-04T09:00:00.000Z`.
    pub start_utc: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_write_tokens: u64,
    pub cache_read_tokens: u64,
    pub total_tokens: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Report {
    blocks: Vec<Block>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Block {
    actual_end_time: Option<String>,
    /// Set only for the active block. The app keeps no projection.
    burn_rate: Option<serde_json::Value>,
    #[serde(rename = "costUSD")]
    cost_usd: f64,
    end_time: String,
    entries: u64,
    id: String,
    is_active: bool,
    is_gap: bool,
    models: Vec<String>,
    projection: Option<serde_json::Value>,
    start_time: String,
    token_counts: BlockCounts,
    total_tokens: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct BlockCounts {
    cache_creation_input_tokens: u64,
    cache_read_input_tokens: u64,
    input_tokens: u64,
    output_tokens: u64,
}

/// A whole UTC hour in the form that ccusage writes: `2026-11-04T09:00:00.000Z`.
fn whole_hour(value: &str) -> bool {
    let b = value.as_bytes();
    b.len() == 24
        && crate::report::valid_date(&value[..10])
        && b[10] == b'T'
        && b[11..13].iter().all(u8::is_ascii_digit)
        && &value[11..13] < "24"
        && &value[13..] == ":00:00.000Z"
}

fn shape(id: &str) -> ReportError {
    ReportError::Shape(format!("block {id:?}"))
}

/// Parses the one-hour blocks of Claude Code. Gaps carry no usage and give no row.
pub fn parse_hours(bytes: &[u8]) -> Result<Vec<HourRow>, ReportError> {
    let report: Report =
        serde_json::from_slice(bytes).map_err(|error| ReportError::Shape(error.to_string()))?;
    if report.blocks.len() > MAX_HOURS {
        return Err(ReportError::Shape("too many blocks".into()));
    }
    let mut rows: Vec<HourRow> = Vec::new();
    for block in report.blocks {
        let sum = counts(
            block.token_counts.input_tokens,
            block.token_counts.output_tokens,
            block.token_counts.cache_creation_input_tokens,
            block.token_counts.cache_read_input_tokens,
        )?;
        if block.is_gap {
            if sum.total()? != 0
                || block.total_tokens != 0
                || block.entries != 0
                || !block.models.is_empty()
                || block.is_active
            {
                return Err(shape(&block.id));
            }
            continue;
        }
        let ordered = rows
            .last()
            .is_none_or(|last| last.start_utc < block.start_time);
        if block.id != block.start_time
            || !whole_hour(&block.start_time)
            || !whole_hour(&block.end_time)
            || block.end_time <= block.start_time
            || !ordered
            || block.entries == 0
            || block.models.is_empty()
            || !block.models.iter().all(|model| valid_model(model))
            || block
                .actual_end_time
                .as_deref()
                .is_some_and(|end| end < block.start_time.as_str())
            || !block.cost_usd.is_finite()
            || (!block.is_active && (block.burn_rate.is_some() || block.projection.is_some()))
        {
            return Err(shape(&block.id));
        }
        if block.total_tokens != sum.total()? {
            return Err(ReportError::Inconsistent(format!("block {}", block.id)));
        }
        rows.push(HourRow {
            tool: "claude",
            start_utc: block.start_time,
            input_tokens: sum.input,
            output_tokens: sum.output,
            cache_write_tokens: sum.write,
            cache_read_tokens: sum.read,
            total_tokens: sum.total()?,
        });
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BLOCKS: &[u8] = include_bytes!("../tests/fixtures/ccusage-20.0.48-blocks-claude.json");

    fn edited(edit: impl Fn(&mut serde_json::Value)) -> Vec<u8> {
        let mut value: serde_json::Value = serde_json::from_slice(BLOCKS).unwrap();
        edit(&mut value);
        serde_json::to_vec(&value).unwrap()
    }

    #[test]
    fn one_hour_blocks_become_exact_hours_and_gaps_give_none() {
        let rows = parse_hours(BLOCKS).unwrap();
        assert_eq!(
            rows.iter()
                .map(|r| (r.start_utc.as_str(), r.total_tokens))
                .collect::<Vec<_>>(),
            [
                ("2026-11-04T09:00:00.000Z", 12_500),
                ("2026-11-05T11:00:00.000Z", 2_300),
                ("2026-11-05T13:00:00.000Z", 150)
            ]
        );
        assert_eq!(
            (
                rows[0].input_tokens,
                rows[0].output_tokens,
                rows[0].cache_write_tokens,
                rows[0].cache_read_tokens
            ),
            (3_000, 1_300, 200, 8_000)
        );
        assert_eq!(parse_hours(br#"{"blocks":[]}"#).unwrap(), []);
    }

    #[test]
    fn a_block_that_is_not_a_whole_hour_or_does_not_add_up_rejects_the_scan() {
        let cases: [fn(&mut serde_json::Value); 6] = [
            |v| v["blocks"][0]["startTime"] = "2026-11-04T09:30:00.000Z".into(),
            |v| v["blocks"][0]["totalTokens"] = 12_501.into(),
            |v| v["blocks"][0]["extra"] = 1.into(),
            |v| v["blocks"][1]["totalTokens"] = 1.into(),
            |v| {
                let start = v["blocks"][0]["startTime"].clone();
                v["blocks"][4]["startTime"] = start.clone();
                v["blocks"][4]["id"] = start;
            },
            |v| v["blocks"][0]["projection"] = serde_json::json!({}),
        ];
        for (index, edit) in cases.iter().enumerate() {
            assert!(parse_hours(&edited(edit)).is_err(), "case {index}");
        }
    }
}
