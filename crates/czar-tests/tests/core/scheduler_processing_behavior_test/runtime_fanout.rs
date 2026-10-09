use super::runtime_fanout_support::*;
use super::support::*;
use anyhow::Result;
use czar_core::database::{NewTask, TaskRepository};
use czar_core::models::TaskStatus;
use czar_core::models::json_inner;
use czar_core::scheduler::Scheduler;
use std::sync::Arc;
use tokio::time::{Duration, sleep, timeout};

#[tokio::test]
async fn test_scheduler_enforces_parallel_concurrency_limit() -> Result<()> {
    let db = setup_db().await;
    let (_workflow_id, run_id) = create_runtime_fanout_run(
        &db,
        "parallel-concurrency-runtime",
        &[
            NewTask {
                task_index: 0,
                task_name: "process[0]".to_string(),
                executor_type: "process".to_string(),
                depends_on: vec![],
                params: serde_json::json!({
                    "task_name":"process[0]",
                    "parallel_group":"process",
                    "parallel_count":3,
                    "parallel_index":0,
                    "parallel_concurrency":2
                }),
                max_retries: 0,
                timeout_seconds: Some(10),
            },
            NewTask {
                task_index: 1,
                task_name: "process[1]".to_string(),
                executor_type: "process".to_string(),
                depends_on: vec![],
                params: serde_json::json!({
                    "task_name":"process[1]",
                    "parallel_group":"process",
                    "parallel_count":3,
                    "parallel_index":1,
                    "parallel_concurrency":2
                }),
                max_retries: 0,
                timeout_seconds: Some(10),
            },
            NewTask {
                task_index: 2,
                task_name: "process[2]".to_string(),
                executor_type: "process".to_string(),
                depends_on: vec![],
                params: serde_json::json!({
                    "task_name":"process[2]",
                    "parallel_group":"process",
                    "parallel_count":3,
                    "parallel_index":2,
                    "parallel_concurrency":2
                }),
                max_retries: 0,
                timeout_seconds: Some(10),
            },
        ],
    )
    .await?;

    let executor = Arc::new(ConcurrencyExecutor::new());
    let scheduler = Scheduler::new_with_config(
        db.clone(),
        Arc::new(ConcurrencyExecutorManager {
            executor: executor.clone(),
        }),
        test_config(),
    );
    let handle = tokio::spawn(async move {
        let _ = scheduler.run().await;
    });

    timeout(Duration::from_secs(2), async {
        loop {
            let dispatched = executor.dispatched_task_names().await;
            if dispatched.len() >= 2 {
                break Ok::<Vec<String>, anyhow::Error>(dispatched);
            }
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await??;

    sleep(Duration::from_millis(100)).await;
    let first_wave = executor.dispatched_task_names().await;
    assert_eq!(first_wave.len(), 2);
    assert!(first_wave.iter().any(|name| name == "process[0]"));
    assert!(first_wave.iter().any(|name| name == "process[1]"));
    assert!(!first_wave.iter().any(|name| name == "process[2]"));

    let first_task = db
        .list_tasks(run_id)
        .await?
        .into_iter()
        .find(|task| task.task_name == "process[0]")
        .expect("process[0] exists");
    executor
        .complete(first_task.id, serde_json::json!({"slot": 0}))
        .await;

    timeout(Duration::from_secs(2), async {
        loop {
            let dispatched = executor.dispatched_task_names().await;
            if dispatched.iter().any(|name| name == "process[2]") {
                break Ok::<(), anyhow::Error>(());
            }
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await??;
    handle.abort();

    let final_dispatched = executor.dispatched_task_names().await;
    assert_eq!(final_dispatched.len(), 3);
    assert!(final_dispatched.iter().any(|name| name == "process[2]"));
    Ok(())
}

#[tokio::test]
async fn test_scheduler_materializes_dynamic_parallel_runtime_fanout() -> Result<()> {
    let db = setup_db().await;
    let (_workflow_id, run_id) = create_runtime_fanout_run(
        &db,
        "dynamic-parallel-runtime",
        &[
            NewTask {
                task_index: 0,
                task_name: "discover".to_string(),
                executor_type: "process".to_string(),
                depends_on: vec![],
                params: serde_json::json!({"task_name":"discover"}),
                max_retries: 0,
                timeout_seconds: Some(10),
            },
            NewTask {
                task_index: 1,
                task_name: "process".to_string(),
                executor_type: "process".to_string(),
                depends_on: vec!["discover".to_string()],
                params: serde_json::json!({
                    "task_name":"process",
                    "parallel_ref":"discover.count",
                    "parallel_mode":"czar"
                }),
                max_retries: 0,
                timeout_seconds: Some(10),
            },
            NewTask {
                task_index: 2,
                task_name: "aggregate".to_string(),
                executor_type: "process".to_string(),
                depends_on: vec!["process".to_string()],
                params: serde_json::json!({
                    "task_name":"aggregate",
                    "task_bindings":{
                        "slots":{"ref":"tasks.process.output.slot"}
                    }
                }),
                max_retries: 0,
                timeout_seconds: Some(10),
            },
        ],
    )
    .await?;
    mark_task_success(&db, run_id, "discover", serde_json::json!({"count": 2})).await?;

    let executor = Arc::new(FanoutExecutor::new());
    let scheduler = Scheduler::new_with_config(
        db.clone(),
        Arc::new(FanoutExecutorManager {
            executor: executor.clone(),
        }),
        test_config(),
    );
    let handle = tokio::spawn(async move {
        let _ = scheduler.run().await;
    });

    let wait = timeout(Duration::from_secs(2), async {
        loop {
            let tasks = db.list_tasks(run_id).await?;
            let aggregate = tasks
                .iter()
                .find(|task| task.task_name == "aggregate")
                .expect("aggregate exists");
            if aggregate.status() == TaskStatus::Success {
                break Ok::<Vec<czar_core::models::Task>, anyhow::Error>(tasks);
            }
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await??;
    handle.abort();

    let task_names: Vec<String> = wait.iter().map(|task| task.task_name.clone()).collect();
    assert!(task_names.iter().any(|name| name == "process[0]"));
    assert!(task_names.iter().any(|name| name == "process[1]"));
    assert!(!task_names.iter().any(|name| name == "process"));

    let aggregate = wait
        .iter()
        .find(|task| task.task_name == "aggregate")
        .expect("aggregate exists");
    assert_eq!(
        aggregate.output.as_ref().map(json_inner).cloned(),
        Some(serde_json::json!({"slots":[0, 1]}))
    );

    let aggregate_depends_on = aggregate.depends_on.clone();
    assert_eq!(
        aggregate_depends_on,
        vec!["process[0]".to_string(), "process[1]".to_string()]
    );
    Ok(())
}

#[tokio::test]
async fn test_scheduler_materializes_dynamic_foreach_runtime_fanout() -> Result<()> {
    let db = setup_db().await;
    let (_workflow_id, run_id) = create_runtime_fanout_run(
        &db,
        "dynamic-foreach-runtime",
        &[
            NewTask {
                task_index: 0,
                task_name: "discover".to_string(),
                executor_type: "process".to_string(),
                depends_on: vec![],
                params: serde_json::json!({"task_name":"discover"}),
                max_retries: 0,
                timeout_seconds: Some(10),
            },
            NewTask {
                task_index: 1,
                task_name: "process".to_string(),
                executor_type: "process".to_string(),
                depends_on: vec!["discover".to_string()],
                params: serde_json::json!({
                    "task_name":"process",
                    "foreach_ref":"discover.items",
                    "parallel_mode":"czar",
                    "task_bindings":{
                        "name":{"ref":"foreach.item.name"}
                    }
                }),
                max_retries: 0,
                timeout_seconds: Some(10),
            },
            NewTask {
                task_index: 2,
                task_name: "aggregate".to_string(),
                executor_type: "process".to_string(),
                depends_on: vec!["process".to_string()],
                params: serde_json::json!({
                    "task_name":"aggregate",
                    "task_bindings":{
                        "names":{"ref":"tasks.process.output.name"}
                    }
                }),
                max_retries: 0,
                timeout_seconds: Some(10),
            },
        ],
    )
    .await?;
    mark_task_success(
        &db,
        run_id,
        "discover",
        serde_json::json!({"items": [{"name": "alpha"}, {"name": "beta"}]}),
    )
    .await?;

    let executor = Arc::new(FanoutExecutor::new());
    let scheduler = Scheduler::new_with_config(
        db.clone(),
        Arc::new(FanoutExecutorManager { executor }),
        test_config(),
    );
    let handle = tokio::spawn(async move {
        let _ = scheduler.run().await;
    });

    let aggregate = timeout(Duration::from_secs(2), async {
        loop {
            let tasks = db.list_tasks(run_id).await?;
            let aggregate = tasks
                .iter()
                .find(|task| task.task_name == "aggregate")
                .expect("aggregate exists")
                .clone();
            if aggregate.status() == TaskStatus::Success {
                break Ok::<czar_core::models::Task, anyhow::Error>(aggregate);
            }
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await??;
    handle.abort();

    assert_eq!(
        aggregate.output.as_ref().map(json_inner).cloned(),
        Some(serde_json::json!({"names":["alpha", "beta"]}))
    );
    Ok(())
}

#[tokio::test]
async fn test_scheduler_materializes_dynamic_cloud_run_task_count() -> Result<()> {
    let db = setup_db().await;
    let (_workflow_id, run_id) = create_runtime_fanout_run(
        &db,
        "dynamic-cloud-run-runtime",
        &[
            NewTask {
                task_index: 0,
                task_name: "discover".to_string(),
                executor_type: "process".to_string(),
                depends_on: vec![],
                params: serde_json::json!({"task_name":"discover"}),
                max_retries: 0,
                timeout_seconds: Some(10),
            },
            NewTask {
                task_index: 1,
                task_name: "cloud-process".to_string(),
                executor_type: "cloudrun".to_string(),
                depends_on: vec!["discover".to_string()],
                params: serde_json::json!({
                    "task_name":"cloud-process",
                    "parallel_ref":"discover.count",
                    "parallel_mode":"cloud_run_tasks"
                }),
                max_retries: 0,
                timeout_seconds: Some(10),
            },
        ],
    )
    .await?;
    mark_task_success(&db, run_id, "discover", serde_json::json!({"count": 3})).await?;

    let executor = Arc::new(FanoutExecutor::new());
    let scheduler = Scheduler::new_with_config(
        db.clone(),
        Arc::new(FanoutExecutorManager {
            executor: executor.clone(),
        }),
        test_config(),
    );
    let handle = tokio::spawn(async move {
        let _ = scheduler.run().await;
    });

    timeout(Duration::from_secs(2), async {
        loop {
            let tasks = db.list_tasks(run_id).await?;
            let task = tasks
                .iter()
                .find(|task| task.task_name == "cloud-process")
                .expect("cloud task exists");
            if task.status() == TaskStatus::Success {
                break Ok::<(), anyhow::Error>(());
            }
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await??;
    handle.abort();

    let dispatched = executor.dispatched_params().await;
    assert_eq!(dispatched.len(), 1);
    assert_eq!(dispatched[0]["task_count"], serde_json::json!(3));

    let tasks = db.list_tasks(run_id).await?;
    let cloud_tasks: Vec<_> = tasks
        .into_iter()
        .filter(|task| task.task_name.starts_with("cloud-process"))
        .collect();
    assert_eq!(
        cloud_tasks.len(),
        1,
        "cloud run mode should stay as one task"
    );
    Ok(())
}
