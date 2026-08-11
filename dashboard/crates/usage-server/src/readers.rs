use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use usage_core::models::{SourceStatus, UsageRecord};
use usage_core::pricing;

const FAST_TTL: Duration = Duration::from_secs(30);
const CURSOR_TTL: Duration = Duration::from_secs(600);

pub struct Reader {
    fast: Mutex<Option<(Instant, Vec<UsageRecord>, Vec<SourceStatus>)>>,
    cursor: Mutex<Option<(Instant, Vec<UsageRecord>, SourceStatus)>>,
}

#[derive(Default, Clone)]
pub struct ReadOut {
    pub records: Vec<UsageRecord>,
    pub sources: Vec<SourceStatus>,
}

impl Reader {
    pub fn new() -> Self {
        Self {
            fast: Mutex::new(None),
            cursor: Mutex::new(None),
        }
    }

    pub fn read(&self) -> ReadOut {
        let mut out = ReadOut::default();

        {
            let mut guard = self.fast.lock().unwrap();
            let fresh = guard
                .as_ref()
                .is_some_and(|(at, _, _)| at.elapsed() < FAST_TTL);
            if !fresh {
                let (opencode_recs, opencode_status) = read_opencode();
                let (codex_recs, codex_status) = read_codex();
                let mut records = Vec::with_capacity(opencode_recs.len() + codex_recs.len());
                records.extend(opencode_recs);
                records.extend(codex_recs);
                let sources = vec![opencode_status, codex_status];
                *guard = Some((Instant::now(), records, sources));
            }
            if let Some((_, records, sources)) = guard.as_ref() {
                out.records.extend(records.iter().cloned());
                out.sources.extend(sources.iter().cloned());
            }
        }

        {
            let mut guard = self.cursor.lock().unwrap();
            let fresh = guard
                .as_ref()
                .is_some_and(|(at, _, _)| at.elapsed() < CURSOR_TTL);
            if !fresh {
                let (recs, status) = read_cursor_blocking();
                *guard = Some((Instant::now(), recs, status));
            }
            if let Some((_, records, status)) = guard.as_ref() {
                out.records.extend(records.iter().cloned());
                out.sources.push(status.clone());
            }
        }

        out
    }
}

fn read_cursor_blocking() -> (Vec<UsageRecord>, SourceStatus) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            let stat = status("cursor", "Cursor", &Err(format!("runtime init failed: {e}")));
            return (Vec::new(), stat);
        }
    };
    runtime.block_on(async {
        match tokio::time::timeout(CURSOR_TIMEOUT, read_cursor()).await {
            Ok((records, stat)) => (records, stat),
            Err(_) => {
                let stat = status("cursor", "Cursor", &Err("cursor request timed out".into()));
                (Vec::new(), stat)
            }
        }
    })
}

const CURSOR_TIMEOUT: Duration = Duration::from_secs(5);

fn home() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/"))
}

fn status(id: &str, name: &str, result: &Result<Vec<UsageRecord>, String>) -> SourceStatus {
    match result {
        Ok(records) => SourceStatus {
            id: id.into(),
            name: name.into(),
            ok: true,
            detail: format!("{} sessions", records.len()),
        },
        Err(e) => SourceStatus {
            id: id.into(),
            name: name.into(),
            ok: false,
            detail: e.clone(),
        },
    }
}

fn parse_model_json(raw: &str) -> (String, String) {
    match serde_json::from_str::<serde_json::Value>(raw) {
        Ok(v) => (
            v["id"].as_str().unwrap_or(raw).to_string(),
            v["providerID"].as_str().unwrap_or("").to_string(),
        ),
        Err(_) => (raw.to_string(), String::new()),
    }
}

fn project_from_dir(dir: &str) -> String {
    Path::new(dir)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default()
}

fn estimate_if_needed(mut rec: UsageRecord) -> UsageRecord {
    if rec.cost <= 0.0 {
        rec.cost = pricing::estimate_cost(&rec.model, &rec.provider, &rec);
    }
    rec
}

