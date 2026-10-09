use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EnvBinding {
    Literal(String),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct SecretFileBinding {
    pub name: String,
    pub path: PathBuf,
}

pub(crate) fn collect_env_bindings(
    env_value: &serde_json::Value,
) -> Result<HashMap<String, EnvBinding>> {
    let Some(env_obj) = env_value.as_object() else {
        anyhow::bail!("task field 'env' must be an object");
    };

    let mut bindings = HashMap::with_capacity(env_obj.len());
    for (env_key, env_value) in env_obj {
        if let Some(value) = scalar_env_value(env_value) {
            bindings.insert(env_key.clone(), EnvBinding::Literal(value));
            continue;
        }

        anyhow::bail!(
            "task env '{}' must be a string, number, or boolean",
            env_key
        );
    }

    Ok(bindings)
}

#[cfg(feature = "cloudrun")]
pub(crate) async fn resolve_env_bindings(
    bindings: HashMap<String, EnvBinding>,
) -> Result<HashMap<String, String>> {
    let mut resolved = HashMap::with_capacity(bindings.len());

    for (env_key, binding) in bindings {
        match binding {
            EnvBinding::Literal(value) => {
                resolved.insert(env_key, value);
            }
        }
    }

    Ok(resolved)
}

pub(crate) fn collect_secret_file_bindings(
    secrets_value: &serde_json::Value,
) -> Result<Vec<SecretFileBinding>> {
    let Some(secret_items) = secrets_value.as_array() else {
        anyhow::bail!("task field 'secrets' must be a list");
    };

    let mut bindings = Vec::with_capacity(secret_items.len());
    for secret_item in secret_items {
        let Some(secret_obj) = secret_item.as_object() else {
            anyhow::bail!("task field 'secrets' entries must be objects");
        };
        let name = secret_obj
            .get("name")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                anyhow::anyhow!("secret file mounts require a non-empty string 'name'")
            })?;
        let _secret_ref = secret_obj
            .get("secret")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                anyhow::anyhow!("secret file mounts require a non-empty string 'secret'")
            })?;
        let path = secret_obj
            .get("path")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                anyhow::anyhow!("secret file mounts require a non-empty string 'path'")
            })?;
        bindings.push(SecretFileBinding {
            name: name.to_string(),
            path: PathBuf::from(path),
        });
    }

    Ok(bindings)
}

fn scalar_env_value(value: &serde_json::Value) -> Option<String> {
    if let Some(raw) = value.as_str() {
        Some(raw.to_string())
    } else if value.is_number() || value.is_boolean() {
        Some(value.to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collect_env_bindings_supports_scalars() {
        let bindings = collect_env_bindings(&serde_json::json!({
            "LOG_LEVEL": "debug",
            "MAX_RETRIES": 3
        }))
        .expect("collect env bindings");

        assert_eq!(
            bindings.get("LOG_LEVEL"),
            Some(&EnvBinding::Literal("debug".to_string()))
        );
        assert_eq!(
            bindings.get("MAX_RETRIES"),
            Some(&EnvBinding::Literal("3".to_string()))
        );
    }

    #[test]
    fn test_collect_env_bindings_rejects_object_values() {
        let err = collect_env_bindings(&serde_json::json!({
            "API_KEY": {
                "secret": "projects/demo/secrets/api-key/versions/latest"
            }
        }))
        .expect_err("object env values must fail");
        assert!(err.to_string().contains("string, number, or boolean"));
    }

    #[test]
    fn test_collect_secret_file_bindings_supports_name_secret_and_path() {
        let bindings = collect_secret_file_bindings(&serde_json::json!([
            {
                "name": "service-account",
                "secret": "projects/demo/secrets/api-key",
                "path": ".czar/secrets/key.json"
            }
        ]))
        .expect("collect secret file bindings");

        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].name, "service-account");
        assert_eq!(bindings[0].path, PathBuf::from(".czar/secrets/key.json"));
    }
}
