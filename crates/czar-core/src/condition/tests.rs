use super::*;

fn outputs(pairs: &[(&str, serde_json::Value)]) -> HashMap<String, serde_json::Value> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect()
}

#[test]
fn test_parse_simple_comparison() {
    let expr = parse_condition("tasks.check.output.score > 0.8").unwrap();
    assert!(matches!(
        expr,
        ConditionExpr::Comparison {
            left: Value::TaskRef { .. },
            op: CompareOp::Gt,
            right: Value::Float(_),
        }
    ));
}

#[test]
fn test_parse_equality() {
    let expr = parse_condition("tasks.validate.output.is_valid == true").unwrap();
    assert!(matches!(
        expr,
        ConditionExpr::Comparison {
            left: Value::TaskRef { .. },
            op: CompareOp::Eq,
            right: Value::Bool(true),
        }
    ));
}

#[test]
fn test_parse_string_comparison() {
    let expr = parse_condition(r#"tasks.classify.output.category == "high""#).unwrap();
    assert!(matches!(
        expr,
        ConditionExpr::Comparison {
            left: Value::TaskRef { .. },
            op: CompareOp::Eq,
            right: Value::Str(ref s),
        } if s == "high"
    ));
}

#[test]
fn test_parse_and() {
    let expr = parse_condition("tasks.a.output.x > 10 and tasks.b.output.y < 5").unwrap();
    assert!(matches!(expr, ConditionExpr::And(_, _)));
}

#[test]
fn test_parse_or() {
    let expr = parse_condition("tasks.a.output.x > 0 or tasks.b.output.y > 0").unwrap();
    assert!(matches!(expr, ConditionExpr::Or(_, _)));
}

#[test]
fn test_parse_not() {
    let expr = parse_condition("not tasks.check.output.failed == true").unwrap();
    assert!(matches!(expr, ConditionExpr::Not(_)));
}

#[test]
fn test_parse_parenthesized() {
    let expr = parse_condition(
        "(tasks.a.output.x > 0 or tasks.b.output.y > 0) and tasks.c.output.ready == true",
    )
    .unwrap();
    assert!(matches!(expr, ConditionExpr::And(_, _)));
}

#[test]
fn test_parse_negative_number() {
    let expr = parse_condition("tasks.a.output.delta > -10").unwrap();
    assert!(matches!(
        expr,
        ConditionExpr::Comparison {
            right: Value::Int(-10),
            ..
        }
    ));
}

#[test]
fn test_parse_ne() {
    let expr = parse_condition("tasks.a.output.status != \"error\"").unwrap();
    assert!(matches!(
        expr,
        ConditionExpr::Comparison {
            op: CompareOp::Ne,
            ..
        }
    ));
}

#[test]
fn test_parse_deep_field() {
    let expr = parse_condition("tasks.a.output.result.score > 0").unwrap();
    if let ConditionExpr::Comparison {
        left: Value::TaskRef { task, fields },
        ..
    } = expr
    {
        assert_eq!(task, "a");
        assert_eq!(fields, vec!["result", "score"]);
    } else {
        panic!("expected comparison with TaskRef");
    }
}

#[test]
fn test_parse_error_empty() {
    assert!(parse_condition("").is_err());
}

#[test]
fn test_parse_error_bad_op() {
    assert!(parse_condition("tasks.a.output.x + 5").is_err());
}

#[test]
fn test_parse_error_unterminated_string() {
    assert!(parse_condition(r#"tasks.a.output.x == "hello"#).is_err());
}

#[test]
fn test_evaluate_gt_true() {
    let expr = parse_condition("tasks.check.output.score > 0.8").unwrap();
    let out = outputs(&[("check", serde_json::json!({"score": 0.9}))]);
    assert_eq!(evaluate_condition(&expr, &out), Ok(true));
}

#[test]
fn test_evaluate_gt_false() {
    let expr = parse_condition("tasks.check.output.score > 0.8").unwrap();
    let out = outputs(&[("check", serde_json::json!({"score": 0.5}))]);
    assert_eq!(evaluate_condition(&expr, &out), Ok(false));
}

#[test]
fn test_evaluate_eq_bool() {
    let expr = parse_condition("tasks.validate.output.is_valid == true").unwrap();
    let out = outputs(&[("validate", serde_json::json!({"is_valid": true}))]);
    assert_eq!(evaluate_condition(&expr, &out), Ok(true));
}

#[test]
fn test_evaluate_eq_string() {
    let expr = parse_condition(r#"tasks.classify.output.category == "high""#).unwrap();
    let out = outputs(&[("classify", serde_json::json!({"category": "high"}))]);
    assert_eq!(evaluate_condition(&expr, &out), Ok(true));

    let out2 = outputs(&[("classify", serde_json::json!({"category": "low"}))]);
    assert_eq!(evaluate_condition(&expr, &out2), Ok(false));
}

#[test]
fn test_evaluate_and() {
    let expr = parse_condition("tasks.a.output.x > 5 and tasks.b.output.y < 10").unwrap();
    let out = outputs(&[
        ("a", serde_json::json!({"x": 8})),
        ("b", serde_json::json!({"y": 3})),
    ]);
    assert_eq!(evaluate_condition(&expr, &out), Ok(true));

    let out2 = outputs(&[
        ("a", serde_json::json!({"x": 2})),
        ("b", serde_json::json!({"y": 3})),
    ]);
    assert_eq!(evaluate_condition(&expr, &out2), Ok(false));
}

#[test]
fn test_evaluate_or() {
    let expr = parse_condition("tasks.a.output.x > 5 or tasks.b.output.y < 10").unwrap();
    let out = outputs(&[
        ("a", serde_json::json!({"x": 2})),
        ("b", serde_json::json!({"y": 3})),
    ]);
    assert_eq!(evaluate_condition(&expr, &out), Ok(true));
}

#[test]
fn test_evaluate_not() {
    let expr = parse_condition("not tasks.a.output.failed == true").unwrap();
    let out = outputs(&[("a", serde_json::json!({"failed": false}))]);
    assert_eq!(evaluate_condition(&expr, &out), Ok(true));
}

#[test]
fn test_evaluate_int_float_cross_comparison() {
    let expr = parse_condition("tasks.a.output.count > 0.0").unwrap();
    let out = outputs(&[("a", serde_json::json!({"count": 5}))]);
    assert_eq!(evaluate_condition(&expr, &out), Ok(true));
}

#[test]
fn test_evaluate_missing_task_output() {
    let expr = parse_condition("tasks.missing.output.x > 0").unwrap();
    let out = outputs(&[]);
    assert!(evaluate_condition(&expr, &out).is_err());
}

#[test]
fn test_collect_task_refs() {
    let expr = parse_condition("tasks.a.output.x > 5 and tasks.b.output.score < 0.8").unwrap();
    let refs = collect_task_refs(&expr);
    let task_names: Vec<&str> = refs.iter().map(|(t, _)| *t).collect();
    assert!(task_names.contains(&"a"));
    assert!(task_names.contains(&"b"));
}

#[test]
fn test_le_ge_operators() {
    let expr = parse_condition("tasks.a.output.x >= 5").unwrap();
    let out = outputs(&[("a", serde_json::json!({"x": 5}))]);
    assert_eq!(evaluate_condition(&expr, &out), Ok(true));

    let expr2 = parse_condition("tasks.a.output.x <= 4").unwrap();
    assert_eq!(evaluate_condition(&expr2, &out), Ok(false));
}
