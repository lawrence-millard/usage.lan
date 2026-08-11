use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UsageRecord {
    pub agent: String,
    pub sub_agent: String,
    pub model: String,
    pub provider: String,
    pub ts_ms: i64,
    pub tokens_input: u64,
    pub tokens_output: u64,
    pub tokens_reasoning: u64,
    pub tokens_cache_read: u64,
    pub tokens_cache_write: u64,
    pub cost: f64,
    pub project: String,
    pub title: String,
    pub sessions: u64,
}

impl UsageRecord {
    pub fn tokens(&self) -> u64 {
        self.tokens_input
            .saturating_add(self.tokens_output)
            .saturating_add(self.tokens_reasoning)
            .saturating_add(self.tokens_cache_read)
            .saturating_add(self.tokens_cache_write)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PeriodStats {
    pub cost: f64,
    pub tokens: u64,
    pub sessions: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PeriodTotals {
    pub today: PeriodStats,
    pub yesterday: PeriodStats,
    pub last7: PeriodStats,
    pub last30: PeriodStats,
    pub all: PeriodStats,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProviderSummary {
    pub id: String,
    pub name: String,
    pub cost: f64,
    pub tokens: u64,
    pub sessions: u64,
    pub pct_cost: f64,
    pub pct_tokens: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModelRow {
    pub model: String,
    pub provider: String,
    pub cost: f64,
    pub tokens: u64,
    pub sessions: u64,
    pub pct_cost: f64,
    pub pct_tokens: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentRow {
    pub agent: String,
    pub cost: f64,
    pub tokens: u64,
    pub sessions: u64,
    pub pct_cost: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectRow {
    pub project: String,
    pub cost: f64,
    pub tokens: u64,
    pub sessions: u64,
    pub pct_cost: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DayPoint {
    pub label: String,
    pub date_iso: String,
    pub cost: f64,
    pub tokens: u64,
    pub sessions: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SourceStatus {
    pub id: String,
    pub name: String,
    pub ok: bool,
    pub detail: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UsageReport {
    pub agent: String,
    pub days: i64,
    pub generated_at_ms: i64,
    pub window: PeriodStats,
    pub periods: PeriodTotals,
    pub providers: Vec<ProviderSummary>,
    pub by_model: Vec<ModelRow>,
    pub by_agent: Vec<AgentRow>,
    pub by_project: Vec<ProjectRow>,
    pub by_day: Vec<DayPoint>,
    pub agents: Vec<String>,
    pub sources: Vec<SourceStatus>,
    pub record_count: u64,
}

pub const AGENTS: [&str; 3] = ["opencode", "codex", "cursor"];
