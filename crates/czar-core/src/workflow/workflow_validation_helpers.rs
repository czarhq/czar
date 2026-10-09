use super::*;
use chrono::{DateTime, NaiveDate, NaiveDateTime};

const VALID_PRIMITIVES: &[&str] = &[
    "str",
    "int",
    "float",
    "bool",
    "date",
    "datetime_notz",
    "datetime_tz",
    "datetime_utc",
];

/// Validate that a dotted field path exists within a type schema.
///
/// `schema` is the output_type value (string, object, array, or type alias).
/// `fields` is the remaining field segments to traverse.
pub(super) fn validate_field_path(
    schema: &serde_json::Value,
    fields: &[String],
    task_name: &str,
    aliases: &BTreeMap<String, serde_json::Value>,
) -> Result<(), String> {
    let expanded = expand_type_schema(schema, aliases)?;
    validate_field_path_expanded(&expanded, fields, task_name)
}

pub(super) fn resolve_field_schema(
    schema: &serde_json::Value,
    fields: &[String],
    task_name: &str,
    aliases: &BTreeMap<String, serde_json::Value>,
) -> Result<serde_json::Value, String> {
    let expanded = expand_type_schema(schema, aliases)?;
    resolve_field_schema_expanded(&expanded, fields, task_name)
}

/// Expand a schema into its canonical representation:
/// - aliases are resolved recursively
/// - string list shorthand like `list[str]` or `str[]` becomes `["str"]`
/// - objects and arrays are recursively normalized
pub(crate) fn expand_type_schema(
    value: &serde_json::Value,
    aliases: &BTreeMap<String, serde_json::Value>,
) -> Result<serde_json::Value, String> {
    expand_type_schema_impl(value, aliases, &mut Vec::new())
}

pub(crate) fn validate_value_against_type_schema(
    value: &serde_json::Value,
    schema: &serde_json::Value,
    aliases: &BTreeMap<String, serde_json::Value>,
) -> Result<(), String> {
    let expanded = expand_type_schema(schema, aliases)?;
    validate_value_against_expanded_schema(value, &expanded, "$")
}

/// Validate that a type schema value uses only valid type expressions:
/// - String leaf values must be a valid primitive type name or alias
/// - Arrays must contain exactly one element (the element type)
/// - Objects recurse into their values
/// - String array shorthand like `list[str]`, `Array[types.Row]`, and `str[]` is supported
pub(super) fn validate_type_schema(
    value: &serde_json::Value,
    task_name: &str,
    path: &str,
    aliases: &BTreeMap<String, serde_json::Value>,
    _alias_stack: &mut Vec<String>,
) -> Result<(), WorkflowValidationError> {
    expand_type_schema(value, aliases)
        .map(|_| ())
        .map_err(|reason| WorkflowValidationError::InvalidTypeSchema {
            task: task_name.to_string(),
            path: path.to_string(),
            reason,
        })
}

fn validate_field_path_expanded(
    schema: &serde_json::Value,
    fields: &[String],
    task_name: &str,
) -> Result<(), String> {
    if fields.is_empty() {
        return Ok(());
    }

    match schema {
        serde_json::Value::String(s) => Err(format!(
            "cannot access field '{}' on primitive type '{}' (output of task '{}')",
            fields[0], s, task_name
        )),
        serde_json::Value::Object(map) => {
            let field = &fields[0];
            if let Some(field_schema) = map.get(field.as_str()) {
                validate_field_path_expanded(field_schema, &fields[1..], task_name)
            } else {
                let available: Vec<&str> = map.keys().map(|k| k.as_str()).collect();
                Err(format!(
                    "field '{}' does not exist in output of task '{}'; available fields: [{}]",
                    field,
                    task_name,
                    available.join(", ")
                ))
            }
        }
        serde_json::Value::Array(_) => Err(format!(
            "cannot access named field '{}' on array type (output of task '{}')",
            fields[0], task_name
        )),
        other => Err(format!(
            "unexpected type schema value {:?} for task '{}'",
            other, task_name
        )),
    }
}

