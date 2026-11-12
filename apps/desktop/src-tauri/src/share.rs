//! Share images made on this device with no network (OU-SHARE-001 to OU-SHARE-006).
//! Rust builds each image as SVG from saved local totals. The renderer draws that SVG as PNG,
//! and Rust checks the PNG before it saves it. Saving is always an explicit action.
use crate::workspace::CommandError;
#[cfg(test)]
use crate::workspace::local_logs::LocalUsageRange;
use crate::{WorkspaceService, arguments, readings_export};
use base64::Engine;
use jiff::{Span, civil::Date};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::Write;
use tauri::{AppHandle, Manager, State};

const REPOSITORY: &str = "github.com/Debrato2005/OpenUsage";
/// The renderer draws PNG at twice the SVG size.
const PNG_SCALE: u32 = 2;
const MAX_PNG_BYTES: usize = 8 * 1024 * 1024;

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum Kind {
    Wrapped,
    Heatmap,
    /// A README badge. It is SVG only, because a README shows SVG (OU-SHARE-006).
    Badge,
}
impl Kind {
    /// The fixed size of a PNG-capable image. A badge sets its own width from its text.
    fn size(self) -> (u32, u32) {
        match self {
            Kind::Wrapped => (1200, 630),
            Kind::Heatmap => (1000, 230),
            Kind::Badge => (0, 20),
        }
    }
    fn prefix(self) -> &'static str {
        match self {
            Kind::Wrapped => "openusage-wrapped-",
            Kind::Heatmap => "openusage-heatmap-",
            Kind::Badge => "openusage-badge-",
        }
    }
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum Format {
    Svg,
    Png,
}

#[derive(Default, Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ShareRange {
    #[default]
    #[serde(rename = "year")]
    Year,
    #[serde(rename = "7d")]
    SevenDays,
    #[serde(rename = "30d")]
    ThirtyDays,
    #[serde(rename = "month")]
    Month,
}