// ---------------------------------------------------------------------------
// OpenCode
// ---------------------------------------------------------------------------

fn read_opencode() -> (Vec<UsageRecord>, SourceStatus) {
    let data_dir = home().join(".local/share/opencode");
    let result = (|| -> Result<Vec<UsageRecord>, String> {
        let dbs: Vec<PathBuf> = std::fs::read_dir(&data_dir)
            .map_err(|e| e.to_string())?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.starts_with("opencode") && n.ends_with(".db"))
                    .unwrap_or(false)
            })
            .collect();
        if dbs.is_empty() {
            return Err(format!("no opencode.db found in {}", data_dir.display()));
        }

        let mut out = Vec::new();
        for db in dbs {
            let conn = rusqlite::Connection::open_with_flags(
                &db,
                rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
            )
            .map_err(|e| e.to_string())?;
            let mut stmt = conn
                .prepare(
                    "SELECT time_created, model, agent, cost, tokens_input, tokens_output, \
                     tokens_reasoning, tokens_cache_read, tokens_cache_write, title, directory \
                     FROM session",
                )
                .map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map([], |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, f64>(3)?,
                        r.get::<_, i64>(4)? as u64,
                        r.get::<_, i64>(5)? as u64,
                        r.get::<_, i64>(6)? as u64,
                        r.get::<_, i64>(7)? as u64,
                        r.get::<_, i64>(8)? as u64,
                        r.get::<_, String>(9)?,
                        r.get::<_, String>(10)?,
                    ))
                })
                .map_err(|e| e.to_string())?;

            for row in rows.flatten() {
                let (model, provider) = parse_model_json(&row.1);
                let rec = estimate_if_needed(UsageRecord {
                    agent: "opencode".into(),
                    sub_agent: row.2.clone(),
                    model,
                    provider,
                    ts_ms: row.0,
                    tokens_input: row.4,
                    tokens_output: row.5,
                    tokens_reasoning: row.6,
                    tokens_cache_read: row.7,
                    tokens_cache_write: row.8,
                    cost: row.3,
                    project: project_from_dir(&row.10),
                    title: row.9,
                    sessions: 1,
                });
                out.push(rec);
            }
        }
        Ok(out)
    })();
    let stat = status("opencode", "OpenCode", &result);
    let records = result.unwrap_or_default();
    (records, stat)
}

// ---------------------------------------------------------------------------
// Codex
// ---------------------------------------------------------------------------

fn read_codex() -> (Vec<UsageRecord>, SourceStatus) {
    let codex_home = std::env::var("CODEX_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| home().join(".codex"));

    let result = (|| -> Result<Vec<UsageRecord>, String> {
        let mut out = Vec::new();

        let state_db = std::fs::read_dir(&codex_home)
            .map_err(|e| e.to_string())?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .find(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.starts_with("state_") && n.ends_with(".sqlite"))
                    .unwrap_or(false)
            });

        if let Some(db) = state_db {
            if let Ok(conn) = rusqlite::Connection::open_with_flags(
                &db,
                rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
            ) {
                if let Ok(mut stmt) = conn.prepare(
                    "SELECT created_at, tokens_used, model, title FROM threads \
                     WHERE tokens_used > 0",
                ) {
                    if let Ok(rows) = stmt.query_map([], |r| {
                        Ok((
                            r.get::<_, i64>(0)?,
                            r.get::<_, i64>(1)? as u64,
                            r.get::<_, Option<String>>(2)?,
                            r.get::<_, String>(3)?,
                        ))
                    }) {
                        for row in rows.flatten() {
                            let model = row.2.unwrap_or_default();
                            let rec = estimate_if_needed(UsageRecord {
                                agent: "codex".into(),
                                sub_agent: String::new(),
                                model: model.clone(),
                                provider: "openai".into(),
                                ts_ms: row.0 * 1000,
                                tokens_input: row.1,
                                tokens_output: 0,
                                tokens_reasoning: 0,
                                tokens_cache_read: 0,
                                tokens_cache_write: 0,
                                cost: 0.0,
                                project: String::new(),
                                title: row.3,
                                sessions: 1,
                            });
                            out.push(rec);
                        }
                    }
                }
            }
        }

        for sub in ["sessions", "archived_sessions"] {
            let base = codex_home.join(sub);
            collect_rollouts(&base, &mut out);
        }

        Ok(out)
    })();

    let stat = status("codex", "Codex", &result);
    let records = result.unwrap_or_default();
    (records, stat)
}

