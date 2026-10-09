use super::*;

fn graph() -> Vec<(String, Vec<String>)> {
    vec![
        ("a".into(), vec![]),
        ("b".into(), vec!["a".into()]),
        ("c".into(), vec!["b".into()]),
        ("d".into(), vec!["b".into()]),
    ]
}

fn expand(selectors: &[&str]) -> Result<ExpandedSelectors> {
    let g = graph();
    expand_selectors(
        &g,
        &selectors.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
    )
}

fn sorted(v: Vec<String>) -> Vec<String> {
    let mut v = v;
    v.sort();
    v
}

#[test]
fn parse_exact_task() {
    let spec = parse_task_selection_spec("b").unwrap();
    assert_eq!(spec.seed, "b");
    assert!(spec.upstream.is_none());
    assert!(spec.downstream.is_none());
}

#[test]
fn parse_upstream_unlimited() {
    let spec = parse_task_selection_spec("+b").unwrap();
    assert_eq!(spec.seed, "b");
    assert!(matches!(spec.upstream, Some(SelectionDepth::Unlimited)));
    assert!(spec.downstream.is_none());
}

#[test]
fn parse_downstream_unlimited() {
    let spec = parse_task_selection_spec("b+").unwrap();
    assert_eq!(spec.seed, "b");
    assert!(spec.upstream.is_none());
    assert!(matches!(spec.downstream, Some(SelectionDepth::Unlimited)));
}

#[test]
fn parse_upstream_limited() {
    let spec = parse_task_selection_spec("2+b").unwrap();
    assert_eq!(spec.seed, "b");
    assert!(matches!(spec.upstream, Some(SelectionDepth::Limited(2))));
    assert!(spec.downstream.is_none());
}

#[test]
fn parse_downstream_limited() {
    let spec = parse_task_selection_spec("b+3").unwrap();
    assert_eq!(spec.seed, "b");
    assert!(spec.upstream.is_none());
    assert!(matches!(spec.downstream, Some(SelectionDepth::Limited(3))));
}

#[test]
fn parse_bidirectional_unlimited() {
    let spec = parse_task_selection_spec("+b+").unwrap();
    assert_eq!(spec.seed, "b");
    assert!(matches!(spec.upstream, Some(SelectionDepth::Unlimited)));
    assert!(matches!(spec.downstream, Some(SelectionDepth::Unlimited)));
}

#[test]
fn parse_bidirectional_mixed_depth() {
    let spec = parse_task_selection_spec("2+b+1").unwrap();
    assert_eq!(spec.seed, "b");
    assert!(matches!(spec.upstream, Some(SelectionDepth::Limited(2))));
    assert!(matches!(spec.downstream, Some(SelectionDepth::Limited(1))));
}

#[test]
fn parse_empty_selector_errors() {
    assert!(parse_task_selection_spec("").is_err());
    assert!(parse_task_selection_spec("   ").is_err());
}

#[test]
fn parse_missing_task_name_errors() {
    assert!(parse_task_selection_spec("+").is_err());
    assert!(parse_task_selection_spec("++").is_err());
}

#[test]
fn parse_too_many_plus_errors() {
    assert!(parse_task_selection_spec("1+b+2+3").is_err());
}

#[test]
fn parse_legacy_wildcard_syntax_errors() {
    assert!(parse_task_selection_spec("*b").is_err());
    assert!(parse_task_selection_spec("b*").is_err());
    assert!(parse_task_selection_spec("?b").is_err());
    assert!(parse_task_selection_spec("b?").is_err());
}

#[test]
fn expand_exact_task() {
    let r = expand(&["b"]).unwrap();
    assert_eq!(r.requested_tasks, vec!["b"]);
    assert_eq!(r.resolved_tasks, vec!["b"]);
}

#[test]
fn expand_upstream_unlimited() {
    let r = expand(&["+c"]).unwrap();
    assert_eq!(r.requested_tasks, vec!["c"]);
    assert_eq!(sorted(r.resolved_tasks), vec!["a", "b", "c"]);
}

#[test]
fn expand_downstream_unlimited() {
    let r = expand(&["b+"]).unwrap();
    assert_eq!(r.requested_tasks, vec!["b"]);
    assert_eq!(sorted(r.resolved_tasks), vec!["b", "c", "d"]);
}

#[test]
fn expand_upstream_depth_1() {
    let r = expand(&["1+c"]).unwrap();
    assert_eq!(sorted(r.resolved_tasks), vec!["b", "c"]);
}

#[test]
fn expand_downstream_depth_1() {
    let r = expand(&["b+1"]).unwrap();
    assert_eq!(sorted(r.resolved_tasks), vec!["b", "c", "d"]);
}

#[test]
fn expand_multiple_selectors_are_additive() {
    let r = expand(&["a", "c"]).unwrap();
    assert_eq!(sorted(r.requested_tasks), vec!["a", "c"]);
    assert_eq!(sorted(r.resolved_tasks), vec!["a", "c"]);
}

#[test]
fn expand_unknown_task_errors() {
    assert!(expand(&["unknown"]).is_err());
}

#[test]
fn expand_empty_selectors_returns_empty() {
    let r = expand(&[]).unwrap();
    assert!(r.requested_tasks.is_empty());
    assert!(r.resolved_tasks.is_empty());
}