impl ShareRange {
    pub(crate) fn dates(self, today: Date) -> Result<(Date, Date), CommandError> {
        let start = match self {
            Self::Month => today.first_of_month(),
            other => today
                .checked_sub(Span::new().days(match other {
                    Self::Year => 364,
                    Self::SevenDays => 6,
                    Self::ThirtyDays => 29,
                    Self::Month => unreachable!(),
                }))
                .map_err(|_| CommandError::InvalidArguments)?,
        };
        Ok((start, today))
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LoadArgs {
    kind: Kind,
    branding: bool,
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    range: ShareRange,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SaveArgs {
    kind: Kind,
    branding: bool,
    format: Format,
    /// Base64 PNG bytes that the renderer drew from this image. Only with the PNG format.
    png: Option<String>,
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    range: ShareRange,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ShareImage {
    svg: String,
    width: u32,
    height: u32,
    /// A plain description of the image for its alternative text.
    summary: String,
    details: Option<ShareDetails>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ShareDetails {
    range: ShareRange,
    start_date: String,
    end_date: String,
    time_zone: String,
    total_tokens: Option<String>,
    estimated_value_usd_micros: Option<String>,
    estimated_value_display: Option<String>,
    estimate_complete: bool,
    active_days: Option<usize>,
    saved_days: usize,
    missing_days: usize,
    missing_components: Vec<&'static str>,
    logs_gone: bool,
    pricing_basis: &'static str,
    source_scope: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ShareReceipt {
    file_name: String,
    directory: String,
    format: &'static str,
    byte_count: String,
}

/// Exact statistics for the dates `start..=end` from saved local days only.
#[derive(Debug, Default, PartialEq, Eq)]
struct Stats {
    start: String,
    end: String,
    days: Vec<(String, Option<u128>)>,
    total: u128,
    total_available: bool,
    active_days: Option<usize>,
    longest_streak: Option<usize>,
    busiest: Option<(String, u128)>,
    top_model: Option<String>,
    tools: Vec<(String, u128)>,
    estimate: Option<u128>,
    estimate_complete: bool,
    missing_components: Vec<&'static str>,
}

fn stats(view: &crate::local_usage::LocalUsageView) -> Result<Stats, CommandError> {
    if !view.available || !view.dated {
        return Err(CommandError::InvalidArguments);
    }
    let number = |value: &str| {
        value
            .parse::<u128>()
            .map_err(|_| CommandError::StorageUnavailable)
    };
    let mut by_day: BTreeMap<String, u128> = BTreeMap::new();
    for bucket in &view.buckets {
        *by_day.entry(bucket.start_date.clone()).or_default() += number(&bucket.total_tokens)?;
    }
    let start = view
        .start_date
        .parse::<Date>()
        .map_err(|_| CommandError::StorageUnavailable)?;
    let end = view
        .end_date
        .parse::<Date>()
        .map_err(|_| CommandError::StorageUnavailable)?;
    let mut cursor = start;
    let mut days = Vec::new();
    while cursor <= end {
        let key = cursor.to_string();
        days.push((key.clone(), by_day.get(&key).copied()));
        cursor = cursor
            .checked_add(Span::new().days(1))
            .map_err(|_| CommandError::StorageUnavailable)?;
    }
    let mut tools = Vec::new();
    let mut priced = None;
    let mut complete = !view.tools.is_empty();
    let mut missing_components = Vec::new();
    for (name, missing) in [
        (
            "inputTokens",
            view.tools.iter().any(|t| t.input_tokens.is_none()),
        ),
        (
            "outputTokens",
            view.tools.iter().any(|t| t.output_tokens.is_none()),
        ),
        (
            "cacheReadTokens",
            view.tools.iter().any(|t| t.cache_read_tokens.is_none()),
        ),
        (
            "cacheWriteTokens",
            view.tools.iter().any(|t| t.cache_write_tokens.is_none()),
        ),
        (
            "reasoningTokens",
            view.tools.iter().any(|t| t.reasoning_tokens.is_none()),
        ),
    ] {
        if missing {
            missing_components.push(name);
        }
    }
    for tool in &view.tools {
        tools.push((tool.tool.clone(), number(&tool.total_tokens)?));
        if let Some(value) = &tool.estimated_value_usd_micros {
            priced = Some(priced.unwrap_or(0) + number(value)?);
        }
        complete &= tool.estimate_complete;
    }
    tools.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let mut models = BTreeMap::<String, u128>::new();
    let mut models_complete = !view.models.is_empty();
    for model in &view.models {
        if let Some(total) = &model.total_tokens {
            *models.entry(model.model.clone()).or_default() += number(total)?;
        } else {
            models_complete = false;
        }
    }
    let total = tools.iter().map(|(_, n)| n).sum();
    models_complete &= models.values().sum::<u128>() == total;
    let top_model = models_complete
        .then(|| {
            models
                .into_iter()
                .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)))
                .map(|(name, _)| name)
        })
        .flatten();
    Ok(Stats {
        start: view.start_date.clone(),
        end: view.end_date.clone(),
        days,
        total,
        total_available: !view.tools.is_empty(),
        active_days: (!by_day.is_empty()).then_some(view.active_days).flatten(),
        longest_streak: (!by_day.is_empty())
            .then_some(view.longest_streak_days)
            .flatten(),
        busiest: view
            .busiest_day
            .as_ref()
            .map(|d| Ok((d.date.clone(), number(&d.total_tokens)?)))
            .transpose()?,
        top_model,
        tools,
        estimate: priced,
        estimate_complete: complete,
        missing_components,
    })
}

fn details(
    view: &crate::local_usage::LocalUsageView,
    stats: &Stats,
    range: ShareRange,
) -> ShareDetails {
    let saved_days = stats
        .days
        .iter()
        .filter(|(_, total)| total.is_some())
        .count();
    ShareDetails {
        range,
        start_date: stats.start.clone(),
        end_date: stats.end.clone(),
        time_zone: view.time_zone.clone(),
        total_tokens: stats.total_available.then(|| stats.total.to_string()),
        estimated_value_usd_micros: stats.estimate.map(|n| n.to_string()),
        estimated_value_display: stats.estimate.map(dollars),
        estimate_complete: stats.estimate_complete,
        active_days: stats.active_days,
        saved_days,
        missing_days: stats.days.len() - saved_days,
        missing_components: stats.missing_components.clone(),
        logs_gone: view.tools.iter().any(|tool| tool.logs_gone),
        pricing_basis: "Stored scanner estimates at public API prices",
        source_scope: if view.scope == "combined" {
            "Accepted local history on this device"
        } else {
            "Selected accepted local history"
        },
    }
}

fn grouped(value: u128) -> String {
    let digits = value.to_string();
    let mut out = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// A short count cut to tenths with integers, such as 55.0M.
fn compact(value: u128) -> String {
    for (unit, suffix) in [(1_000_000_000, "B"), (1_000_000, "M"), (1_000, "K")] {
        if value >= unit {
            let tenths = value * 10 / unit;
            return format!("{}.{}{suffix}", tenths / 10, tenths % 10);
        }
    }
    value.to_string()
}

fn dollars(micros: u128) -> String {
    let cents = (micros + 5_000) / 10_000;
    format!("${}.{:02}", grouped(cents / 100), cents % 100)
}

/// Log text never becomes markup (OU-SEC-021): every string is escaped.
fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn tool_name(tool: &str) -> &str {
    match tool {
        "claude" => "Claude Code",
        "codex" => "Codex",
        other => other,
    }
}

fn tool_color(tool: &str) -> &'static str {
    match tool {
        "claude" => "#f0701e",
        "codex" => "#1ea56a",
        _ => "#7c6cf2",
    }
}

const FONT: &str = "-apple-system, 'Segoe UI', Helvetica, Arial, sans-serif";
const INK: &str = "#f3f4f6";
const MUTED: &str = "#9aa3af";
const BACKGROUND: &str = "#101216";
const PANEL: &str = "#1a1d23";

fn wrapped(stats: &Stats, branding: bool) -> String {
    let (width, height) = Kind::Wrapped.size();
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\" font-family=\"{FONT}\">\
         <rect width=\"{width}\" height=\"{height}\" rx=\"32\" fill=\"{BACKGROUND}\"/>\
         <text x=\"64\" y=\"92\" font-size=\"28\" fill=\"{MUTED}\">Local AI coding usage, {} to {}</text>",
        escape(&stats.start),
        escape(&stats.end)
    );
    let _ = write!(
        svg,
        "<text x=\"64\" y=\"222\" font-size=\"128\" font-weight=\"700\" fill=\"{INK}\">{}</text>\
         <text x=\"64\" y=\"266\" font-size=\"28\" fill=\"{MUTED}\">{}</text>",
        if stats.total_available {
            compact(stats.total)
        } else {
            "Not available".into()
        },
        if stats.total_available {
            format!("{} tokens", grouped(stats.total))
        } else {
            "Tokens".into()
        }
    );
    let busiest = stats
        .busiest
        .as_ref()
        .map_or("Not available".to_string(), |(date, total)| {
            format!("{date}, {}", compact(*total))
        });
    let top = stats.top_model.as_deref().unwrap_or("Not available");
    let tiles = [
        (
            "Active days",
            stats
                .active_days
                .map_or("Not available".into(), |n| n.to_string()),
        ),
        (
            "Longest streak",
            stats
                .longest_streak
                .map_or("Not available".into(), |n| format!("{n} days")),
        ),
        ("Busiest day", busiest),
        ("Top model", top.to_string()),
    ];
    for (index, (label, value)) in tiles.iter().enumerate() {
        let x = 64 + index as u32 * 272;
        // A long value gets a smaller size, and past 20 characters it ends with an ellipsis.
        let shown: String = if value.chars().count() > 20 {
            value.chars().take(19).chain(['…']).collect()
        } else {
            value.clone()
        };
        let _ = write!(
            svg,
            "<rect x=\"{x}\" y=\"312\" width=\"256\" height=\"112\" rx=\"20\" fill=\"{PANEL}\"/>\
             <text x=\"{}\" y=\"352\" font-size=\"20\" fill=\"{MUTED}\">{label}</text>\
             <text x=\"{}\" y=\"396\" font-size=\"{}\" font-weight=\"600\" fill=\"{INK}\">{}</text>",
            x + 20,
            x + 20,
            if shown.chars().count() > 13 { 19 } else { 30 },
            escape(&shown)
        );
    }
    // One stacked bar of tools. Tools stay separate, so the bar shows shares, not a sum label.
    let mut x = 64u128;
    let span = 1072u128;
    for (tool, total) in &stats.tools {
        let width = (span * total)
            .checked_div(stats.total)
            .map_or(0, |w| w.max(4));
        let _ = write!(
            svg,
            "<rect x=\"{x}\" y=\"456\" width=\"{width}\" height=\"18\" rx=\"9\" fill=\"{}\"/>",
            tool_color(tool)
        );
        x += width;
    }
    for (index, (tool, total)) in stats.tools.iter().take(3).enumerate() {
        let x = 64 + index as u32 * 300;
        let _ = write!(
            svg,
            "<circle cx=\"{}\" cy=\"508\" r=\"7\" fill=\"{}\"/><text x=\"{}\" y=\"515\" font-size=\"22\" fill=\"{INK}\">{} {}</text>",
            x + 7,
            tool_color(tool),
            x + 22,
            escape(tool_name(tool)),
            compact(*total)
        );
    }
    let estimate = match stats.estimate {
        None => "Estimated API value Not available".to_string(),
        Some(micros) => format!(
            "Estimated API value {}{}",
            dollars(micros),
            if stats.estimate_complete {
                ""
            } else {
                " · Incomplete"
            }
        ),
    };
    let _ = write!(
        svg,
        "<text x=\"64\" y=\"562\" font-size=\"22\" fill=\"{MUTED}\">{}</text>",
        escape(&estimate)
    );
    let missing_days = stats
        .days
        .iter()
        .filter(|(_, total)| total.is_none())
        .count();
    let _ = write!(
        svg,
        "<text x=\"64\" y=\"596\" font-size=\"18\" fill=\"{MUTED}\">{missing_days} days without saved usage</text>"
    );
    if branding {
        let _ = write!(
            svg,
            "<text x=\"{}\" y=\"596\" font-size=\"20\" text-anchor=\"end\" fill=\"{MUTED}\">OpenUsage  {REPOSITORY}</text>",
            width - 64
        );
    }
    svg.push_str("</svg>");
    svg
}

/// Days with saved usage get one of four levels. A day with no saved usage is an outline (OU-SHARE-004).
fn heatmap(stats: &Stats, branding: bool) -> String {
    let (width, height) = Kind::Heatmap.size();
    let mut nonzero: Vec<u128> = stats
        .days
        .iter()
        .filter_map(|(_, t)| *t)
        .filter(|t| *t > 0)
        .collect();
    nonzero.sort_unstable();
    let cut = |q: usize| {
        nonzero
            .get(nonzero.len().saturating_sub(1) * q / 4)
            .copied()
            .unwrap_or(0)
    };
    let cuts = [cut(1), cut(2), cut(3)];
    let shades = ["#3d3480", "#5a4bc0", "#7c6cf2", "#b8adff"];
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\" font-family=\"{FONT}\">\
         <rect width=\"{width}\" height=\"{height}\" rx=\"24\" fill=\"{BACKGROUND}\"/>\
         <text x=\"32\" y=\"44\" font-size=\"20\" font-weight=\"600\" fill=\"{INK}\">Tokens per day, {} to {}</text>\
         <text x=\"{}\" y=\"44\" font-size=\"20\" text-anchor=\"end\" fill=\"{MUTED}\">{} tokens</text>",
        escape(&stats.start),
        escape(&stats.end),
        width - 32,
        grouped(stats.total)
    );
    let first = stats
        .days
        .first()
        .and_then(|(date, _)| date.parse::<Date>().ok());
    let offset = first.map_or(0, |d| i64::from(d.weekday().to_monday_zero_offset()));
    for (index, (date, total)) in stats.days.iter().enumerate() {
        let cell = index as i64 + offset;
        let (column, row) = (cell / 7, cell % 7);
        let (x, y) = (32 + column * 17, 64 + row * 17);
        let fill = if total.is_none() {
            "fill=\"none\" stroke=\"#3a3f47\"".to_string()
        } else if *total == Some(0) {
            "fill=\"#252932\"".to_string()
        } else {
            let level = cuts
                .iter()
                .filter(|c| total.is_some_and(|t| t > **c))
                .count();
            format!("fill=\"{}\"", shades[level])
        };
        let _ = write!(
            svg,
            "<rect x=\"{x}\" y=\"{y}\" width=\"13\" height=\"13\" rx=\"3\" {fill}><title>{}</title></rect>",
            escape(&if total.is_none() {
                format!("{date}: no saved usage")
            } else {
                format!("{date}: {} tokens", grouped(total.unwrap_or(0)))
            })
        );
    }
    let _ = write!(
        svg,
        "<rect x=\"32\" y=\"196\" width=\"13\" height=\"13\" rx=\"3\" fill=\"none\" stroke=\"#3a3f47\"/>\
         <text x=\"52\" y=\"207\" font-size=\"14\" fill=\"{MUTED}\">No saved usage</text>\
         <text x=\"200\" y=\"207\" font-size=\"14\" fill=\"{MUTED}\">Less</text>"
    );
    for (index, shade) in shades.iter().enumerate() {
        let _ = write!(
            svg,
            "<rect x=\"{}\" y=\"196\" width=\"13\" height=\"13\" rx=\"3\" fill=\"{shade}\"/>",
            240 + index * 17
        );
    }
    let _ = write!(
        svg,
        "<text x=\"312\" y=\"207\" font-size=\"14\" fill=\"{MUTED}\">More</text>"
    );
    if branding {
        let _ = write!(
            svg,
            "<text x=\"{}\" y=\"207\" font-size=\"14\" text-anchor=\"end\" fill=\"{MUTED}\">OpenUsage  {REPOSITORY}</text>",
            width - 32
        );
    }
    svg.push_str("</svg>");
    svg
}

/// Text widths use 7 pixels a character at 11 pixels, which fits the common badge fonts.
fn badge(stats: &Stats, branding: bool) -> String {
    let left = if branding { "OpenUsage" } else { "AI coding" };
    let right = format!("{} tokens, 365 days", compact(stats.total));
    let width = |text: &str| 12 + 7 * text.chars().count() as u32;
    let (left_width, right_width) = (width(left), width(&right));
    let total = left_width + right_width;
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{total}\" height=\"20\" viewBox=\"0 0 {total} 20\" font-family=\"Verdana, 'DejaVu Sans', sans-serif\" font-size=\"11\">\
         <rect width=\"{total}\" height=\"20\" rx=\"3\" fill=\"#7c6cf2\"/>\
         <rect width=\"{left_width}\" height=\"20\" rx=\"3\" fill=\"#555b66\"/>\
         <rect x=\"{}\" width=\"4\" height=\"20\" fill=\"#555b66\"/>\
         <text x=\"{}\" y=\"14\" fill=\"#ffffff\" text-anchor=\"middle\">{}</text>\
         <text x=\"{}\" y=\"14\" fill=\"#ffffff\" text-anchor=\"middle\">{}</text></svg>",
        left_width - 4,
        left_width / 2,
        escape(left),
        left_width + right_width / 2,
        escape(&right)
    )
}