fn collect_rollouts(dir: &Path, out: &mut Vec<UsageRecord>) {
    if !dir.exists() {
        return;
    }
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("rollout-") && n.ends_with(".jsonl"))
                .unwrap_or(false)
            {
                if let Some(rec) = parse_rollout(&path) {
                    out.push(rec);
                }
            }
        }
    }
}

fn parse_rollout(path: &Path) -> Option<UsageRecord> {
    let file = std::fs::File::open(path).ok()?;
    let reader = std::io::BufReader::new(file);

    let mut ts = 0i64;
    let mut model = String::new();
    let mut sum_in = 0u64;
    let mut sum_out = 0u64;
    let mut sum_reason = 0u64;
    let mut sum_cache = 0u64;

    for line in std::io::BufRead::lines(reader) {
        let line = line.ok()?;
        if line.trim().is_empty() {
            continue;
        }
        let v: serde_json::Value = serde_json::from_str(&line).ok()?;
        if ts == 0 {
            ts = v["timestamp"]
                .as_i64()
                .or_else(|| v["timestamp"].as_f64().map(|f| f as i64))
                .unwrap_or(0)
                * 1000;
        }
        if v["type"].as_str() == Some("response_item") {
            if let Some(u) = v.get("usage") {
                sum_in += u["input_tokens"].as_u64().unwrap_or(0);
                sum_out += u["output_tokens"].as_u64().unwrap_or(0);
                sum_reason += u["reasoning_tokens"].as_u64().unwrap_or(0);
                sum_cache += u["cached_input_tokens"].as_u64().unwrap_or(0);
            }
            if model.is_empty() {
                model = v["model"]
                    .as_str()
                    .or_else(|| v["payload"]["model"].as_str())
                    .or_else(|| v["payload"]["response"]["model"].as_str())
                    .unwrap_or("")
                    .to_string();
            }
        }
    }

    if sum_in + sum_out + sum_cache == 0 {
        return None;
    }

    let rec = estimate_if_needed(UsageRecord {
        agent: "codex".into(),
        sub_agent: String::new(),
        model: model.clone(),
        provider: "openai".into(),
        ts_ms: ts,
        tokens_input: sum_in,
        tokens_output: sum_out,
        tokens_reasoning: sum_reason,
        tokens_cache_read: sum_cache,
        tokens_cache_write: 0,
        cost: 0.0,
        project: String::new(),
        title: path
            .parent()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default(),
        sessions: 1,
    });
    Some(rec)
}

// ---------------------------------------------------------------------------
// Cursor
// ---------------------------------------------------------------------------

const CURSOR_CLIENT_ID: &str = "KbZUR41cY7W6zRSdpSUJ7I7mLYBKOCmB";
const CURSOR_REFRESH_URL: &str = "https://api2.cursor.sh/oauth/token";
const CURSOR_CSV_URL: &str = "https://cursor.com/api/dashboard/export-usage-events-csv";
const CURSOR_USER_AGENT: &str =
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Safari/605.1.15";

