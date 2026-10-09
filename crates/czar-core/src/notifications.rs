use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct NotificationsConfig {
    #[serde(default)]
    pub on_failure: Option<FailureNotificationsConfig>,
}

impl NotificationsConfig {
    pub fn is_empty(&self) -> bool {
        self.on_failure
            .as_ref()
            .is_none_or(FailureNotificationsConfig::is_empty)
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct FailureNotificationsConfig {
    #[serde(default)]
    pub email: Option<EmailNotificationConfig>,
    #[serde(default)]
    pub slack: Option<SlackNotificationConfig>,
}

impl FailureNotificationsConfig {
    pub fn is_empty(&self) -> bool {
        self.email.is_none() && self.slack.is_none()
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct EmailNotificationConfig {
    #[serde(default)]
    pub to: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct SlackNotificationConfig {
    pub webhook_url: String,
}

#[derive(Debug, Clone, Default)]
pub struct RunFailureNotification {
    pub run_id: String,
    pub workflow_name: String,
    pub error: Option<String>,
    pub failed_tasks: Vec<String>,
}

pub async fn send_run_failure_notifications(
    config: &NotificationsConfig,
    notification: &RunFailureNotification,
) -> Result<()> {
    let Some(on_failure) = config.on_failure.as_ref() else {
        return Ok(());
    };

    if let Some(slack) = &on_failure.slack {
        send_slack_failure_notification(slack, notification).await?;
    }
    if let Some(email) = &on_failure.email
        && !email.to.is_empty()
    {
        send_email_failure_notification(email, notification)?;
    }

    Ok(())
}

pub fn failure_subject(notification: &RunFailureNotification) -> String {
    format!(
        "[Czar] Run failed: {} ({})",
        notification.workflow_name, notification.run_id
    )
}

pub fn failure_body(notification: &RunFailureNotification) -> String {
    let mut lines = vec![
        format!("Workflow: {}", notification.workflow_name),
        format!("Run ID: {}", notification.run_id),
    ];
    if !notification.failed_tasks.is_empty() {
        lines.push(format!(
            "Failed tasks: {}",
            notification.failed_tasks.join(", ")
        ));
    }
    if let Some(error) = notification.error.as_deref()
        && !error.trim().is_empty()
    {
        lines.push(format!("Error: {}", error.trim()));
    }
    lines.join("\n")
}

async fn send_slack_failure_notification(
    config: &SlackNotificationConfig,
    notification: &RunFailureNotification,
) -> Result<()> {
    let client = Client::new();
    let payload = serde_json::json!({
        "text": format!("{}\n{}", failure_subject(notification), failure_body(notification))
    });

    let response = client
        .post(&config.webhook_url)
        .json(&payload)
        .send()
        .await
        .context("failed to send Slack notification")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("Slack notification failed with status {}: {}", status, body);
    }

    Ok(())
}

fn send_email_failure_notification(
    config: &EmailNotificationConfig,
    notification: &RunFailureNotification,
) -> Result<()> {
    let sendmail_bin =
        std::env::var("CZAR_SENDMAIL_BIN").unwrap_or_else(|_| "sendmail".to_string());
    let from = std::env::var("CZAR_NOTIFICATION_EMAIL_FROM")
        .unwrap_or_else(|_| "czar@localhost".to_string());

    let mut child = Command::new(&sendmail_bin)
        .arg("-t")
        .arg("-i")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("failed to spawn sendmail binary '{}'", sendmail_bin))?;

    let message = format!(
        "To: {}\nFrom: {}\nSubject: {}\nContent-Type: text/plain; charset=utf-8\n\n{}\n",
        config.to.join(", "),
        from,
        failure_subject(notification),
        failure_body(notification)
    );

    if let Some(stdin) = child.stdin.as_mut() {
        stdin
            .write_all(message.as_bytes())
            .context("failed to write sendmail message")?;
    }
    let output = child
        .wait_with_output()
        .context("failed to wait for sendmail delivery")?;
    if !output.status.success() {
        anyhow::bail!(
            "sendmail delivery failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;
    use uuid::Uuid;

    fn sample_notification() -> RunFailureNotification {
        RunFailureNotification {
            run_id: "run-123".to_string(),
            workflow_name: "daily_etl".to_string(),
            error: Some("upstream failed".to_string()),
            failed_tasks: vec!["transform".to_string(), "load".to_string()],
        }
    }

    #[test]
    fn test_failure_body_includes_failed_tasks_and_error() {
        let body = failure_body(&sample_notification());
        assert!(body.contains("Workflow: daily_etl"));
        assert!(body.contains("Failed tasks: transform, load"));
        assert!(body.contains("Error: upstream failed"));
    }

    #[tokio::test]
    async fn test_send_slack_failure_notification_posts_payload() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let addr = listener.local_addr().expect("local addr");
        let captured = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        let captured_clone = captured.clone();
        let handle = thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 8192];
                let read = stream.read(&mut buf).expect("read request");
                *captured_clone.lock().expect("capture lock") =
                    String::from_utf8_lossy(&buf[..read]).to_string();
                let response =
                    "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nContent-Type: text/plain\r\n\r\nok";
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
            }
        });

        let config = SlackNotificationConfig {
            webhook_url: format!("http://{addr}/hooks/test"),
        };
        send_slack_failure_notification(&config, &sample_notification())
            .await
            .expect("send slack notification");

        let _ = handle.join();
        let request = captured.lock().expect("capture lock").clone();
        assert!(request.starts_with("POST /hooks/test"));
        assert!(request.contains("daily_etl"));
        assert!(request.contains("transform, load"));
    }

