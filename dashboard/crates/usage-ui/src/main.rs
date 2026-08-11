use futures::AsyncReadExt;
use open_gpui::prelude::*;
use open_gpui::{
    div, px, rgb, size, App, Bounds, Context, ElementId, FontWeight, Task, Window,
    WindowBounds, WindowOptions,
};
use usage_core::models::*;

struct Dashboard {
    selected_agent: String,
    selected_days: i64,
    report: Option<UsageReport>,
    is_loading: bool,
    error: Option<String>,
    _task: Option<Task<()>>,
}

impl Dashboard {
    fn new(cx: &mut Context<Self>) -> Self {
        let mut this = Self {
            selected_agent: "all".to_string(),
            selected_days: 30,
            report: None,
            is_loading: false,
            error: None,
            _task: None,
        };
        this.fetch(cx);
        this
    }

    fn fetch(&mut self, cx: &mut Context<Self>) {
        self.is_loading = true;
        self.error = None;
        cx.notify();

        let agent = self.selected_agent.clone();
        let days = self.selected_days;
        let client = cx.http_client().clone();

        self._task = Some(cx.spawn(async move |this, cx| {
            let url = format!("/api/usage?agent={}&days={}", agent, days);

            let res = async {
                let resp = client.get(&url, open_gpui::http_client::AsyncBody::empty(), true).await?;
                if !resp.status().is_success() {
                    return Err(anyhow::anyhow!("HTTP status {}", resp.status()));
                }
                let mut body = resp.into_body();
                let mut buf = Vec::new();
                body.read_to_end(&mut buf).await?;
                let report: UsageReport = serde_json::from_slice(&buf)?;
                Ok(report)
            }.await;

            this.update(cx, |this, cx| {
                this.is_loading = false;
                match res {
                    Ok(r) => {
                        this.report = Some(r);
                    }
                    Err(e) => {
                        this.error = Some(e.to_string());
                    }
                }
                cx.notify();
            }).ok();
        }));
    }
}

// Color Palette Constants
const BG_PAGE: u32 = 0xf3f4f6;       // Gray 100
const BG_CARD: u32 = 0xffffff;       // White
const BORDER_COLOR: u32 = 0xe5e7eb;   // Gray 200
const TEXT_MAIN: u32 = 0x111827;     // Gray 900
const TEXT_MUTED: u32 = 0x4b5563;    // Gray 600
const TEXT_LIGHT: u32 = 0x9ca3af;    // Gray 400
const COLOR_BLUE: u32 = 0x3b82f6;    // Blue 500
const COLOR_GREEN: u32 = 0x10b981;   // Emerald 500
const COLOR_RED: u32 = 0xef4444;     // Red 500

