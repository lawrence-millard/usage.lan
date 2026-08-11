use std::collections::BTreeMap;

use chrono::{DateTime, Duration, Local};
use usage_core::models::*;

pub fn build_report(
    records: &[UsageRecord],
    sources: Vec<SourceStatus>,
    agent: &str,
    days: i64,
    now: DateTime<Local>,
) -> UsageReport {
    let filtered: Vec<&UsageRecord> = records
        .iter()
        .filter(|r| agent == "all" || r.agent == agent)
        .collect();

    let mut agents: Vec<String> = records.iter().map(|r| r.agent.clone()).collect();
    agents.sort();
    agents.dedup();

    // ---- window ----
    let window_start = if days > 0 {
        Some(now - Duration::days(days))
    } else {
        None
    };
    let window_records: Vec<&UsageRecord> = filtered
        .iter()
        .copied()
        .filter(|r| window_start.map_or(true, |w| local_dt(r.ts_ms).is_some_and(|t| t >= w)))
        .collect();

    let window = sum_of(window_records.iter().copied());

    // ---- day buckets ----
    let mut bucket_map: BTreeMap<String, (String, PeriodStats)> = BTreeMap::new();
    for rec in &window_records {
        let Some(dt) = local_dt(rec.ts_ms) else {
            continue;
        };
        let key = day_key(&dt);
        let entry = bucket_map
            .entry(key.clone())
            .or_insert_with(|| (day_label(&dt), PeriodStats::default()));
        add_stats(&mut entry.1, rec);
    }

    let by_day = if days > 0 {
        let today = now.date_naive();
        let mut out = Vec::new();
        for i in (0..days).rev() {
            let d = today - Duration::days(i);
            let key = d.format("%Y-%m-%d").to_string();
            let label = d.format("%b %d").to_string();
            let stats = bucket_map
                .get(&key)
                .map(|(_, s)| s.clone())
                .unwrap_or_default();
            out.push(DayPoint {
                label,
                date_iso: key,
                cost: stats.cost,
                tokens: stats.tokens,
                sessions: stats.sessions,
            });
        }
        out
    } else {
        bucket_map
            .into_iter()
            .map(|(date_iso, (label, s))| DayPoint {
                label,
                date_iso,
                cost: s.cost,
                tokens: s.tokens,
                sessions: s.sessions,
            })
            .collect()
    };

    // ---- periods ----
    let today_start = now
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_local_timezone(Local)
        .single()
        .unwrap_or(now);
    let mut periods = PeriodTotals::default();
    for rec in &filtered {
        let Some(t) = local_dt(rec.ts_ms) else {
            continue;
        };
        if t >= today_start {
            add_stats(&mut periods.today, rec);
        }
        if t >= today_start - Duration::days(1) && t < today_start {
            add_stats(&mut periods.yesterday, rec);
        }
        if t >= now - Duration::days(7) {
            add_stats(&mut periods.last7, rec);
        }
        if t >= now - Duration::days(30) {
            add_stats(&mut periods.last30, rec);
        }
        add_stats(&mut periods.all, rec);
    }

    // ---- providers ----
    let total_cost = if window.cost > 0.0 { window.cost } else { 1.0 };
    let total_tokens = if window.tokens > 0 { window.tokens } else { 1 };
    let provider_names = [
        ("opencode", "OpenCode"),
        ("codex", "Codex"),
        ("cursor", "Cursor"),
    ];
    let mut providers = Vec::new();
    for (id, name) in provider_names {
        if agent != "all" && agent != id {
            continue;
        }
        let stats = sum_of(window_records.iter().copied().filter(|r| r.agent == id));
        providers.push(ProviderSummary {
            id: id.into(),
            name: name.into(),
            cost: stats.cost,
            tokens: stats.tokens,
            sessions: stats.sessions,
            pct_cost: stats.cost / total_cost,
            pct_tokens: stats.tokens as f64 / total_tokens as f64,
        });
    }

    // ---- by model ----
    let mut model_map: BTreeMap<(String, String), PeriodStats> = BTreeMap::new();
    for rec in &window_records {
        let key = (rec.model.clone(), rec.provider.clone());
        add_stats(model_map.entry(key).or_default(), rec);
    }
    let mut by_model: Vec<ModelRow> = model_map
        .into_iter()
        .map(|((model, provider), s)| ModelRow {
            model,
            provider,
            cost: s.cost,
            tokens: s.tokens,
            sessions: s.sessions,
            pct_cost: s.cost / total_cost,
            pct_tokens: s.tokens as f64 / total_tokens as f64,
        })
        .collect();
    by_model.sort_by(|a, b| {
        b.cost
            .partial_cmp(&a.cost)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // ---- by agent (sub-agents) ----
    let mut agent_map: BTreeMap<String, PeriodStats> = BTreeMap::new();
    for rec in &window_records {
        let name = row_agent(rec);
        add_stats(agent_map.entry(name).or_default(), rec);
    }
    let mut by_agent: Vec<AgentRow> = agent_map
        .into_iter()
        .map(|(agent, s)| AgentRow {
            agent,
            cost: s.cost,
            tokens: s.tokens,
            sessions: s.sessions,
            pct_cost: s.cost / total_cost,
        })
        .collect();
    by_agent.sort_by(|a, b| {
        b.cost
            .partial_cmp(&a.cost)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // ---- by project ----
    let mut project_map: BTreeMap<String, PeriodStats> = BTreeMap::new();
    for rec in &window_records {
        if rec.project.is_empty() {
            continue;
        }
        add_stats(project_map.entry(rec.project.clone()).or_default(), rec);
    }
    let mut by_project: Vec<ProjectRow> = project_map
        .into_iter()
        .map(|(project, s)| ProjectRow {
            project,
            cost: s.cost,
            tokens: s.tokens,
            sessions: s.sessions,
            pct_cost: s.cost / total_cost,
        })
        .collect();
    by_project.sort_by(|a, b| {
        b.cost
            .partial_cmp(&a.cost)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    UsageReport {
        agent: agent.into(),
        days,
        generated_at_ms: now.timestamp_millis(),
        window,
        periods,
        providers,
        by_model,
        by_agent,
        by_project,
        by_day,
        agents,
        sources,
        record_count: filtered.len() as u64,
    }
}

fn row_agent(rec: &UsageRecord) -> String {
    if rec.sub_agent.is_empty() {
        rec.agent.clone()
    } else {
        rec.sub_agent.clone()
    }
}

fn local_dt(ts_ms: i64) -> Option<DateTime<Local>> {
    DateTime::from_timestamp_millis(ts_ms).map(|u| u.with_timezone(&Local))
}

fn day_key(dt: &DateTime<Local>) -> String {
    dt.format("%Y-%m-%d").to_string()
}

fn day_label(dt: &DateTime<Local>) -> String {
    dt.format("%b %d").to_string()
}

fn sum_of<'a>(recs: impl Iterator<Item = &'a UsageRecord>) -> PeriodStats {
    let mut s = PeriodStats::default();
    for r in recs {
        add_stats(&mut s, r);
    }
    s
}

fn add_stats(s: &mut PeriodStats, r: &UsageRecord) {
    s.cost += r.cost;
    s.tokens += r.tokens();
    s.sessions += r.sessions;
}