    #[test]
    fn test_send_email_failure_notification_uses_sendmail_binary() {
        let root = std::env::temp_dir().join(format!("czar-sendmail-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("create dir");
        let capture_path = root.join("message.txt");
        let script_path = root.join("sendmail");
        fs::write(
            &script_path,
            format!("#!/bin/sh\ncat > \"{}\"\n", capture_path.to_string_lossy()),
        )
        .expect("write script");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&script_path).expect("metadata").permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&script_path, perms).expect("set perms");
        }

        let prev_bin = std::env::var("CZAR_SENDMAIL_BIN").ok();
        let prev_from = std::env::var("CZAR_NOTIFICATION_EMAIL_FROM").ok();
        unsafe {
            std::env::set_var("CZAR_SENDMAIL_BIN", &script_path);
            std::env::set_var("CZAR_NOTIFICATION_EMAIL_FROM", "czar@test.local");
        }

        let email = EmailNotificationConfig {
            to: vec!["team@example.com".to_string()],
        };
        send_email_failure_notification(&email, &sample_notification())
            .expect("send email notification");

        let message = fs::read_to_string(&capture_path).expect("read capture");
        assert!(message.contains("To: team@example.com"));
        assert!(message.contains("From: czar@test.local"));
        assert!(message.contains("Subject: [Czar] Run failed: daily_etl (run-123)"));

        match prev_bin {
            Some(value) => unsafe { std::env::set_var("CZAR_SENDMAIL_BIN", value) },
            None => unsafe { std::env::remove_var("CZAR_SENDMAIL_BIN") },
        }
        match prev_from {
            Some(value) => unsafe { std::env::set_var("CZAR_NOTIFICATION_EMAIL_FROM", value) },
            None => unsafe { std::env::remove_var("CZAR_NOTIFICATION_EMAIL_FROM") },
        }
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn test_notifications_config_is_empty_when_unset() {
        assert!(NotificationsConfig::default().is_empty());
        assert!(
            !NotificationsConfig {
                on_failure: Some(FailureNotificationsConfig {
                    email: Some(EmailNotificationConfig {
                        to: vec!["team@example.com".to_string()],
                    }),
                    slack: None,
                }),
            }
            .is_empty()
        );
    }
}