fn validate_value_against_expanded_schema(
    value: &serde_json::Value,
    schema: &serde_json::Value,
    path: &str,
) -> Result<(), String> {
    match schema {
        serde_json::Value::String(primitive) => validate_primitive_value(value, primitive, path),
        serde_json::Value::Object(map) => {
            let Some(obj) = value.as_object() else {
                return Err(format!(
                    "{} expected object, got {}",
                    path,
                    json_type_name(value)
                ));
            };
            for (field, field_schema) in map {
                let field_value = obj
                    .get(field)
                    .ok_or_else(|| format!("{}.{} is missing", path, field))?;
                validate_value_against_expanded_schema(
                    field_value,
                    field_schema,
                    &format!("{}.{}", path, field),
                )?;
            }
            Ok(())
        }
        serde_json::Value::Array(items) => {
            let item_schema = items
                .first()
                .ok_or_else(|| format!("{} expected array item schema", path))?;
            let Some(arr) = value.as_array() else {
                return Err(format!(
                    "{} expected array, got {}",
                    path,
                    json_type_name(value)
                ));
            };
            for (index, item) in arr.iter().enumerate() {
                validate_value_against_expanded_schema(
                    item,
                    item_schema,
                    &format!("{}[{}]", path, index),
                )?;
            }
            Ok(())
        }
        other => Err(format!(
            "{} uses unsupported expanded schema {:?}",
            path, other
        )),
    }
}

fn validate_primitive_value(
    value: &serde_json::Value,
    primitive: &str,
    path: &str,
) -> Result<(), String> {
    match primitive {
        "str" => {
            if value.is_string() {
                Ok(())
            } else {
                Err(format!(
                    "{} expected str, got {}",
                    path,
                    json_type_name(value)
                ))
            }
        }
        "int" => {
            if value.as_i64().is_some() || value.as_u64().is_some() {
                Ok(())
            } else {
                Err(format!(
                    "{} expected int, got {}",
                    path,
                    json_type_name(value)
                ))
            }
        }
        "float" => {
            if value.is_number() {
                Ok(())
            } else {
                Err(format!(
                    "{} expected float, got {}",
                    path,
                    json_type_name(value)
                ))
            }
        }
        "bool" => {
            if value.is_boolean() {
                Ok(())
            } else {
                Err(format!(
                    "{} expected bool, got {}",
                    path,
                    json_type_name(value)
                ))
            }
        }
        "date" => validate_string_format(value, path, primitive, |raw| {
            NaiveDate::parse_from_str(raw, "%Y-%m-%d")
                .map(|_| ())
                .map_err(|_| raw.to_string())
        }),
        "datetime_notz" => validate_string_format(value, path, primitive, parse_datetime_notz),
        "datetime_tz" => validate_string_format(value, path, primitive, |raw| {
            DateTime::parse_from_rfc3339(raw)
                .map(|_| ())
                .map_err(|_| raw.to_string())
        }),
        "datetime_utc" => validate_string_format(value, path, primitive, |raw| {
            let parsed = DateTime::parse_from_rfc3339(raw).map_err(|_| raw.to_string())?;
            if parsed.offset().local_minus_utc() != 0 || !raw.ends_with('Z') {
                return Err(raw.to_string());
            }
            Ok(())
        }),
        other => Err(format!("{} references unknown primitive '{}'", path, other)),
    }
}

fn validate_string_format<F>(
    value: &serde_json::Value,
    path: &str,
    type_name: &str,
    parser: F,
) -> Result<(), String>
where
    F: FnOnce(&str) -> Result<(), String>,
{
    let Some(raw) = value.as_str() else {
        return Err(format!(
            "{} expected {}, got {}",
            path,
            type_name,
            json_type_name(value)
        ));
    };
    parser(raw).map_err(|_| format!("{} expected {}, got {}", path, type_name, raw))
}

fn parse_datetime_notz(raw: &str) -> Result<(), String> {
    NaiveDateTime::parse_from_str(raw, "%Y-%m-%dT%H:%M:%S%.f")
        .or_else(|_| NaiveDateTime::parse_from_str(raw, "%Y-%m-%dT%H:%M:%S"))
        .map(|_| ())
        .map_err(|_| raw.to_string())
}

