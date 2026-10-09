use super::*;

#[tokio::test]
async fn test_execute_process_with_task_module_runner_path() {
    let _guard = python_test_lock().lock().await;
    let base = std::env::temp_dir().join(format!("czar-process-test-{}", Uuid::new_v4()));
    fs::create_dir_all(&base).expect("create temp dir");
    write_pyproject(&base);

    let runner = base.join("runner.py");
    fs::write(
        &runner,
        r#"
import json
import os

payload = {
    "module": os.environ.get("CZAR_TASK_MODULE"),
    "function": os.environ.get("CZAR_TASK_FUNCTION"),
    "workflow": os.environ.get("CZAR_WORKFLOW_NAME"),
    "input": json.loads(os.environ.get("CZAR_INPUT_JSON", "{}")),
}
print("CZAR_OUTPUT:" + json.dumps(payload))
"#,
    )
    .expect("write runner");

    let executor = ProcessExecutor::new(Some(base.to_string_lossy().to_string()));
    let (tx, mut rx) = mpsc::unbounded_channel();
    executor.set_status_channel(tx).await;
    let task_id = Uuid::new_v4();

    let execution_id = executor
        .execute(
            task_id,
            "ignored",
            Some(serde_json::json!({
                "task_module": "pkg.tasks",
                "task_function": "run",
                "runner_path": runner,
                "python_path": base,
                "task_input": {"value": 9},
                "task_name": "demo-task",
                "workflow_name": "demo-workflow",
                "run_id": task_id.to_string(),
            })),
        )
        .await
        .expect("execute process with module");
    assert!(!execution_id.is_empty());

    let updates = collect_updates(&mut rx).await.expect("collect updates");
    let terminal = updates.last().expect("terminal update");
    assert_eq!(terminal.status, "success");
    let output = terminal.output.clone().expect("output payload");
    assert_eq!(output["module"], "pkg.tasks");
    assert_eq!(output["function"], "run");
    assert_eq!(output["workflow"], "demo-workflow");
    assert_eq!(output["input"], serde_json::json!({"value": 9}));

    let _ = fs::remove_dir_all(&base);
}

