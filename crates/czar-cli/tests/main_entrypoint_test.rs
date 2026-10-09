use std::process::Command;

#[test]
fn test_czar_binary_help_succeeds() {
    let output = Command::new(env!("CARGO_BIN_EXE_czar"))
        .arg("--help")
        .output()
        .expect("run czar --help");
    assert!(output.status.success(), "stdout: {:?}", output.stdout);
}

#[test]
fn test_czar_binary_validate_workflow_missing_file_fails_before_db_connect() {
    let output = Command::new(env!("CARGO_BIN_EXE_czar"))
        .args(["validate-workflow", "/tmp/does-not-exist-workflow.yaml"])
        .output()
        .expect("run czar validate-workflow");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Failed to load workflow source"),
        "stderr was: {stderr}"
    );
}

#[cfg(unix)]
#[test]
fn test_czar_binary_validates_a_dbt_import_from_the_workflow_folder() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::TempDir::new().expect("temp dir");
    let root = dir.path();
    std::fs::create_dir_all(root.join("transform")).expect("create dbt project");
    std::fs::create_dir_all(root.join("bin")).expect("create bin");
    // A stand-in for dbt: `dbt parse --project-dir DIR` writes DIR/target/manifest.json.
    let dbt = root.join("bin").join("dbt");
    std::fs::write(
        &dbt,
        "#!/bin/sh\nmkdir -p \"$3/target\"\necho '{\"metadata\":{\"project_name\":\"analytics\"},\"nodes\":{}}' > \"$3/target/manifest.json\"\n",
    )
    .expect("write dbt");
    std::fs::set_permissions(&dbt, std::fs::Permissions::from_mode(0o755)).expect("chmod dbt");
    std::fs::write(
        root.join("workflow.yaml"),
        "name: dbt_from_here\nimports:\n  - dbt: transform\n    label: analytics\ntasks:\n  extract:\n    executor: process\n    command: echo extract\n",
    )
    .expect("write workflow");

    // `czar validate-workflow workflow.yaml`, run from the workflow's folder.
    let output = Command::new(env!("CARGO_BIN_EXE_czar"))
        .args(["validate-workflow", "workflow.yaml"])
        .current_dir(root)
        .output()
        .expect("run czar validate-workflow");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr was: {stderr}");
    assert!(root.join("transform/target/manifest.json").exists());
}
