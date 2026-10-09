pub(super) fn build_wasm_env(
    params: Option<&serde_json::Value>,
) -> std::collections::HashMap<String, String> {
    let mut env_vars = std::collections::HashMap::new();

    if let Some(params) = params
        && let Some(obj) = params.as_object()
    {
        for (key, value) in obj {
            match key.as_str() {
                "task_input" => {
                    env_vars.insert("CZAR_INPUT_JSON".to_string(), value.to_string());
                }
                "env" => {
                    if let Some(env_obj) = value.as_object() {
                        for (env_key, env_value) in env_obj {
                            if let Some(value) = env_value.as_str() {
                                env_vars.insert(env_key.clone(), value.to_string());
                            } else if env_value.is_number() || env_value.is_boolean() {
                                env_vars.insert(env_key.clone(), env_value.to_string());
                            }
                        }
                    }
                }
                _ => {
                    if let Some(value) = value.as_str() {
                        env_vars.insert(key.clone(), value.to_string());
                    } else if value.is_number() || value.is_boolean() {
                        env_vars.insert(key.clone(), value.to_string());
                    }
                }
            }
        }
    }

    env_vars
}

pub(super) fn is_wasmtime_runtime(runtime: &str) -> bool {
    std::path::Path::new(runtime)
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name == "wasmtime")
        .unwrap_or(false)
}