#[tokio::test]
async fn test_execute_process_python_runner_injects_czar_context() {
    let _guard = python_test_lock().lock().await;
    let base = std::env::temp_dir().join(format!("czar-process-test-{}", Uuid::new_v4()));
    fs::create_dir_all(&base).expect("create temp dir");

    let sdk_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../python-sdk")
        .canonicalize()
        .expect("canonicalize python-sdk path");
    fs::write(
        base.join("pyproject.toml"),
        format!(
            "[project]\nname='czar-process-test'\nversion='0.1.0'\nrequires-python='>=3.12'\ndependencies=['czar']\n\n[tool.uv.sources]\nczar = {{ path = '{}' }}\n",
            sdk_path.display()
        ),
    )
    .expect("write pyproject");

    let task_file = base.join("task_context.py");
    fs::write(
        &task_file,
        r#"
from czar import Context, task

@task
def run(value: int, ctx: Context) -> dict:
    ctx.log.info(f"processing {value}")
    ctx.metrics.record("rows_processed", value, labels={"task": ctx.task_name, "attempt": ctx.attempt})
    ctx.progress(1, 1, message="done")
    return {
        "value": value,
        "run_id": ctx.run_id,
        "workflow_name": ctx.workflow_name,
        "task_name": ctx.task_name,
        "task_id": ctx.task_id,
        "execution_name": ctx.execution_name,
        "attempt": ctx.attempt,
        "parallel_index": ctx.parallel_index,
        "parallel_count": ctx.parallel_count,
    }
"#,
    )
    .expect("write task file");

    let runner = base.join("run_task.py");
    fs::write(&runner, crate::python_runtime::RUN_TASK_PY).expect("write runner script");

    let executor = ProcessExecutor::new(Some(base.to_string_lossy().to_string()));
    let (tx, mut rx) = mpsc::unbounded_channel();
    executor.set_status_channel(tx).await;
    let task_id = Uuid::new_v4();

    let execution_id = executor
        .execute(
            task_id,
            "ignored",
            Some(serde_json::json!({
                "task_file": task_file,
                "task_function": "run",
                "runner_path": runner,
                "python_path": base,
                "task_input": {"value": 123},
                "task_name": "task-context[3]",
                "parallel_index": 3,
                "parallel_count": 8,
                "workflow_name": "wf-context",
                "run_id": task_id.to_string(),
                "env": {"CZAR_ATTEMPT": 4},
            })),
        )
        .await
        .expect("execute process with context task");
    assert!(!execution_id.is_empty());

    let updates = collect_updates(&mut rx).await.expect("collect updates");
    let progress_updates: Vec<&StatusUpdate> = updates
        .iter()
        .filter(|update| update.status == "progress")
        .collect();
    assert!(
        !progress_updates.is_empty(),
        "expected at least one progress update"
    );

    let terminal = updates.last().expect("terminal update");
    assert_eq!(terminal.status, "success");
    let output = terminal.output.clone().expect("output payload");
    assert_eq!(output["value"], serde_json::json!(123));
    assert_eq!(output["run_id"], serde_json::json!(task_id.to_string()));
    assert_eq!(output["workflow_name"], serde_json::json!("wf-context"));
    assert_eq!(output["task_name"], serde_json::json!("task-context[3]"));
    assert_eq!(output["task_id"], serde_json::json!(task_id.to_string()));
    assert!(
        output["execution_name"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
    );
    assert_eq!(output["attempt"], serde_json::json!(4));
    assert_eq!(output["parallel_index"], serde_json::json!(3));
    assert_eq!(output["parallel_count"], serde_json::json!(8));
    let log_lines: Vec<&str> = updates
        .iter()
        .filter_map(|update| update.log.as_deref())
        .collect();
    assert!(log_lines.iter().any(|line| line.contains("processing 123")));
    assert!(
        log_lines
            .iter()
            .any(|line| line.contains("\"rows_processed\""))
    );

    let _ = fs::remove_dir_all(&base);
}

#[tokio::test]
async fn test_execute_process_with_task_file_sets_expected_env() {
    let _guard = python_test_lock().lock().await;
    let base = std::env::temp_dir().join(format!("czar-process-test-{}", Uuid::new_v4()));
    fs::create_dir_all(&base).expect("create temp dir");
    write_pyproject(&base);

    let task_file = base.join("task.py");
    fs::write(&task_file, "def run(x: int) -> int:\n    return x\n").expect("write task file");

    let runner = base.join("runner.py");
    fs::write(
        &runner,
        r#"
import json
import os

print(
    "CZAR_OUTPUT:"
    + json.dumps(
        {
            "task_file": os.environ.get("CZAR_TASK_FILE"),
            "task_function": os.environ.get("CZAR_TASK_FUNCTION"),
            "task_name": os.environ.get("CZAR_TASK_NAME"),
            "workflow_name": os.environ.get("CZAR_WORKFLOW_NAME"),
            "run_id": os.environ.get("CZAR_RUN_ID"),
            "pythonpath": os.environ.get("PYTHONPATH"),
            "input": json.loads(os.environ.get("CZAR_INPUT_JSON", "{}")),
        }
    )
)
"#,
    )
    .expect("write runner");

    let executor = ProcessExecutor::new(Some(base.to_string_lossy().to_string()));
    let (tx, mut rx) = mpsc::unbounded_channel();
    executor.set_status_channel(tx).await;
    let task_id = Uuid::new_v4();

    let execution_id = executor
        .execute(
            task_id,
            "ignored",
            Some(serde_json::json!({
                "task_file": task_file,
                "task_function": "run",
                "runner_path": runner,
                "python_path": base,
                "task_input": {"value": 123},
                "task_name": "task-a",
                "workflow_name": "wf-a",
                "run_id": task_id.to_string(),
            })),
        )
        .await
        .expect("execute process with task file");
    assert!(!execution_id.is_empty());

    let updates = collect_updates(&mut rx).await.expect("collect updates");
    let terminal = updates.last().expect("terminal update");
    assert_eq!(terminal.status, "success");
    let output = terminal.output.clone().expect("output payload");
    assert!(
        output["task_file"]
            .as_str()
            .unwrap_or("")
            .ends_with("task.py")
    );
    assert_eq!(output["task_function"], "run");
    assert_eq!(output["task_name"], "task-a");
    assert_eq!(output["workflow_name"], "wf-a");
    assert_eq!(output["run_id"], task_id.to_string());
    assert_eq!(output["input"], serde_json::json!({"value": 123}));
    let pythonpath = output["pythonpath"].as_str().unwrap_or("");
    let mut pythonpath_parts = std::env::split_paths(pythonpath);
    let first = pythonpath_parts
        .next()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_default();
    assert_eq!(first, base.to_string_lossy());

    let _ = fs::remove_dir_all(&base);
}

#[tokio::test]
async fn test_execute_process_python_runner_serializes_datetime_outputs() {
    let _guard = python_test_lock().lock().await;
    let base = std::env::temp_dir().join(format!("czar-process-test-{}", Uuid::new_v4()));
    fs::create_dir_all(&base).expect("create temp dir");
    write_pyproject(&base);

    let task_file = base.join("task_datetime.py");
    fs::write(
        &task_file,
        r#"
from datetime import date, datetime, time

def run():
    return {
        "dt": datetime(2024, 1, 2, 3, 4, 5, 123456),
        "d": date(2024, 1, 2),
        "t": time(3, 4, 5, 654321),
    }
"#,
    )
    .expect("write task file");
    let runner = base.join("run_task.py");
    fs::write(&runner, crate::python_runtime::RUN_TASK_PY).expect("write runner script");

    let executor = ProcessExecutor::new(Some(base.to_string_lossy().to_string()));
    let (tx, mut rx) = mpsc::unbounded_channel();
    executor.set_status_channel(tx).await;
    let task_id = Uuid::new_v4();

    let execution_id = executor
        .execute(
            task_id,
            "ignored",
            Some(serde_json::json!({
                "task_file": task_file,
                "task_function": "run",
                "runner_path": runner,
                "python_path": base,
                "task_name": "task-datetime",
                "workflow_name": "wf-datetime",
                "run_id": task_id.to_string(),
            })),
        )
        .await
        .expect("execute process with datetime output");
    assert!(!execution_id.is_empty());

    let updates = collect_updates(&mut rx).await.expect("collect updates");
    let terminal = updates.last().expect("terminal update");
    assert_eq!(terminal.status, "success");
    let output = terminal.output.clone().expect("output payload");
    assert_eq!(output["dt"], "2024-01-02T03:04:05.123456");
    assert_eq!(output["d"], "2024-01-02");
    assert_eq!(output["t"], "03:04:05.654321");

    let _ = fs::remove_dir_all(&base);
}

#[tokio::test]
async fn test_execute_process_with_uv_path_sets_cache_env_and_fails_cleanly() {
    let _guard = python_test_lock().lock().await;
    let base = std::env::temp_dir().join(format!("czar-process-test-{}", Uuid::new_v4()));
    fs::create_dir_all(&base).expect("create temp dir");
    write_pyproject(&base);

    let task_file = base.join("task.py");
    fs::write(&task_file, "def run(x: int) -> int:\n    return x\n").expect("write task file");
    let missing_runner = base.join("missing-runner.py");

    let executor = ProcessExecutor::new(Some(base.to_string_lossy().to_string()));
    let (tx, mut rx) = mpsc::unbounded_channel();
    executor.set_status_channel(tx).await;
    let task_id = Uuid::new_v4();

    let execution_id = executor
        .execute(
            task_id,
            "ignored",
            Some(serde_json::json!({
                "task_file": task_file,
                "runner_path": missing_runner,
                "python_path": base,
                "task_input": {"value": 1},
                "task_name": "uv-task",
            })),
        )
        .await
        .expect("execute process with uv path");
    assert!(!execution_id.is_empty());

    let updates = collect_updates(&mut rx).await.expect("collect updates");
    let terminal = updates.last().expect("terminal update");
    assert_eq!(terminal.status, "failed");
    assert!(terminal.output.is_none());

    let _ = fs::remove_dir_all(&base);
}

#[tokio::test]
async fn test_execute_process_python_runner_auto_bridges_tqdm_progress() {
    let _guard = python_test_lock().lock().await;
    let base = std::env::temp_dir().join(format!("czar-process-test-{}", Uuid::new_v4()));
    fs::create_dir_all(base.join("tqdm")).expect("create tqdm package");
    write_pyproject(&base);

    fs::write(
        base.join("tqdm").join("__init__.py"),
        r#"
class tqdm:
    def __init__(self, iterable=None, total=None, desc=None, **kwargs):
        self.iterable = iterable
        self.total = total if total is not None else (len(iterable) if iterable is not None else 0)
        self.n = 0
        self.desc = desc

    def __iter__(self):
        if self.iterable is None:
            return iter(())
        for item in self.iterable:
            self.update(1)
            yield item

    def update(self, n=1):
        self.n += n
        return self.n

    def refresh(self, *args, **kwargs):
        return None

    def close(self):
        return None


def trange(*args, **kwargs):
    return tqdm(range(*args), **kwargs)
"#,
    )
    .expect("write tqdm __init__");

    fs::write(
        base.join("tqdm").join("auto.py"),
        "from . import tqdm, trange\n",
    )
    .expect("write tqdm auto");

    let task_file = base.join("task_tqdm.py");
    fs::write(
        &task_file,
        r#"
def run():
    from tqdm import tqdm

    bar = tqdm(total=4, desc="phase")
    for _ in range(4):
        bar.update(1)
        bar.refresh()
    bar.close()

    return {"ok": True}
"#,
    )
    .expect("write task");

    let runner = base.join("run_task.py");
    fs::write(&runner, crate::python_runtime::RUN_TASK_PY).expect("write runner");

    let executor = ProcessExecutor::new(Some(base.to_string_lossy().to_string()));
    let (tx, mut rx) = mpsc::unbounded_channel();
    executor.set_status_channel(tx).await;
    let task_id = Uuid::new_v4();

    let execution_id = executor
        .execute(
            task_id,
            "ignored",
            Some(serde_json::json!({
                "task_file": task_file,
                "task_function": "run",
                "runner_path": runner,
                "python_path": base,
                "task_name": "task-tqdm",
            })),
        )
        .await
        .expect("execute process with tqdm task");
    assert!(!execution_id.is_empty());

    let updates = collect_updates(&mut rx).await.expect("collect updates");
    let progress_updates: Vec<&StatusUpdate> = updates
        .iter()
        .filter(|update| update.status == "progress")
        .collect();
    assert!(
        !progress_updates.is_empty(),
        "expected at least one progress update"
    );

    let final_progress = progress_updates
        .last()
        .and_then(|u| u.progress.as_ref())
        .expect("final progress payload");
    assert_eq!(final_progress["current"], serde_json::json!(4));
    assert_eq!(final_progress["total"], serde_json::json!(4));
    assert_eq!(final_progress["message"], serde_json::json!("phase"));

    let terminal = updates.last().expect("terminal update");
    assert_eq!(terminal.status, "success");
    assert_eq!(terminal.output, Some(serde_json::json!({"ok": true})));

    let _ = fs::remove_dir_all(&base);
}
