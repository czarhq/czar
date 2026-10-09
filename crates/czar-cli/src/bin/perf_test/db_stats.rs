use anyhow::Result;
use sqlx::PgPool;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};

/// A snapshot of pg_stat_statements for a single query at a point in time.
#[derive(Debug, Clone, Default)]
pub struct QueryStat {
    pub query: String,
    pub calls: i64,
    pub total_exec_time_ms: f64,
    pub mean_exec_time_ms: f64,
    pub rows: i64,
}

/// Check whether pg_stat_statements is available and enabled.
pub async fn is_pg_stat_statements_available(pool: &PgPool) -> bool {
    let result: Result<(i64,), _> =
        sqlx::query_as("SELECT COUNT(*) FROM pg_extension WHERE extname = 'pg_stat_statements'")
            .fetch_one(pool)
            .await;
    matches!(result, Ok((count,)) if count > 0)
}

/// Reset pg_stat_statements counters for a clean baseline before the benchmark run.
pub async fn reset_pg_stat_statements(pool: &PgPool) -> Result<()> {
    sqlx::query("SELECT pg_stat_statements_reset()")
        .execute(pool)
        .await?;
    Ok(())
}

/// Query the top N queries by total execution time, excluding internal perf-test queries.
pub async fn query_top_statements(pool: &PgPool, limit: i64) -> Result<Vec<QueryStat>> {
    // Exclude pg_stat_statements_reset itself and our own monitoring queries
    let rows: Vec<(String, i64, f64, f64, i64)> = sqlx::query_as(
        r#"
        SELECT
            LEFT(query, 120) AS query,
            calls,
            total_exec_time AS total_exec_time_ms,
            mean_exec_time AS mean_exec_time_ms,
            rows
        FROM pg_stat_statements
        WHERE query NOT LIKE '%pg_stat_statements%'
          AND query NOT LIKE '%pg_extension%'
          AND query NOT LIKE '%tasks GROUP BY status%'
          AND query NOT LIKE '%finished_at%'
          AND query NOT LIKE '%pg_stat_statements_reset%'
        ORDER BY total_exec_time DESC
        LIMIT $1
        "#,
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(query, calls, total_exec_time_ms, mean_exec_time_ms, rows)| QueryStat {
                query: query.trim().to_string(),
                calls,
                total_exec_time_ms,
                mean_exec_time_ms,
                rows,
            },
        )
        .collect())
}

/// Format pg_stat_statements results as display lines.
pub fn format_db_stats(stats: &[QueryStat]) -> Vec<String> {
    if stats.is_empty() {
        return vec!["  (no query stats available)".to_string()];
    }

    let total_time_ms: f64 = stats.iter().map(|s| s.total_exec_time_ms).sum();
    let total_calls: i64 = stats.iter().map(|s| s.calls).sum();

    let mut lines = vec![
        format!(
            "  Total tracked: {total_calls} calls, {:.1}ms total DB time",
            total_time_ms
        ),
        String::new(),
        format!(
            "  {:<8} {:>8} {:>10} {:>8}  {}",
            "Calls", "Total ms", "Mean ms", "Rows", "Query (truncated)"
        ),
        format!("  {}", "-".repeat(90)),
    ];

    for stat in stats {
        // Normalize whitespace in query for display
        let query_preview: String = stat.query.split_whitespace().collect::<Vec<_>>().join(" ");
        let query_preview = if query_preview.len() > 60 {
            format!("{}...", &query_preview[..57])
        } else {
            query_preview
        };

        lines.push(format!(
            "  {:<8} {:>8.1} {:>10.2} {:>8}  {}",
            stat.calls, stat.total_exec_time_ms, stat.mean_exec_time_ms, stat.rows, query_preview,
        ));
    }

    lines
}

// ── DB_TRACE parsing (from CZAR_DB_TRACE=1 scheduler output) ─────────────────

#[derive(Debug, Clone, Default)]
pub struct MethodStats {
    pub calls: u64,
    pub total_ms: f64,
}

/// Parse `DB_TRACE: {json}` lines from the scheduler log.
/// Each line is a cumulative snapshot from process start; we take the **last**
/// one so we get complete data regardless of when the process is killed.
pub fn parse_db_trace_log(log_path: &str) -> HashMap<String, MethodStats> {
    let Ok(file) = std::fs::File::open(log_path) else {
        return HashMap::new();
    };
    let mut last: Option<HashMap<String, MethodStats>> = None;
    for line in BufReader::new(file).lines().map_while(Result::ok) {
        let Some(json) = line.strip_prefix("DB_TRACE: ") else {
            continue;
        };
        let Ok(snapshot) = serde_json::from_str::<HashMap<String, serde_json::Value>>(json) else {
            continue;
        };
        let mut totals: HashMap<String, MethodStats> = HashMap::new();
        for (method, val) in snapshot {
            let calls = val["calls"].as_u64().unwrap_or(0);
            let total_ms = val["total_ms"].as_f64().unwrap_or(0.0);
            totals.insert(method, MethodStats { calls, total_ms });
        }
        last = Some(totals);
    }
    last.unwrap_or_default()
}

/// Format DB call profile as display lines, sorted by total time descending.
pub fn format_db_profile(stats: &HashMap<String, MethodStats>) -> Vec<String> {
    if stats.is_empty() {
        return vec!["  (no DB trace data — is CZAR_DB_TRACE=1 set?)".to_string()];
    }

    let mut rows: Vec<(&String, &MethodStats)> = stats.iter().collect();
    rows.sort_by(|a, b| b.1.total_ms.partial_cmp(&a.1.total_ms).unwrap());

    let total_calls: u64 = rows.iter().map(|(_, s)| s.calls).sum();
    let total_ms: f64 = rows.iter().map(|(_, s)| s.total_ms).sum();

    let mut lines = vec![
        format!(
            "  Total: {} calls, {:.1}ms cumulative DB time",
            total_calls, total_ms
        ),
        String::new(),
        format!(
            "  {:<40} {:>8} {:>10} {:>9}",
            "Method", "Calls", "Total ms", "Mean ms"
        ),
        format!("  {}", "-".repeat(72)),
    ];

    for (method, s) in &rows {
        let mean_ms = if s.calls > 0 {
            s.total_ms / s.calls as f64
        } else {
            0.0
        };
        lines.push(format!(
            "  {:<40} {:>8} {:>10.1} {:>9.3}",
            method, s.calls, s.total_ms, mean_ms,
        ));
    }

    lines
}
