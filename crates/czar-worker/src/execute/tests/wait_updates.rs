use super::*;
use czar_executors::dispatch::wait_for_terminal_update;

#[tokio::test]
async fn test_wait_for_terminal_update_skips_other_tasks_and_uses_failure_default() {
    let task_id = Uuid::new_v4();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    tx.send(StatusUpdate {
        task_id: Uuid::new_v4(),
        status: "success".to_string(),
        log: None,
        output: Some(serde_json::json!({"wrong": true})),
        error: None,
        progress: None,
    })
    .expect("send update");
    tx.send(StatusUpdate {
        task_id,
        status: "failed".to_string(),
        log: None,
        output: None,
        error: None,
        progress: None,
    })
    .expect("send update");

    let err = wait_for_terminal_update(task_id, &mut rx)
        .await
        .expect_err("should fail");
    assert!(err.contains("Execution failed"));
}

#[tokio::test]
async fn test_wait_for_terminal_update_closed_channel_and_explicit_error() {
    let task_id = Uuid::new_v4();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    tx.send(StatusUpdate {
        task_id,
        status: "failed".to_string(),
        log: None,
        output: None,
        error: Some("boom".to_string()),
        progress: None,
    })
    .expect("send update");
    let err = wait_for_terminal_update(task_id, &mut rx)
        .await
        .expect_err("should fail with explicit error");
    assert!(err.contains("boom"));

    let (tx2, mut rx2) = tokio::sync::mpsc::unbounded_channel::<StatusUpdate>();
    drop(tx2);
    let err2 = wait_for_terminal_update(Uuid::new_v4(), &mut rx2)
        .await
        .expect_err("closed channel should fail");
    assert!(err2.contains("channel closed"));
}
