//! Condition expression DSL for workflow task `when` clauses.
//!
//! This module provides a custom DSL for expressing conditional execution in workflows.
//! Tasks with a `when:` field only execute when the condition evaluates to `true`.
//! If the condition is `false`, the task (and all tasks that depend on it) are skipped.
//!
//! # Syntax
//!
//! ```text
//! condition  := or_expr
//! or_expr    := and_expr ('or' and_expr)*
//! and_expr   := not_expr ('and' not_expr)*
//! not_expr   := 'not' not_expr | primary
//! primary    := '(' condition ')' | comparison
//! comparison := value op value
//! value      := task_ref | literal
//! task_ref   := 'tasks' '.' IDENT '.' 'output' ('.' IDENT)*
//! literal    := INTEGER | FLOAT | BOOL | STRING
//! op         := '==' | '!=' | '>' | '>=' | '<' | '<='
//! ```
//!
//! # Examples
//!
//! ```text
//! tasks.check_quality.output.score > 0.8
//! tasks.validate.output.is_valid == true
//! tasks.classify.output.category == "high"
//! tasks.a.output.x > 10 and tasks.b.output.y < 5
//! not tasks.validate.output.failed == true
//! (tasks.a.output.x > 0 or tasks.b.output.y > 0) and tasks.c.output.ready == true
//! ```

use std::collections::HashMap;

// ─── AST ─────────────────────────────────────────────────────────────────────

/// A parsed condition expression.
#[derive(Debug, Clone, PartialEq)]
pub enum ConditionExpr {
    Comparison {
        left: Value,
        op: CompareOp,
        right: Value,
    },
    And(Box<ConditionExpr>, Box<ConditionExpr>),
    Or(Box<ConditionExpr>, Box<ConditionExpr>),
    Not(Box<ConditionExpr>),
}

/// A value in a condition expression: either a task output reference or a literal.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// Reference to a task output: `tasks.<task>.output[.<field>...]`
    TaskRef {
        task: String,
        /// Path segments after `.output` (may be empty to reference the whole output)
        fields: Vec<String>,
    },
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum CompareOp {
    Eq,
    Ne,
    Gt,
    Ge,
    Lt,
    Le,
}

mod parser;

// ─── Public API ───────────────────────────────────────────────────────────────

/// Parse a condition expression string into an AST.
///
/// Returns an error string describing the parse failure if the input is invalid.
pub fn parse_condition(input: &str) -> Result<ConditionExpr, String> {
    let tokens = parser::tokenize(input)?;
    if tokens.is_empty() {
        return Err("condition expression is empty".to_string());
    }
    let mut p = parser::Parser::new(tokens);
    let expr = p.parse_condition()?;
    if !p.at_end() {
        let remaining: Vec<_> = p.tokens[p.pos..].iter().collect();
        return Err(format!(
            "unexpected tokens after condition: {:?}",
            remaining
        ));
    }
    Ok(expr)
}

/// Collect all task output references from a condition expression.
///
/// Returns a list of `(task_name, field_path)` pairs.
pub fn collect_task_refs(expr: &ConditionExpr) -> Vec<(&str, &[String])> {
    let mut refs = Vec::new();
    collect_task_refs_inner(expr, &mut refs);
    refs
}

fn collect_task_refs_inner<'a>(expr: &'a ConditionExpr, refs: &mut Vec<(&'a str, &'a [String])>) {
    match expr {
        ConditionExpr::Comparison { left, right, .. } => {
            collect_value_ref(left, refs);
            collect_value_ref(right, refs);
        }
        ConditionExpr::And(a, b) | ConditionExpr::Or(a, b) => {
            collect_task_refs_inner(a, refs);
            collect_task_refs_inner(b, refs);
        }
        ConditionExpr::Not(e) => collect_task_refs_inner(e, refs),
    }
}

fn collect_value_ref<'a>(value: &'a Value, refs: &mut Vec<(&'a str, &'a [String])>) {
    if let Value::TaskRef { task, fields } = value {
        refs.push((task.as_str(), fields.as_slice()));
    }
}