fn summary(kind: Kind, stats: &Stats) -> String {
    match kind {
        Kind::Badge => format!(
            "{} tokens from local logs in 365 days",
            grouped(stats.total)
        ),
        Kind::Wrapped => format!(
            "{} tokens from local logs, {} to {}, active days: {}",
            if stats.total_available {
                grouped(stats.total)
            } else {
                "Not available".into()
            },
            stats.start,
            stats.end,
            stats
                .active_days
                .map_or("Not available".into(), |n| n.to_string())
        ),
        Kind::Heatmap => format!(
            "Tokens per day from local logs, {} to {}",
            stats.start, stats.end
        ),
    }
}

fn image(
    workspace: &mut crate::workspace::Workspace,
    kind: Kind,
    branding: bool,
    scope: Option<String>,
    range: ShareRange,
) -> Result<(Stats, String, Option<ShareDetails>), CommandError> {
    if kind != Kind::Wrapped && range != ShareRange::Year {
        return Err(CommandError::InvalidArguments);
    }
    let view = crate::local_usage::share_view(workspace, scope, range)?;
    let stats = stats(&view)?;
    if view.tools.is_empty() && range == ShareRange::Year {
        return Err(CommandError::InvalidArguments);
    }
    let svg = match kind {
        Kind::Wrapped => wrapped(&stats, branding),
        Kind::Heatmap => heatmap(&stats, branding),
        Kind::Badge => badge(&stats, branding),
    };
    let details = (kind == Kind::Wrapped).then(|| details(&view, &stats, range));
    Ok((stats, svg, details))
}

