# Shared charts and native desktop presentation

October 21, 2026. This decision follows the user's requests for reusable components, the supplied three-panel dashboard, blue heatmaps and native desktop polish. The normative SRS is unchanged.

## Reuse

The calendar uses pinned MIT `react-activity-calendar` 3.2.1. The donut uses pinned MIT Recharts 3.10.1. Existing coss/Base UI controls remain shared. Package integrity is recorded in the npm lockfile, and distributed notices accompany the frontend. The inspected public Codex repository provides CLI, SDK and terminal UI code; it does not supply a reusable desktop React interface in the inspected tree.

Rust prepares exact decimal strings, calendar dates, intensity levels, ISO-week subtotals and selected-period cumulative values. The calendar's numeric count field is unused. Its total, legend totals and tooltips are disabled. All visible values, accessible labels and the equivalent table use Rust results. Missing dates retain a distinct outline and never become verified zeroes. Calendar-generated filler dates do not become saved readings.

The Overview retains three panels: selected-account quotas, a donut, and blue calendar activity. The donut's selected-account view shows a real reported quota window and the Rust-derived remaining percentage. Account-share mode shows an explicit unavailable state until verified identity and compatible scope/coverage permit aggregation. No percentages, account shares, selected-period token totals or trends are inferred from provisional account identities. Combined selection exposes each exclusion without a fabricated chart.

## Native presentation

Tauri's standard window decorations remain enabled. Its native controls preserve operating-system dragging, resizing and keyboard actions without new renderer permissions or a custom titlebar dependency. Successful workspace operations synchronize saved Light, Dark or System appearance through Tauri's native theme API. Linux compositors can choose their own frame appearance.

The old native assets contained a teal OU placeholder, while the renderer already used the approved growth mark. The new icon-only SVG follows that mark. The official Tauri 2.12.4 icon command generates PNG, multi-image ICO and ICNS assets. White backing gives the black mark contrast on both themes; transparent rounded corners preserve the silhouette. Renderer branding remains unchanged. System-tray implementation is outside this slice.

The host uses WSLg/Weston. Synthetic native acceptance uses Xvfb without a window manager, so its screenshots cannot qualify host titlebar appearance or Windows/macOS controls. Linux package and host-window results are recorded separately in acceptance evidence. Generated platform assets do not establish platform execution, installer signing or release readiness.

## Sources

- [Calendar source](https://github.com/grubersjoe/react-activity-calendar), [license](https://github.com/grubersjoe/react-activity-calendar/blob/main/LICENSE)
- [Recharts Pie](https://recharts.github.io/en-US/api/Pie/), [source and MIT license](https://github.com/recharts/recharts)
- [Coss component licensing](https://github.com/cosscom/ui/blob/main/LICENSING.md)
- [Codex workspace](https://github.com/openai/codex/blob/main/pnpm-workspace.yaml), [terminal UI](https://github.com/openai/codex/blob/main/codex-rs/tui/Cargo.toml)
- [Tauri icons](https://v2.tauri.app/develop/icons/), [window customization](https://v2.tauri.app/learn/window-customization/), [Debian packaging](https://v2.tauri.app/distribute/debian/)