impl Render for Dashboard {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // --- HEADER ---
        let title_section = div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .child(
                div()
                    .text_xl()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(TEXT_MAIN))
                    .child("usage.local"),
            )
            .child(
                div()
                    .px_2()
                    .py_0p5()
                    .rounded_md()
                    .bg(rgb(0xdbeafe))
                    .text_color(rgb(0x1e40af))
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .child("v1.0.0"),
            );

        // Filter Controls
        let agent_options = vec![
            ("all", "All Agents"),
            ("opencode", "OpenCode"),
            ("codex", "Codex"),
            ("cursor", "Cursor"),
        ];

        let agent_selector = agent_options.into_iter().enumerate().fold(
            div().flex().flex_row().gap_1().bg(rgb(0xe5e7eb)).p_1().rounded_lg(),
            |row, (idx, (val, label))| {
                let is_selected = self.selected_agent == val;
                row.child(
                    div()
                        .id(ElementId::NamedInteger("agent_opt".into(), idx as u64))
                        .px_3()
                        .py_1()
                        .rounded_md()
                        .bg(rgb(if is_selected { 0xffffff } else { 0xe5e7eb }))
                        .text_color(rgb(if is_selected { TEXT_MAIN } else { TEXT_MUTED }))
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _event, _window, cx| {
                            this.selected_agent = val.to_string();
                            this.fetch(cx);
                        }))
                        .child(label),
                )
            },
        );

        let days_options = vec![
            (7, "7 Days"),
            (30, "30 Days"),
            (90, "90 Days"),
            (-1, "All Time"),
        ];

        let days_selector = days_options.into_iter().enumerate().fold(
            div().flex().flex_row().gap_1().bg(rgb(0xe5e7eb)).p_1().rounded_lg(),
            |row, (idx, (val, label))| {
                let is_selected = self.selected_days == val;
                row.child(
                    div()
                        .id(ElementId::NamedInteger("days_opt".into(), idx as u64))
                        .px_3()
                        .py_1()
                        .rounded_md()
                        .bg(rgb(if is_selected { 0xffffff } else { 0xe5e7eb }))
                        .text_color(rgb(if is_selected { TEXT_MAIN } else { TEXT_MUTED }))
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _event, _window, cx| {
                            this.selected_days = val;
                            this.fetch(cx);
                        }))
                        .child(label),
                )
            },
        );

        let header = div()
            .flex()
            .flex_row()
            .justify_between()
            .items_center()
            .px_6()
            .py_4()
            .bg(rgb(BG_CARD))
            .border_b_1()
            .border_color(rgb(BORDER_COLOR))
            .child(title_section)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_4()
                    .items_center()
                    .child(agent_selector)
                    .child(days_selector),
            );

        // --- MAIN CONTAINER ---
        let mut main_content = div()
            .id("main-scroll")
            .flex()
            .flex_col()
            .flex_grow(1.0)
            .p_6()
            .gap_6()
            .overflow_y_scroll();

        if self.is_loading && self.report.is_none() {
            main_content = main_content.child(
                div()
                    .flex()
                    .justify_center()
                    .items_center()
                    .h_64()
                    .text_lg()
                    .text_color(rgb(TEXT_MUTED))
                    .child("Loading usage data..."),
            );
        } else if let Some(err) = &self.error {
            main_content = main_content.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_4()
                    .bg(rgb(0xfef2f2))
                    .border_1()
                    .border_color(rgb(COLOR_RED))
                    .rounded_lg()
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(COLOR_RED))
                            .child("Failed to fetch dashboard data"),
                    )
                    .child(div().text_sm().text_color(rgb(TEXT_MUTED)).child(err.clone())),
            );
        } else if let Some(report) = &self.report {
            // --- SOURCE STATUS HEADER BADGES ---
            let mut status_row = div().flex().flex_row().gap_3().flex_wrap();
            for src in &report.sources {
                let (color_bg, color_txt, dot_color, status_txt) = if src.ok {
                    (0xd1fae5, 0x065f46, 0x10b981, format!("{} ({})", src.name, src.detail))
                } else {
                    (0xfef2f2, 0x991b1b, 0xef4444, format!("{}: {}", src.name, src.detail))
                };
                status_row = status_row.child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .px_2p5()
                        .py_1()
                        .rounded_full()
                        .bg(rgb(color_bg))
                        .text_color(rgb(color_txt))
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .child(div().size(px(6.)).rounded_full().bg(rgb(dot_color)))
                        .child(status_txt),
                );
            }
            main_content = main_content.child(status_row);

            // --- STAT CARDS ---
            let cost_today = report.periods.today.cost;
            let cost_yest = report.periods.yesterday.cost;
            let pct_change = if cost_yest > 0.0 {
                (cost_today - cost_yest) / cost_yest * 100.0
            } else {
                0.0
            };
            let trend_txt = if pct_change > 0.0 {
                format!("▲ {:.1}% vs yesterday", pct_change)
            } else if pct_change < 0.0 {
                format!("▼ {:.1}% vs yesterday", pct_change.abs())
            } else {
                "flat vs yesterday".to_string()
            };
            let trend_color = if pct_change > 0.0 {
                COLOR_RED
            } else if pct_change < 0.0 {
                COLOR_GREEN
            } else {
                TEXT_LIGHT
            };

            let cards_grid = div()
                .flex()
                .flex_row()
                .gap_4()
                .w_full()
                .child(render_stat_card(
                    "Today's Cost",
                    &format!("${:.3}", cost_today),
                    &trend_txt,
                    trend_color,
                ))
                .child(render_stat_card(
                    "Today's Tokens",
                    &format_number(report.periods.today.tokens),
                    &format!("{} sessions", report.periods.today.sessions),
                    TEXT_MUTED,
                ))
                .child(render_stat_card(
                    "Last 7 Days",
                    &format!("${:.2}", report.periods.last7.cost),
                    &format!("{} tokens", format_number(report.periods.last7.tokens)),
                    TEXT_MUTED,
                ))
                .child(render_stat_card(
                    "Last 30 Days",
                    &format!("${:.2}", report.periods.last30.cost),
                    &format!("{} tokens", format_number(report.periods.last30.tokens)),
                    TEXT_MUTED,
                ))
                .child(render_stat_card(
                    "Selected Window",
                    &format!("${:.2}", report.window.cost),
                    &format!("{} tokens / {} sessions", format_number(report.window.tokens), report.window.sessions),
                    TEXT_MUTED,
                ));
            main_content = main_content.child(cards_grid);

            // --- DAILY CHART ---
            let max_cost = report.by_day.iter().map(|d| d.cost).fold(0.001_f64, |a, b| a.max(b));
            let mut chart_bars = div()
                .flex()
                .flex_row()
                .justify_between()
                .items_end()
                .h(px(140.))
                .px_4()
                .w_full();

            for (idx, day) in report.by_day.iter().enumerate() {
                let height_ratio = (day.cost / max_cost) as f32;
                let bar_height = px((height_ratio * 110.0).max(4.0));
                
                chart_bars = chart_bars.child(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .flex_grow(1.0)
                        .child(
                            div()
                                .id(ElementId::NamedInteger("bar".into(), idx as u64))
                                .w(px(24.))
                                .h(bar_height)
                                .bg(rgb(if height_ratio > 0.0 { COLOR_BLUE } else { 0xe5e7eb }))
                                .rounded_t_sm(),
                        )
                        .child(
                            div()
                                .mt_2()
                                .text_xs()
                                .text_color(rgb(TEXT_LIGHT))
                                .child(day.label.clone()),
                        ),
                );
            }

            let chart_card = div()
                .flex()
                .flex_col()
                .p_6()
                .bg(rgb(BG_CARD))
                .border_1()
                .border_color(rgb(BORDER_COLOR))
                .rounded_lg()
                .child(
                    div()
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(TEXT_MAIN))
                        .mb_4()
                        .child("Daily Cost Trend ($)"),
                )
                .child(chart_bars);
            main_content = main_content.child(chart_card);

            // --- PROVIDERS BREAKDOWN ---
            let mut provider_row = div().flex().flex_row().gap_4().w_full();
            for (idx, prov) in report.providers.iter().enumerate() {
                provider_row = provider_row.child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .p_5()
                        .bg(rgb(BG_CARD))
                        .border_1()
                        .border_color(rgb(BORDER_COLOR))
                        .rounded_lg()
                        .child(
                            div()
                                .font_weight(FontWeight::BOLD)
                                .text_color(rgb(TEXT_MAIN))
                                .text_sm()
                                .child(prov.name.clone()),
                        )
                        .child(
                            div()
                                .text_2xl()
                                .font_weight(FontWeight::BOLD)
                                .text_color(rgb(TEXT_MAIN))
                                .my_2()
                                .child(format!("${:.3}", prov.cost)),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(TEXT_MUTED))
                                .child(format!("{} tokens", format_number(prov.tokens))),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(TEXT_MUTED))
                                .child(format!("{} sessions", prov.sessions)),
                        )
                        .child(
                            div()
                                .w_full()
                                .h(px(4.))
                                .bg(rgb(0xf3f4f6))
                                .rounded_full()
                                .mt_3()
                                .child(
                                    div()
                                        .id(ElementId::NamedInteger("prov_bar".into(), idx as u64))
                                        .h_full()
                                        .rounded_full()
                                        .bg(rgb(COLOR_BLUE))
                                        .w(open_gpui::relative(prov.pct_cost as f32)),
                                ),
                        ),
                );
            }
            main_content = main_content.child(provider_row);

            // --- DETAILED BREAKDOWNS (TABLES) ---
            let mut model_table = div()
                .flex()
                .flex_col()
                .flex_grow(1.0)
                .p_5()
                .bg(rgb(BG_CARD))
                .border_1()
                .border_color(rgb(BORDER_COLOR))
                .rounded_lg();

            model_table = model_table
                .child(
                    div()
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(TEXT_MAIN))
                        .mb_3()
                        .child("Model Usage & Costs"),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .pb_2()
                        .border_b_1()
                        .border_color(rgb(BORDER_COLOR))
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(TEXT_MUTED))
                        .child(div().w(px(200.)).child("Model"))
                        .child(div().w(px(100.)).child("Provider"))
                        .child(div().w(px(80.)).child("Cost"))
                        .child(div().w(px(120.)).child("Tokens"))
                        .child(div().w(px(180.)).child("Relative Share")),
                );

            for (idx, m) in report.by_model.iter().enumerate() {
                model_table = model_table.child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .py_2p5()
                        .border_b_1()
                        .border_color(rgb(BORDER_COLOR))
                        .text_sm()
                        .text_color(rgb(TEXT_MAIN))
                        .child(
                            div()
                                .w(px(200.))
                                .font_weight(FontWeight::MEDIUM)
                                .child(m.model.clone()),
                        )
                        .child(div().w(px(100.)).child(m.provider.clone()))
                        .child(
                            div()
                                .w(px(80.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(format!("${:.3}", m.cost)),
                        )
                        .child(div().w(px(120.)).child(format_number(m.tokens)))
                        .child(
                            div()
                                .w(px(180.))
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap_2()
                                .child(
                                    div()
                                        .w(px(100.))
                                        .h(px(8.))
                                        .bg(rgb(0xf3f4f6))
                                        .rounded_sm()
                                        .child(
                                            div()
                                                .id(ElementId::NamedInteger("model_bar".into(), idx as u64))
                                                .h_full()
                                                .rounded_sm()
                                                .bg(rgb(COLOR_BLUE))
                                                .w(open_gpui::relative(m.pct_cost as f32)),
                                        ),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(TEXT_MUTED))
                                        .child(format!("{:.1}%", m.pct_cost * 100.0)),
                                ),
                        ),
                );
            }

            let mut side_tables = div()
                .flex()
                .flex_col()
                .gap_6()
                .w(px(350.));

            // Sub-Agents table
            let mut agent_table = div()
                .flex()
                .flex_col()
                .p_5()
                .bg(rgb(BG_CARD))
                .border_1()
                .border_color(rgb(BORDER_COLOR))
                .rounded_lg()
                .child(
                    div()
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(TEXT_MAIN))
                        .mb_3()
                        .child("Agent / Tool Breakdown"),
                );

            for a in report.by_agent.iter() {
                agent_table = agent_table.child(
                    div()
                        .flex()
                        .flex_row()
                        .justify_between()
                        .items_center()
                        .py_2()
                        .text_sm()
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(rgb(TEXT_MAIN))
                                        .child(a.agent.clone()),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(TEXT_MUTED))
                                        .child(format!("{} sessions", a.sessions)),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .items_end()
                                .child(
                                    div()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(rgb(TEXT_MAIN))
                                        .child(format!("${:.3}", a.cost)),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(TEXT_MUTED))
                                        .child(format!("{:.1}%", a.pct_cost * 100.0)),
                                ),
                        ),
                );
            }
            side_tables = side_tables.child(agent_table);

            // Projects table
            let mut project_table = div()
                .flex()
                .flex_col()
                .p_5()
                .bg(rgb(BG_CARD))
                .border_1()
                .border_color(rgb(BORDER_COLOR))
                .rounded_lg()
                .child(
                    div()
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(TEXT_MAIN))
                        .mb_3()
                        .child("Workspace Projects"),
                );

            for p in report.by_project.iter() {
                project_table = project_table.child(
                    div()
                        .flex()
                        .flex_row()
                        .justify_between()
                        .items_center()
                        .py_2()
                        .text_sm()
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(rgb(TEXT_MAIN))
                                        .child(p.project.clone()),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(TEXT_MUTED))
                                        .child(format!("{} sessions", p.sessions)),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .items_end()
                                .child(
                                    div()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(rgb(TEXT_MAIN))
                                        .child(format!("${:.3}", p.cost)),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(TEXT_MUTED))
                                        .child(format!("{:.1}%", p.pct_cost * 100.0)),
                                ),
                        ),
                );
            }
            side_tables = side_tables.child(project_table);

            let detailed_grid = div()
                .flex()
                .flex_row()
                .gap_6()
                .w_full()
                .child(model_table)
                .child(side_tables);
            main_content = main_content.child(detailed_grid);
        } else {
            main_content = main_content.child(
                div()
                    .flex()
                    .justify_center()
                    .items_center()
                    .h_64()
                    .text_lg()
                    .text_color(rgb(TEXT_MUTED))
                    .child("No data loaded."),
            );
        }

        // --- FULL LAYOUT ---
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(BG_PAGE))
            .child(header)
            .child(main_content)
    }
}

fn render_stat_card(
    title: &'static str,
    value: &str,
    subtext: &str,
    subtext_color: u32,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .flex_1()
        .p_5()
        .bg(rgb(BG_CARD))
        .border_1()
        .border_color(rgb(BORDER_COLOR))
        .rounded_lg()
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(TEXT_MUTED))
                .child(title),
        )
        .child(
            div()
                .text_2xl()
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(TEXT_MAIN))
                .my_1()
                .child(value.to_string()),
        )
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::MEDIUM)
                .text_color(rgb(subtext_color))
                .child(subtext.to_string()),
        )
}

fn format_number(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    result.chars().rev().collect()
}

fn main() {
    open_gpui_platform::web_init();
    let http_client = unsafe { open_gpui_web::FetchHttpClient::new() };
    open_gpui_platform::application()
        .with_http_client(std::sync::Arc::new(http_client))
        .run(|cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(1280.), px(800.)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |_, cx| cx.new(Dashboard::new),
            )
            .expect("failed to open window");
            cx.activate(true);
        });
}