fn badge_width(svg: &str) -> Option<u32> {
    let rest = svg.split_once("width=\"")?.1;
    rest.split_once('"')?.0.parse().ok()
}

/// The PNG must be a PNG at twice the SVG size, so a renderer cannot save other bytes.
fn checked_png(kind: Kind, encoded: &str) -> Result<Vec<u8>, CommandError> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| CommandError::InvalidArguments)?;
    let (width, height) = kind.size();
    let expected = [width * PNG_SCALE, height * PNG_SCALE];
    if bytes.len() > MAX_PNG_BYTES
        || bytes.len() < 33
        || bytes[..8] != *b"\x89PNG\r\n\x1a\n"
        || &bytes[12..16] != b"IHDR"
        || [
            u32::from_be_bytes(bytes[16..20].try_into().unwrap_or_default()),
            u32::from_be_bytes(bytes[20..24].try_into().unwrap_or_default()),
        ] != expected
    {
        return Err(CommandError::InvalidArguments);
    }
    Ok(bytes)
}

#[tauri::command]
pub async fn load_share_image(
    request: tauri::ipc::Request<'_>,
    service: State<'_, WorkspaceService>,
) -> Result<ShareImage, CommandError> {
    let args: LoadArgs = arguments(&request)?;
    crate::subscriptions::access(service, move |workspace| {
        let (stats, svg, details) = image(
            workspace,
            args.kind,
            args.branding,
            args.scope.clone(),
            args.range,
        )?;
        let (width, height) = match args.kind {
            // The badge width is the first number in its SVG, set by its text.
            Kind::Badge => (
                badge_width(&svg).ok_or(CommandError::StorageUnavailable)?,
                20,
            ),
            kind => kind.size(),
        };
        Ok(ShareImage {
            svg,
            width,
            height,
            summary: summary(args.kind, &stats),
            details,
        })
    })
    .await
}

