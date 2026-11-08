//! Share images made on this device with no network (OU-SHARE-001 to OU-SHARE-006).
//! Rust builds each image as SVG from saved local totals. The renderer draws that SVG as PNG,
//! and Rust checks the PNG before it saves it. Saving is always an explicit action.
use crate::workspace::{CommandError, local_logs::LocalUsageRange};
use crate::{WorkspaceService, arguments, readings_export};
use base64::Engine;
use jiff::{Span, civil::Date, tz::TimeZone};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::Write;
use tauri::{AppHandle, Manager, State};

const REPOSITORY: &str = "github.com/Debrato2005/OpenUsage";
const DAYS: u16 = 365;
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LoadArgs {
    kind: Kind,
    branding: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SaveArgs {
    kind: Kind,
    branding: bool,
    format: Format,
    /// Base64 PNG bytes that the renderer drew from this image. Only with the PNG format.
    png: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ShareImage {
    svg: String,
    width: u32,
    height: u32,
    /// A plain description of the image for its alternative text.
    summary: String,
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
    days: Vec<(String, u128)>,
    total: u128,
    active_days: usize,
    longest_streak: usize,
    busiest: Option<(String, u128)>,
    top_model: Option<String>,
    tools: Vec<(String, u128)>,
    estimate: Option<u128>,
    estimate_complete: bool,
}

fn stats(range: &LocalUsageRange, start: Date, end: Date) -> Stats {
    let mut by_day: BTreeMap<String, u128> = BTreeMap::new();
    let mut by_tool: BTreeMap<String, u128> = BTreeMap::new();
    let mut by_model: BTreeMap<String, u128> = BTreeMap::new();
    let (mut priced, mut unpriced) = (0u128, 0usize);
    for day in &range.days {
        let row = &day.counts;
        let total = u128::from(row.total_tokens);
        *by_day.entry(row.local_date.clone()).or_default() += total;
        *by_tool.entry(row.tool.clone()).or_default() += total;
        *by_model.entry(row.model.clone()).or_default() += total;
        match row.estimated_value_usd_micros {
            Some(value) => priced += u128::from(value),
            None => unpriced += 1,
        }
    }
    let mut days = Vec::new();
    let (mut streak, mut longest) = (0usize, 0usize);
    let mut cursor = start;
    while cursor <= end {
        let key = cursor.to_string();
        let total = by_day.get(&key).copied().unwrap_or(0);
        streak = if total > 0 { streak + 1 } else { 0 };
        longest = longest.max(streak);
        days.push((key, total));
        let Ok(next) = cursor.checked_add(Span::new().days(1)) else {
            break;
        };
        cursor = next;
    }
    let busiest = days
        .iter()
        .filter(|(_, total)| *total > 0)
        .fold(None::<&(String, u128)>, |best, day| match best {
            Some(best) if best.1 >= day.1 => Some(best),
            _ => Some(day),
        })
        .cloned();
    let top_model = by_model
        .iter()
        .fold(None::<(&String, u128)>, |best, (model, total)| match best {
            Some(best) if best.1 >= *total => Some(best),
            _ => Some((model, *total)),
        })
        .map(|(model, _)| model.clone());
    let mut tools: Vec<_> = by_tool
        .into_iter()
        .filter(|(_, total)| *total > 0)
        .collect();
    tools.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    Stats {
        start: start.to_string(),
        end: end.to_string(),
        total: days.iter().map(|(_, total)| total).sum(),
        active_days: days.iter().filter(|(_, total)| *total > 0).count(),
        longest_streak: longest,
        days,
        busiest,
        top_model,
        tools,
        estimate: (unpriced == 0 || priced > 0).then_some(priced),
        estimate_complete: unpriced == 0,
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
         <text x=\"64\" y=\"266\" font-size=\"28\" fill=\"{MUTED}\">{} tokens</text>",
        compact(stats.total),
        grouped(stats.total)
    );
    let busiest = stats
        .busiest
        .as_ref()
        .map_or("Not available".to_string(), |(date, total)| {
            format!("{date}, {}", compact(*total))
        });
    let top = stats.top_model.as_deref().unwrap_or("Not available");
    let tiles = [
        ("Active days", stats.active_days.to_string()),
        ("Longest streak", format!("{} days", stats.longest_streak)),
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
        None => "Estimated value Not available".to_string(),
        Some(micros) => format!(
            "Estimated {} at public API prices{}",
            dollars(micros),
            if stats.estimate_complete {
                ""
            } else {
                ", some models have no price"
            }
        ),
    };
    let _ = write!(
        svg,
        "<text x=\"64\" y=\"562\" font-size=\"22\" fill=\"{MUTED}\">{}</text>",
        escape(&estimate)
    );
    if branding {
        let _ = write!(
            svg,
            "<text x=\"{}\" y=\"592\" font-size=\"20\" text-anchor=\"end\" fill=\"{MUTED}\">OpenUsage  {REPOSITORY}</text>",
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
        .map(|(_, t)| *t)
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
        let fill = if *total == 0 {
            "fill=\"none\" stroke=\"#3a3f47\"".to_string()
        } else {
            let level = cuts.iter().filter(|c| *total > **c).count();
            format!("fill=\"{}\"", shades[level])
        };
        let _ = write!(
            svg,
            "<rect x=\"{x}\" y=\"{y}\" width=\"13\" height=\"13\" rx=\"3\" {fill}><title>{}</title></rect>",
            escape(&if *total == 0 {
                format!("{date}: no saved usage")
            } else {
                format!("{date}: {} tokens", grouped(*total))
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
            "{} tokens from local logs, {} to {}, on {} active days",
            grouped(stats.total),
            stats.start,
            stats.end,
            stats.active_days
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
) -> Result<(Stats, String), CommandError> {
    let zone = workspace.local_usage_zone()?;
    let today = TimeZone::get(&zone)
        .map(|tz| jiff::Timestamp::now().to_zoned(tz).date())
        .map_err(|_| CommandError::StorageUnavailable)?;
    let start = today
        .checked_sub(Span::new().days(i64::from(DAYS) - 1))
        .map_err(|_| CommandError::StorageUnavailable)?;
    // No session window: share images never read a session or a project name (OU-LOG-004).
    let range = workspace.local_usage_range((&start.to_string(), &today.to_string()), None)?;
    let stats = stats(&range, start, today);
    let svg = match kind {
        Kind::Wrapped => wrapped(&stats, branding),
        Kind::Heatmap => heatmap(&stats, branding),
        Kind::Badge => badge(&stats, branding),
    };
    Ok((stats, svg))
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
        let (stats, svg) = image(workspace, args.kind, args.branding)?;
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
        let bytes = match png {
            Some(bytes) => bytes,
            None => image(workspace, args.kind, args.branding)?.1.into_bytes(),
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
        let stats = stats(
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
        assert_eq!((stats.active_days, stats.longest_streak), (4, 3));
        assert_eq!(stats.busiest, Some(("2026-11-03".into(), 9_001_000)));
        assert_eq!(stats.top_model.as_deref(), Some("gpt-5-codex"));
        assert_eq!(stats.tools[0], ("codex".into(), 9_000_001));
        assert_eq!(
            (stats.estimate, stats.estimate_complete),
            (Some(111), false)
        );
        let empty = super::stats(
            &range(Vec::new()),
            "2026-11-01".parse().unwrap(),
            "2026-11-02".parse().unwrap(),
        );
        assert_eq!(
            (empty.total, empty.busiest, empty.top_model, empty.estimate),
            (0, None, None, Some(0))
        );
    }

    #[test]
    fn images_hold_no_private_fields_escape_text_and_keep_missing_days_distinct() {
        let stats = stats(
            &range(vec![day("2026-11-03", "claude", "a<b>&'\"", 5, None)]),
            "2026-11-01".parse().unwrap(),
            "2026-11-08".parse().unwrap(),
        );
        let card = wrapped(&stats, true);
        assert!(card.contains("a&lt;b&gt;&amp;&#39;&quot;") && !card.contains("a<b>"));
        assert!(card.contains("Estimated value Not available"));
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
        let stats = stats(
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
