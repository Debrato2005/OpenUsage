//! Exact local measurements. A missing measurement has no numeric substitute.
use openusage_storage::LocalTokenDetails;
use serde::Serialize;

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Metric {
    pub value: Option<String>,
    pub state: &'static str,
    pub status: Option<&'static str>,
    pub source: Option<String>,
    pub observed_at_utc: Option<String>,
}

impl Metric {
    pub fn new(value: Option<u128>, source: Option<&str>, observed: Option<&str>) -> Self {
        Self {
            value: value.map(|v| v.to_string()),
            state: if value.is_some() {
                "current"
            } else {
                "not_available"
            },
            status: None,
            source: source.map(str::to_owned),
            observed_at_utc: observed.map(str::to_owned),
        }
    }
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Metrics {
    pub total_tokens: Metric,
    pub input_tokens: Metric,
    pub output_tokens: Metric,
    pub cache_write_tokens: Metric,
    pub cache_read_tokens: Metric,
    pub reasoning_tokens: Metric,
    pub estimated_value_usd_micros: Metric,
}

impl Metrics {
    pub fn new(
        total: Option<u128>,
        details: &LocalTokenDetails,
        estimate: Option<u128>,
        source: Option<&str>,
        observed: Option<&str>,
    ) -> Self {
        let metric = |value| Metric::new(value, source, observed);
        Self {
            total_tokens: metric(total),
            input_tokens: metric(details.input_tokens),
            output_tokens: metric(details.output_tokens),
            cache_write_tokens: metric(details.cache_write_tokens),
            cache_read_tokens: metric(details.cache_read_tokens),
            reasoning_tokens: metric(details.reasoning_tokens),
            estimated_value_usd_micros: metric(estimate),
        }
    }
}
