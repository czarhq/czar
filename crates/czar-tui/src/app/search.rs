use crate::api::Run;

/// Format an ISO8601 timestamp as "MM-DD HH:MM" for display and search.
pub fn fmt_datetime_short(s: &str) -> String {
    let md = s.get(5..10).unwrap_or("");
    let hm = s.get(11..16).unwrap_or("");
    format!("{md} {hm}")
}

/// Check if a run matches a fuzzy query across workflow name, run ID, status, and time.
pub fn run_fuzzy_matches(r: &Run, query: &str) -> bool {
    let ts = fmt_datetime_short(&r.created_at);
    fuzzy_match(&r.workflow, query)
        || fuzzy_match(&r.id[..r.id.len().min(8)], query)
        || fuzzy_match(&r.status, query)
        || fuzzy_match(&ts, query)
}

/// Case-insensitive subsequence fuzzy match.
pub fn fuzzy_match(text: &str, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    let text_lc: Vec<char> = text.to_lowercase().chars().collect();
    let query_lc: Vec<char> = query.to_lowercase().chars().collect();
    let mut qi = 0;
    for tc in &text_lc {
        if qi < query_lc.len() && *tc == query_lc[qi] {
            qi += 1;
        }
    }
    qi == query_lc.len()
}

/// Returns char-level positions in `text` that match `query` (first-left greedy).
pub fn fuzzy_match_positions(text: &str, query: &str) -> Option<Vec<usize>> {
    if query.is_empty() {
        return Some(vec![]);
    }
    let text_lc: Vec<char> = text.to_lowercase().chars().collect();
    let query_lc: Vec<char> = query.to_lowercase().chars().collect();
    let mut positions = Vec::new();
    let mut qi = 0;
    for (i, tc) in text_lc.iter().enumerate() {
        if qi < query_lc.len() && *tc == query_lc[qi] {
            positions.push(i);
            qi += 1;
        }
    }
    if qi == query_lc.len() {
        Some(positions)
    } else {
        None
    }
}