/// Evaluate a condition expression against resolved task outputs.
///
/// `outputs` maps task names to their complete output JSON values.
///
/// Returns `Ok(true)` if the condition is satisfied, `Ok(false)` if not, or an
/// error string if evaluation fails (e.g., missing output, type mismatch).
pub fn evaluate_condition(
    expr: &ConditionExpr,
    outputs: &HashMap<String, serde_json::Value>,
) -> Result<bool, String> {
    match expr {
        ConditionExpr::And(a, b) => {
            Ok(evaluate_condition(a, outputs)? && evaluate_condition(b, outputs)?)
        }
        ConditionExpr::Or(a, b) => {
            Ok(evaluate_condition(a, outputs)? || evaluate_condition(b, outputs)?)
        }
        ConditionExpr::Not(e) => Ok(!evaluate_condition(e, outputs)?),
        ConditionExpr::Comparison { left, op, right } => {
            let lv = resolve_value(left, outputs)?;
            let rv = resolve_value(right, outputs)?;
            compare_values(&lv, op, &rv)
        }
    }
}

fn resolve_value(
    value: &Value,
    outputs: &HashMap<String, serde_json::Value>,
) -> Result<serde_json::Value, String> {
    match value {
        Value::TaskRef { task, fields } => {
            let output = outputs.get(task.as_str()).ok_or_else(|| {
                format!(
                    "task '{}' output is not available for condition evaluation",
                    task
                )
            })?;
            let mut current = output;
            for field in fields {
                let obj = current.as_object().ok_or_else(|| {
                    format!(
                        "cannot access field '{}' on non-object output of task '{}'",
                        field, task
                    )
                })?;
                current = obj.get(field.as_str()).ok_or_else(|| {
                    format!("field '{}' not found in output of task '{}'", field, task)
                })?;
            }
            Ok(current.clone())
        }
        Value::Int(n) => Ok(serde_json::Value::Number((*n).into())),
        Value::Float(f) => {
            let n = serde_json::Number::from_f64(*f)
                .ok_or_else(|| format!("invalid float value: {}", f))?;
            Ok(serde_json::Value::Number(n))
        }
        Value::Bool(b) => Ok(serde_json::Value::Bool(*b)),
        Value::Str(s) => Ok(serde_json::Value::String(s.clone())),
    }
}

fn compare_values(
    left: &serde_json::Value,
    op: &CompareOp,
    right: &serde_json::Value,
) -> Result<bool, String> {
    match op {
        CompareOp::Eq => Ok(json_eq(left, right)),
        CompareOp::Ne => Ok(!json_eq(left, right)),
        CompareOp::Gt => Ok(numeric_cmp(left, right)? > 0.0),
        CompareOp::Ge => Ok(numeric_cmp(left, right)? >= 0.0),
        CompareOp::Lt => Ok(numeric_cmp(left, right)? < 0.0),
        CompareOp::Le => Ok(numeric_cmp(left, right)? <= 0.0),
    }
}

/// Equality that handles int/float cross-comparison.
fn json_eq(a: &serde_json::Value, b: &serde_json::Value) -> bool {
    // If both are numbers, compare numerically (handles int == float)
    if let (Some(la), Some(ra)) = (a.as_f64(), b.as_f64()) {
        return la == ra;
    }
    a == b
}

/// Returns `left - right` as f64 for ordering comparisons.
/// Only works on numeric JSON values.
fn numeric_cmp(left: &serde_json::Value, right: &serde_json::Value) -> Result<f64, String> {
    let l = left
        .as_f64()
        .ok_or_else(|| format!("ordered comparison requires numeric values, got {:?}", left))?;
    let r = right.as_f64().ok_or_else(|| {
        format!(
            "ordered comparison requires numeric values, got {:?}",
            right
        )
    })?;
    Ok(l - r)
}

#[cfg(test)]
mod tests;