fn cursor_state_value(key: &str) -> Option<String> {
    let db = home().join("Library/Application Support/Cursor/User/globalStorage/state.vscdb");
    let conn = rusqlite::Connection::open_with_flags(
        &db,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .ok()?;
    let mut stmt = conn.prepare("SELECT value FROM ItemTable WHERE key = ?1 LIMIT 1").ok()?;
    let mut rows = stmt.query_map([key], |r| r.get::<_, String>(0)).ok()?;
    let value = rows.next()?.ok()?.trim().to_string();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

/// Extract the `sub` claim from a JWT's payload (base64url, no validation).
fn jwt_subject(token: &str) -> Option<String> {
    let part = token.split('.').nth(1)?;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine as _;
    let json = URL_SAFE_NO_PAD.decode(part).ok()?;
    let v: serde_json::Value = serde_json::from_slice(&json).ok()?;
    v["sub"].as_str().map(|s| s.to_string())
}

/// Build the `WorkosCursorSessionToken` cookie value from the access token's subject.
fn cursor_session_cookie(token: &str) -> Option<String> {
    let subject = jwt_subject(token)?;
    let parts: Vec<&str> = subject.split('|').collect();
    let user_id = if parts.len() > 1 {
        parts[1]
    } else {
        parts[0]
    };
    if user_id.is_empty() {
        return None;
    }
    Some(format!("{}%3A%3A{}", user_id, token))
}

async fn refresh_cursor_token(refresh_token: &str) -> Result<String, String> {
    let client = reqwest::Client::new();
    let resp = client
        .post(CURSOR_REFRESH_URL)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "grant_type": "refresh_token",
            "client_id": CURSOR_CLIENT_ID,
            "refresh_token": refresh_token,
        }))
        .send()
        .await
        .map_err(|e| format!("cursor token refresh failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("cursor token refresh returned {}", resp.status()));
    }
    let v: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("cursor token refresh bad body: {e}"))?;
    v["access_token"]
        .as_str()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "cursor token refresh returned no access_token".to_string())
}

async fn fetch_cursor_csv(cookie: &str, start_ms: i64, end_ms: i64) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .get(CURSOR_CSV_URL)
        .query(&[
            ("startDate", start_ms.to_string()),
            ("endDate", end_ms.to_string()),
            ("strategy", "tokens".to_string()),
        ])
        .header("Cookie", format!("WorkosCursorSessionToken={cookie}"))
        .header("Accept", "text/csv")
        .header("User-Agent", CURSOR_USER_AGENT)
        .send()
        .await
        .map_err(|e| format!("cursor export request failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("cursor export returned {}", resp.status()));
    }
    resp.text().await.map_err(|e| e.to_string())
}

async fn read_cursor() -> (Vec<UsageRecord>, SourceStatus) {
    let result = (|| async move {
        let access_token =
            cursor_state_value("cursorAuth/accessToken").ok_or_else(|| "no cursor access token in Cursor state".to_string())?;

        // The CSV export needs the session cookie, not the bearer token.
        let cookie = cursor_session_cookie(&access_token)
            .ok_or_else(|| "cursor access token has no usable subject (JWT)".to_string())?;

        let now_ms = chrono::Local::now().timestamp_millis();
        let start_ms = now_ms - 90 * 24 * 3600 * 1000;

        match fetch_cursor_csv(&cookie, start_ms, now_ms).await {
            Ok(text) => return Ok(parse_usage_csv(&text)),
            Err(e) => log::warn!("cursor csv fetch failed: {e}"),
        }

        // Try refreshing the token once on failure, then retry.
        if let Some(refresh_token) = cursor_state_value("cursorAuth/refreshToken") {
            if let Ok(new_token) = refresh_cursor_token(&refresh_token).await {
                if let Some(new_cookie) = cursor_session_cookie(&new_token) {
                    match fetch_cursor_csv(&new_cookie, start_ms, now_ms).await {
                        Ok(text) => return Ok(parse_usage_csv(&text)),
                        Err(e) => return Err(e),
                    }
                }
            }
        }
        Err("cursor export failed after token refresh attempt".to_string())
    })()
    .await;

    let stat = status("cursor", "Cursor", &result);
    let records = result.unwrap_or_default();
    (records, stat)
}

