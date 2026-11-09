//! Bundled tool definitions shared with the desktop renderer. No runtime file is read.

use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::OnceLock;

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MarkerKind {
    Directory,
    File,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Root {
    pub path: String,
    pub marker: String,
    pub kind: MarkerKind,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SessionShape {
    Claude,
    Codex,
    Agent,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ProjectPolicy {
    Unavailable,
    Reported,
    Pi,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PeriodPolicy {
    Day,
    Unplaced,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RootMode {
    CommaList,
    Single,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct AgentReportPolicy {
    pub metadata: bool,
    pub activity_times: bool,
    pub project: ProjectPolicy,
    pub model_prefix: Option<String>,
    pub null_empty_totals: bool,
    pub period: PeriodPolicy,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Tool {
    #[serde(skip)]
    pub id: String,
    pub name: String,
    pub order: u8,
    pub tint: u8,
    pub enabled: bool,
    pub folder_variable: Option<String>,
    pub root_mode: RootMode,
    pub roots: Vec<Root>,
    pub session_shape: Option<SessionShape>,
    pub hours: bool,
    pub agent_report: Option<AgentReportPolicy>,
}

pub fn tools() -> &'static [Tool] {
    static TOOLS: OnceLock<Vec<Tool>> = OnceLock::new();
    TOOLS.get_or_init(|| {
        let definitions: BTreeMap<String, Tool> =
            serde_json::from_str(include_str!("../sources/tools.json"))
                .expect("The bundled tool catalog must be valid.");
        let mut tools: Vec<_> = definitions
            .into_iter()
            .map(|(id, mut tool)| {
                tool.id = id;
                tool
            })
            .collect();
        tools.sort_by_key(|tool| tool.order);
        tools
    })
}

pub fn enabled_tools() -> impl Iterator<Item = &'static Tool> {
    tools().iter().filter(|tool| tool.enabled)
}

pub fn tool(id: &str) -> Option<&'static Tool> {
    tools().iter().find(|tool| tool.id == id)
}

pub fn hour_tools() -> impl Iterator<Item = &'static Tool> {
    enabled_tools().filter(|tool| tool.hours)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::path::{Component, Path};

    #[test]
    fn catalog_has_unique_order_and_complete_enabled_definitions() {
        let mut orders = BTreeSet::new();
        let mut variables = BTreeSet::new();
        let relative = |text: &str| {
            !text.is_empty()
                && Path::new(text)
                    .components()
                    .all(|component| matches!(component, Component::Normal(_)))
        };
        for tool in tools() {
            assert!(!tool.id.is_empty());
            assert!(tool.id.bytes().all(|b| b.is_ascii_lowercase()));
            assert!(orders.insert(tool.order));
            assert!(!tool.name.is_empty() && !tool.name.chars().any(char::is_control));
            assert!(tool.tint < 6);
            assert_eq!(
                tool.agent_report.is_some(),
                tool.session_shape == Some(SessionShape::Agent)
            );
            if let Some(policy) = &tool.agent_report {
                assert!(
                    policy.metadata
                        || (!policy.activity_times && policy.project == ProjectPolicy::Unavailable)
                );
                assert!(!policy.activity_times || policy.period == PeriodPolicy::Day);
            }
            if tool.enabled {
                let variable = tool.folder_variable.as_ref().unwrap();
                assert!(!variable.is_empty() && variables.insert(variable));
                assert!(
                    variable
                        .bytes()
                        .all(|b| b.is_ascii_uppercase() || b == b'_')
                );
                assert!(!tool.roots.is_empty());
                assert!(tool.session_shape.is_some());
            }
            for root in &tool.roots {
                assert!(relative(&root.path) && relative(&root.marker));
            }
        }
        assert_eq!(
            orders.into_iter().collect::<Vec<_>>(),
            (0..18).collect::<Vec<_>>()
        );
        assert_eq!(
            enabled_tools()
                .map(|tool| tool.id.as_str())
                .collect::<Vec<_>>(),
            ["claude", "codex"]
        );
        assert_eq!(
            hour_tools()
                .map(|tool| tool.id.as_str())
                .collect::<Vec<_>>(),
            ["claude"]
        );
    }
}