/// Saves the image in Downloads/OpenUsage. The renderer never chooses the path or the name.
#[tauri::command]
pub async fn save_share_image(
    request: tauri::ipc::Request<'_>,
    app: AppHandle,
    service: State<'_, WorkspaceService>,
) -> Result<ShareReceipt, CommandError> {
    let args: SaveArgs = arguments(&request)?;
    let png = match (args.format, args.png.as_deref()) {
        (Format::Png, Some(_)) if args.kind == Kind::Badge => {
            return Err(CommandError::InvalidArguments);
        }
        (Format::Png, Some(encoded)) => Some(checked_png(args.kind, encoded)?),
        (Format::Svg, None) => None,
        _ => return Err(CommandError::InvalidArguments),
    };
    let directory = app
        .path()
        .download_dir()
        .map_err(|_| CommandError::ExportUnavailable)?
        .join("OpenUsage");
    crate::subscriptions::access(service, move |workspace| {
        let (_, svg, _) = image(
            workspace,
            args.kind,
            args.branding,
            args.scope.clone(),
            args.range,
        )?;
        let bytes = match png {
            Some(bytes) => bytes,
            None => svg.into_bytes(),
        };
        let extension = if args.format == Format::Png {
            "png"
        } else {
            "svg"
        };
        let (file, ()) = readings_export::publish(
            &directory,
            args.kind.prefix(),
            extension,
            MAX_PNG_BYTES as u64,
            |writer| writer.write_all(&bytes).map_err(readings_export::io_error),
        )?;
        Ok(ShareReceipt {
            file_name: file.file_name,
            directory: file.directory,
            format: extension,
            byte_count: file.byte_count.to_string(),
        })
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    fn legacy_stats(range: &LocalUsageRange, start: Date, end: Date) -> Stats {
        super::stats(&crate::local_usage::summarize(
            range,
            start,
            end,
            crate::local_usage::Period::Day,
            "UTC".into(),
        ))
        .unwrap()
    }

    use openusage_storage::{LocalUsageCoverage, LocalUsageDay, LocalUsageRow};

    fn day(date: &str, tool: &str, model: &str, total: u64, value: Option<u64>) -> LocalUsageDay {
        LocalUsageDay {
            log_source_id: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa".into(),
            counts: LocalUsageRow {
                local_date: date.into(),
                tool: tool.into(),
                model: model.into(),
                input_tokens: total,
                output_tokens: 0,
                cache_write_tokens: 0,
                cache_read_tokens: 0,
                total_tokens: total,
                estimated_value_usd_micros: value,
            },
            scanner_version: "20.0.48".into(),
            coverage: LocalUsageCoverage::Complete,
        }
    }

    fn range(days: Vec<LocalUsageDay>) -> LocalUsageRange {
        LocalUsageRange {
            days,
            differences: Vec::new(),
            sources: Vec::new(),
            sessions: Vec::new(),
            hours: Vec::new(),
            window: None,
        }
    }

    #[test]
    fn statistics_are_exact_and_streaks_count_only_days_with_usage() {
        let stats = legacy_stats(
            &range(vec![
                day(
                    "2026-11-02",
                    "claude",
                    "claude-sonnet-4",
                    4_000_000,
                    Some(100),
                ),
                day("2026-11-03", "claude", "claude-sonnet-4", 1_000, Some(10)),
                day("2026-11-03", "codex", "gpt-5-codex", 9_000_000, None),
                day("2026-11-04", "codex", "gpt-5-codex", 1, None),
                day("2026-11-07", "claude", "claude-opus-4", 2, Some(1)),
            ]),
            "2026-11-01".parse().unwrap(),
            "2026-11-08".parse().unwrap(),
        );
        assert_eq!(stats.days.len(), 8);
        assert_eq!(stats.total, 13_001_003);
        assert_eq!(
            (stats.active_days, stats.longest_streak),
            (Some(4), Some(3))
        );
        assert_eq!(stats.busiest, Some(("2026-11-03".into(), 9_001_000)));
        assert_eq!(stats.top_model.as_deref(), Some("gpt-5-codex"));
        assert_eq!(stats.tools[0], ("codex".into(), 9_000_001));
        assert_eq!(
            (stats.estimate, stats.estimate_complete),
            (Some(111), false)
        );
        let empty = legacy_stats(
            &range(Vec::new()),
            "2026-11-01".parse().unwrap(),
            "2026-11-02".parse().unwrap(),
        );
        assert_eq!(
            (empty.total, empty.busiest, empty.top_model, empty.estimate),
            (0, None, None, None)
        );
    }

    #[test]
    fn images_hold_no_private_fields_escape_text_and_keep_missing_days_distinct() {
        let stats = legacy_stats(
            &range(vec![day("2026-11-03", "claude", "a<b>&'\"", 5, None)]),
            "2026-11-01".parse().unwrap(),
            "2026-11-08".parse().unwrap(),
        );
        let card = wrapped(&stats, true);
        assert!(card.contains("a&lt;b&gt;&amp;&#39;&quot;") && !card.contains("a<b>"));
        assert!(card.contains("Estimated API value Not available"));
        assert!(card.contains(REPOSITORY));
        assert!(!wrapped(&stats, false).contains(REPOSITORY));
        let map = heatmap(&stats, false);
        assert_eq!(map.matches("no saved usage</title>").count(), 7);
        assert_eq!(map.matches(": 5 tokens</title>").count(), 1);
        for svg in [&card, &map] {
            assert!(svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\""));
            assert!(!svg.contains("<script") && !svg.contains("href") && !svg.contains("/home"));
        }
    }

    #[test]
    fn the_badge_fits_its_text_and_can_drop_the_name() {
        let stats = legacy_stats(
            &range(vec![day("2026-11-03", "claude", "m", 1_374_682_792, None)]),
            "2026-11-01".parse().unwrap(),
            "2026-11-08".parse().unwrap(),
        );
        let svg = badge(&stats, true);
        assert!(svg.contains(">OpenUsage</text>") && svg.contains(">1.3B tokens, 365 days</text>"));
        // 12 + 7 * 9 for the name, and 12 + 7 * 21 for the count.
        assert_eq!(badge_width(&svg), Some(75 + 159));
        assert!(!badge(&stats, false).contains("OpenUsage"));
    }

    #[test]
    fn grouped_share_totals_do_not_require_models_and_zero_is_not_missing() {
        let mut view = crate::local_usage::summarize(
            &range(Vec::new()),
            "2026-11-01".parse().unwrap(),
            "2026-11-03".parse().unwrap(),
            crate::local_usage::Period::Day,
            "UTC".into(),
        );
        view.tools.push(crate::local_usage::ToolTotals {
            tool: "kilo".into(),
            input_tokens: None,
            output_tokens: None,
            cache_write_tokens: None,
            cache_read_tokens: None,
            reasoning_tokens: None,
            total_tokens: "173".into(),
            estimated_value_usd_micros: None,
            estimate_complete: false,
            logs_gone: false,
        });
        view.models.push(crate::local_usage::ModelTotals {
            tool: "kilo".into(),
            model: "m".into(),
            total_tokens: None,
            estimated_value_usd_micros: None,
        });
        for (date, total) in [("2026-11-02", "0"), ("2026-11-03", "173")] {
            view.buckets.push(crate::local_usage::Bucket {
                start_date: date.into(),
                tool: "kilo".into(),
                total_tokens: total.into(),
            });
        }
        view.active_days = Some(1);
        view.longest_streak_days = Some(1);
        let saved = super::stats(&view).unwrap();
        assert_eq!(saved.total, 173);
        assert_eq!(saved.top_model, None);
        assert_eq!(saved.estimate, None);
        assert_eq!(
            saved.days,
            vec![
                ("2026-11-01".into(), None),
                ("2026-11-02".into(), Some(0)),
                ("2026-11-03".into(), Some(173))
            ]
        );
        let svg = heatmap(&saved, false);
        assert!(svg.contains("2026-11-01: no saved usage"));
        assert!(svg.contains("2026-11-02: 0 tokens"));
        assert!(svg.contains("2026-11-03: 173 tokens"));
        view.available = false;
        assert_eq!(super::stats(&view), Err(CommandError::InvalidArguments));
        view.available = true;
        view.dated = false;
        assert_eq!(super::stats(&view), Err(CommandError::InvalidArguments));
    }

    #[test]
    fn a_png_must_be_a_png_at_twice_the_image_size() {
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        png.extend_from_slice(&2400u32.to_be_bytes());
        png.extend_from_slice(&1260u32.to_be_bytes());
        png.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
        let encoded = base64::engine::general_purpose::STANDARD.encode(&png);
        assert_eq!(checked_png(Kind::Wrapped, &encoded).unwrap(), png);
        assert_eq!(
            checked_png(Kind::Heatmap, &encoded),
            Err(CommandError::InvalidArguments)
        );
        assert_eq!(
            checked_png(Kind::Wrapped, "not base64!"),
            Err(CommandError::InvalidArguments)
        );
        let svg = base64::engine::general_purpose::STANDARD
            .encode(b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>");
        assert_eq!(
            checked_png(Kind::Wrapped, &svg),
            Err(CommandError::InvalidArguments)
        );
    }
}
