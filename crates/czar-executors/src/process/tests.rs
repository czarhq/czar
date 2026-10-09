use super::support::{build_python_command, resolve_command_path};
use super::*;
use std::fs;
use std::path::Path;
use std::time::Duration;
use tokio::sync::{Mutex, mpsc};
use tokio::time::timeout;

mod basics;
mod execution_core;
mod execution_python;

// Test-only lock: these Python executor tests shell out through uv/python and
// can flake when cargo runs them concurrently. Serialize them here rather than
// affecting any production executor behavior.
fn python_test_lock() -> &'static Mutex<()> {
    crate::python_runtime::python_runtime_test_lock()
}

fn write_pyproject(dir: &Path) {
    fs::write(
        dir.join("pyproject.toml"),
        "[project]\nname='czar-process-test'\nversion='0.1.0'\n",
    )
    .expect("write pyproject");
}

async fn collect_updates(
    rx: &mut mpsc::UnboundedReceiver<StatusUpdate>,
) -> anyhow::Result<Vec<StatusUpdate>> {
    let mut updates = Vec::new();
    let saw_terminal = timeout(Duration::from_secs(5), async {
        loop {
            let update = rx.recv().await.expect("status update");
            let terminal = update.status == "success" || update.status == "failed";
            updates.push(update);
            if terminal {
                break true;
            }
        }
    })
    .await
    .map_err(|_| anyhow::anyhow!("timed out waiting for process updates"))?;
    if saw_terminal {
        tokio::time::sleep(Duration::from_millis(50)).await;
        while let Ok(update) = rx.try_recv() {
            updates.push(update);
        }
    }
    Ok(updates)
}