fn parse_cursor_timestamp(raw: &str) -> i64 {
    let raw = raw.trim();
    if raw.is_empty() {
        return 0;
    }
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(raw) {
        return dt.timestamp_millis();
    }
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&format!("{raw}Z")) {
        return dt.timestamp_millis();
    }
    if let Ok(date) = chrono::NaiveDate::parse_from_str(raw, "%Y-%m-%d") {
        return date
            .and_hms_opt(12, 0, 0)
            .map(|d| d.and_utc().timestamp_millis())
            .unwrap_or(0);
    }
    if let Ok(date) = chrono::NaiveDate::parse_from_str(raw, "%m/%d/%Y") {
        return date
            .and_hms_opt(12, 0, 0)
            .map(|d| d.and_utc().timestamp_millis())
            .unwrap_or(0);
    }
    0
}

fn parse_usage_csv(text: &str) -> Vec<UsageRecord> {
    let mut lines = text.lines();
    let Some(header) = lines.next() else {
        return Vec::new();
    };
    let cols: Vec<String> = header
        .split(',')
        .map(|s| s.trim_matches('"').to_lowercase())
        .collect();

    let find = |needles: &[&str]| -> Option<usize> {
        cols.iter()
            .position(|c| needles.iter().any(|n| c.contains(n)))
    };

    let (Some(i_date), Some(i_model)) = (find(&["date"]), find(&["model"])) else {
        return Vec::new();
    };
    let i_input = find(&["input (w/o cache write)"]).or_else(|| find(&["input"]));
    let i_output = find(&["output tokens"]).or_else(|| find(&["output"]));
    let i_cache_read = find(&["cache read"]);
    let i_cache_write = find(&["input (w/ cache write)"]).or_else(|| find(&["cache write"]));
    let i_cost = find(&["cost"]);
    let i_requests = find(&["request"]);

    let parse_u = |s: &str| -> u64 {
        s.trim()
            .trim_matches('"')
            .replace(',', "")
            .replace(['$', '"'], "")
            .parse::<f64>()
            .map(|f| f as u64)
            .unwrap_or(0)
    };
    let parse_f = |s: &str| -> f64 {
        s.trim()
            .trim_matches('"')
            .replace(',', "")
            .replace(['$', '"'], "")
            .parse::<f64>()
            .unwrap_or(0.0)
    };

    let mut out = Vec::new();
    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
        let get = |i: usize| -> &str { fields.get(i).copied().unwrap_or("") };

        let date = get(i_date).trim_matches('"');
        let ts_ms = parse_cursor_timestamp(date);
        if ts_ms == 0 {
            continue;
        }

        let model = get(i_model).trim_matches('"');
        let input = i_input.map(get).map(parse_u).unwrap_or(0);
        let output = i_output.map(get).map(parse_u).unwrap_or(0);
        let cache_read = i_cache_read.map(get).map(parse_u).unwrap_or(0);
        let cache_write = i_cache_write.map(get).map(parse_u).unwrap_or(0);
        let cost = i_cost.map(get).map(parse_f).unwrap_or(0.0);
        let requests = i_requests.map(get).map(parse_u).unwrap_or(1);

        if input + output + cache_read + cache_write == 0 {
            continue;
        }

        let rec = estimate_if_needed(UsageRecord {
            agent: "cursor".into(),
            sub_agent: String::new(),
            model: model.to_string(),
            provider: "cursor".into(),
            ts_ms,
            tokens_input: input,
            tokens_output: output,
            tokens_reasoning: 0,
            tokens_cache_read: cache_read,
            tokens_cache_write: cache_write,
            cost,
            project: String::new(),
            title: String::new(),
            sessions: requests.max(1),
        });
        out.push(rec);
    }
    out
}
