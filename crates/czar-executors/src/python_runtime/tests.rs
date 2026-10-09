use super::*;

#[tokio::test]
async fn test_get_run_task_script_writes_embedded_content() {
    let _guard = python_runtime_test_lock().lock().await;
    let temp_root =
        std::env::temp_dir().join(format!("czar-runtime-test-{}", uuid::Uuid::new_v4()));
    let prev = std::env::var("XDG_CACHE_HOME").ok();

    unsafe {
        std::env::set_var("XDG_CACHE_HOME", &temp_root);
    }

    let path = get_run_task_script().expect("get run_task.py");
    assert!(path.exists());
    let content = fs::read_to_string(&path).expect("read script");
    assert_eq!(content, RUN_TASK_PY);

    match prev {
        Some(v) => unsafe { std::env::set_var("XDG_CACHE_HOME", v) },
        None => unsafe { std::env::remove_var("XDG_CACHE_HOME") },
    }
    let _ = fs::remove_dir_all(temp_root);
}

#[tokio::test]
async fn test_get_inspect_task_script_writes_embedded_content() {
    let _guard = python_runtime_test_lock().lock().await;
    let temp_root =
        std::env::temp_dir().join(format!("czar-runtime-test-{}", uuid::Uuid::new_v4()));
    let prev = std::env::var("XDG_CACHE_HOME").ok();

    unsafe {
        std::env::set_var("XDG_CACHE_HOME", &temp_root);
    }

    let path = get_inspect_task_script().expect("get inspect_task.py");
    assert!(path.exists());
    let content = fs::read_to_string(&path).expect("read script");
    assert_eq!(content, INSPECT_TASK_PY);

    let second = get_inspect_task_script().expect("get inspect_task.py again");
    assert_eq!(second, path);

    match prev {
        Some(v) => unsafe { std::env::set_var("XDG_CACHE_HOME", v) },
        None => unsafe { std::env::remove_var("XDG_CACHE_HOME") },
    }
    let _ = fs::remove_dir_all(temp_root);
}

#[tokio::test]
async fn test_get_run_task_script_overwrites_stale_cached_content() {
    let _guard = python_runtime_test_lock().lock().await;
    let temp_root =
        std::env::temp_dir().join(format!("czar-runtime-test-{}", uuid::Uuid::new_v4()));
    let prev = std::env::var("XDG_CACHE_HOME").ok();

    unsafe {
        std::env::set_var("XDG_CACHE_HOME", &temp_root);
    }

    let path = get_run_task_script().expect("get run_task.py");
    fs::write(&path, "# stale script\n").expect("write stale content");

    let refreshed = get_run_task_script().expect("refresh run_task.py");
    assert_eq!(refreshed, path);
    let content = fs::read_to_string(&path).expect("read refreshed script");
    assert_eq!(content, RUN_TASK_PY);

    match prev {
        Some(v) => unsafe { std::env::set_var("XDG_CACHE_HOME", v) },
        None => unsafe { std::env::remove_var("XDG_CACHE_HOME") },
    }
    let _ = fs::remove_dir_all(temp_root);
}
