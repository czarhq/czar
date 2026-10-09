use anyhow::Result;

use crate::client::D1Client;

const MIGRATIONS: &[(&str, &str)] = &[("001_init.sql", include_str!("../migrations/001_init.sql"))];

pub(crate) async fn run_migrations(client: &D1Client) -> Result<()> {
    client
        .execute(
            "CREATE TABLE IF NOT EXISTS __czar_d1_migrations (name TEXT PRIMARY KEY, applied_at TEXT NOT NULL DEFAULT (STRFTIME('%Y-%m-%dT%H:%M:%fZ','now')))",
            Vec::new(),
        )
        .await?;

    for (name, sql) in MIGRATIONS {
        let already_applied = client
            .execute(
                "SELECT name FROM __czar_d1_migrations WHERE name = ? LIMIT 1",
                vec![(*name).into()],
            )
            .await?;
        if !already_applied.results.is_empty() {
            continue;
        }

        for statement in split_sql(sql) {
            client.execute(&statement, Vec::new()).await?;
        }

        client
            .execute(
                "INSERT INTO __czar_d1_migrations (name) VALUES (?)",
                vec![(*name).into()],
            )
            .await?;
    }

    Ok(())
}

fn split_sql(input: &str) -> Vec<String> {
    let mut statements = Vec::new();
    let mut current = String::new();
    let mut chars = input.chars().peekable();
    let mut in_single_quote = false;

    while let Some(ch) = chars.next() {
        if !in_single_quote && ch == '-' && chars.peek() == Some(&'-') {
            chars.next();
            for next in chars.by_ref() {
                if next == '\n' {
                    current.push('\n');
                    break;
                }
            }
            continue;
        }

        if ch == '\'' {
            in_single_quote = !in_single_quote;
            current.push(ch);
            continue;
        }

        if ch == ';' && !in_single_quote {
            let trimmed = current.trim();
            if !trimmed.is_empty() {
                statements.push(trimmed.to_string());
            }
            current.clear();
            continue;
        }

        current.push(ch);
    }

    let trimmed = current.trim();
    if !trimmed.is_empty() {
        statements.push(trimmed.to_string());
    }

    statements
}
