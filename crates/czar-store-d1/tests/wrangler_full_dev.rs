mod support;

use anyhow::Result;
use reqwest::StatusCode;
use support::{
    DevVarsGuard, TEST_ACCOUNT_ID, TEST_BASIC_AUTH_PASS, TEST_BASIC_AUTH_USER, TEST_DATABASE_ID,
    TEST_TOKEN, full_app_dir, harness_dir, host_ip_for_containers, require_react_dist,
    reserve_local_port, start_wrangler_dev, wait_for_http_status,
};
use tokio::time::{Duration, Instant, sleep};

#[tokio::test]
#[ignore = "requires local wrangler dev, Docker, and React assets; run `just test-cloudflare-full-local`"]
async fn wrangler_dev_full_app_smoke() -> Result<()> {
    require_react_dist()?;

    let d1_port = reserve_local_port()?;
    let d1 = start_wrangler_dev(&harness_dir(), "wrangler.jsonc", "0.0.0.0", d1_port).await?;
    wait_for_http_status(&format!("{}/health", d1.base_url()), StatusCode::OK, None).await?;

    let host_ip = host_ip_for_containers()?;
    let full_dir = full_app_dir();
    let _dev_vars = DevVarsGuard::write(
        full_dir.join(".dev.vars"),
        &format!(
            "BASIC_AUTH_USER={TEST_BASIC_AUTH_USER}\n\
             BASIC_AUTH_PASS={TEST_BASIC_AUTH_PASS}\n\
             CZAR_D1_ACCOUNT_ID={TEST_ACCOUNT_ID}\n\
             CZAR_D1_DATABASE_ID={TEST_DATABASE_ID}\n\
             CZAR_D1_API_TOKEN={TEST_TOKEN}\n\
             CZAR_D1_BASE_URL=http://{host_ip}:{d1_port}\n"
        ),
    )?;

    let full_port = reserve_local_port()?;
    let full = start_wrangler_dev(&full_dir, "wrangler.jsonc", "127.0.0.1", full_port).await?;

    wait_for_http_status(
        &format!("{}/", full.base_url()),
        StatusCode::UNAUTHORIZED,
        None,
    )
    .await?;
    wait_for_http_status(
        &format!("{}/api/workflows", full.base_url()),
        StatusCode::OK,
        Some((TEST_BASIC_AUTH_USER, TEST_BASIC_AUTH_PASS)),
    )
    .await?;

    let client = reqwest::Client::new();

    let root = client
        .get(format!("{}/", full.base_url()))
        .basic_auth(TEST_BASIC_AUTH_USER, Some(TEST_BASIC_AUTH_PASS))
        .send()
        .await?;
    assert_eq!(root.status(), StatusCode::OK);
    assert!(root.text().await?.contains("<!doctype html>"));

    let workflows = client
        .get(format!("{}/api/workflows", full.base_url()))
        .basic_auth(TEST_BASIC_AUTH_USER, Some(TEST_BASIC_AUTH_PASS))
        .send()
        .await?;
    assert_eq!(workflows.status(), StatusCode::OK);
    let body: serde_json::Value = workflows.json().await?;
    let workflows = body
        .get("items")
        .and_then(serde_json::Value::as_array)
        .expect("workflow list should expose an items array");
    assert!(
        !workflows.is_empty(),
        "expected the Cloudflare full app to preload example workflows"
    );

    let create_run = client
        .post(format!("{}/api/runs", full.base_url()))
        .basic_auth(TEST_BASIC_AUTH_USER, Some(TEST_BASIC_AUTH_PASS))
        .json(&serde_json::json!({ "workflow": "simple_demo" }))
        .send()
        .await?;
    assert_eq!(create_run.status(), StatusCode::OK);
    let create_run_body: serde_json::Value = create_run.json().await?;
    let run_id = create_run_body
        .get("run_id")
        .and_then(serde_json::Value::as_str)
        .expect("run create should return run_id");

    let deadline = Instant::now() + Duration::from_secs(90);
    loop {
        let detail = client
            .get(format!("{}/api/runs/{run_id}", full.base_url()))
            .basic_auth(TEST_BASIC_AUTH_USER, Some(TEST_BASIC_AUTH_PASS))
            .send()
            .await?;
        assert_eq!(detail.status(), StatusCode::OK);
        let body: serde_json::Value = detail.json().await?;
        let status = body
            .get("run")
            .and_then(|run| run.get("status"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        match status {
            "success" => break,
            "failed" | "cancelled" | "canceled" => {
                panic!("expected simple_demo run to succeed, got {status}: {body}");
            }
            _ => {}
        }

        if Instant::now() >= deadline {
            panic!("timed out waiting for simple_demo run to succeed: {body}");
        }

        sleep(Duration::from_secs(2)).await;
    }

    Ok(())
}