fn json_type_name(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "bool",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

fn resolve_field_schema_expanded(
    schema: &serde_json::Value,
    fields: &[String],
    task_name: &str,
) -> Result<serde_json::Value, String> {
    if fields.is_empty() {
        return Ok(schema.clone());
    }

    match schema {
        serde_json::Value::String(s) => Err(format!(
            "cannot access field '{}' on primitive type '{}' (output of task '{}')",
            fields[0], s, task_name
        )),
        serde_json::Value::Object(map) => {
            let field = &fields[0];
            if let Some(field_schema) = map.get(field.as_str()) {
                resolve_field_schema_expanded(field_schema, &fields[1..], task_name)
            } else {
                let available: Vec<&str> = map.keys().map(|k| k.as_str()).collect();
                Err(format!(
                    "field '{}' does not exist in output of task '{}'; available fields: [{}]",
                    field,
                    task_name,
                    available.join(", ")
                ))
            }
        }
        serde_json::Value::Array(_) => Err(format!(
            "cannot access named field '{}' on array type (output of task '{}')",
            fields[0], task_name
        )),
        other => Err(format!(
            "unexpected type schema value {:?} for task '{}'",
            other, task_name
        )),
    }
}

fn expand_type_schema_impl(
    value: &serde_json::Value,
    aliases: &BTreeMap<String, serde_json::Value>,
    alias_stack: &mut Vec<String>,
) -> Result<serde_json::Value, String> {
    match value {
        serde_json::Value::String(raw) => expand_string_type_schema(raw, aliases, alias_stack),
        serde_json::Value::Array(arr) => {
            if arr.len() != 1 {
                return Err(format!(
                    "array type must have exactly one element (the element type), got {}",
                    arr.len()
                ));
            }
            let item = expand_type_schema_impl(&arr[0], aliases, alias_stack)?;
            Ok(serde_json::Value::Array(vec![item]))
        }
        serde_json::Value::Object(map) => {
            let mut expanded = serde_json::Map::with_capacity(map.len());
            for (key, value) in map {
                expanded.insert(
                    key.clone(),
                    expand_type_schema_impl(value, aliases, alias_stack)?,
                );
            }
            Ok(serde_json::Value::Object(expanded))
        }
        other => Err(format!(
            "expected a type name (string), array type, or object, got {:?}",
            other
        )),
    }
}

fn expand_string_type_schema(
    raw: &str,
    aliases: &BTreeMap<String, serde_json::Value>,
    alias_stack: &mut Vec<String>,
) -> Result<serde_json::Value, String> {
    let raw = raw.trim();

    if let Some(element_expr) = parse_array_type_expr(raw)? {
        return Ok(serde_json::Value::Array(vec![expand_string_type_schema(
            &element_expr,
            aliases,
            alias_stack,
        )?]));
    }

    if VALID_PRIMITIVES.contains(&raw) {
        return Ok(serde_json::Value::String(raw.to_string()));
    }

    if let Some(alias_name) = raw.strip_prefix("types.") {
        let alias_schema = aliases
            .get(alias_name)
            .ok_or_else(|| format!("unknown type alias '{}'", raw))?;
        if alias_stack.iter().any(|existing| existing == alias_name) {
            let mut chain = alias_stack.clone();
            chain.push(alias_name.to_string());
            return Err(format!(
                "recursive type alias chain: {}",
                chain.join(" -> ")
            ));
        }
        alias_stack.push(alias_name.to_string());
        let result = expand_type_schema_impl(alias_schema, aliases, alias_stack);
        alias_stack.pop();
        return result;
    }

    Err(format!(
        "unknown type '{}'. Valid types: {}",
        raw,
        VALID_PRIMITIVES.join(", ")
    ))
}

fn parse_array_type_expr(raw: &str) -> Result<Option<String>, String> {
    let raw = raw.trim();

    if matches!(raw, "list" | "List" | "array" | "Array") {
        return Err(
            "array type requires an element type, for example `list[str]` or `str[]`".to_string(),
        );
    }

    if let Some(inner) = raw.strip_suffix("[]") {
        let inner = inner.trim();
        if inner.is_empty() {
            return Err(
                "array type requires an element type, for example `list[str]` or `str[]`"
                    .to_string(),
            );
        }
        return Ok(Some(inner.to_string()));
    }

    for prefix in ["list", "List", "array", "Array"] {
        let Some(rest) = raw.strip_prefix(prefix) else {
            continue;
        };
        if !rest.starts_with('[') || !rest.ends_with(']') {
            continue;
        }
        let inner = &rest[1..rest.len() - 1];
        let inner = inner.trim();
        if inner.is_empty() {
            return Err(
                "array type requires an element type, for example `list[str]` or `str[]`"
                    .to_string(),
            );
        }
        return Ok(Some(inner.to_string()));
    }

    Ok(None)
}

pub(super) fn detect_cycle(workflow: &Workflow) -> Option<String> {
    fn visit(workflow: &Workflow, name: &str, visiting: &mut HashMap<String, bool>) -> bool {
        match visiting.get(name) {
            Some(true) => return true,
            Some(false) => return false,
            None => {}
        }

        visiting.insert(name.to_string(), true);
        if let Some(task) = workflow.tasks.get(name) {
            for dep in &task.depends_on {
                if visit(workflow, dep, visiting) {
                    return true;
                }
            }
        }
        visiting.insert(name.to_string(), false);
        false
    }

    let mut visiting = HashMap::new();
    for name in workflow.tasks.keys() {
        if visit(workflow, name, &mut visiting) {
            return Some(name.clone());
        }
    }
    None
}
