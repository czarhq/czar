use super::*;

#[test]
fn test_compile_interpolates_string_constants_inside_task_inputs() {
    let dir = TempDir::new().expect("temp dir");
    let workflow: Workflow = serde_yaml::from_str(
        r#"
name: input-interpolation
vars:
  BUILD_PROFILE: debug
tasks:
  build:
    executor: process
    command: echo
    input_type:
      profile: str
    inputs:
      profile:
        const: "${{ vars.BUILD_PROFILE }}"
"#,
    )
    .expect("parse workflow");

    let compiled = workflow.compile(dir.path()).expect("compile workflow");
    assert_eq!(compiled.tasks[0].inputs["profile"]["const"], "debug");
}

#[test]
fn test_build_workflow_tasks_preserves_effective_workflow_vars_for_ref_bindings() {
    let dir = TempDir::new().expect("temp dir");
    let workflow: Workflow = serde_yaml::from_str(
        r#"
name: input-var-refs
vars:
  GREETING: hello
tasks:
  build:
    executor: process
    command: echo
    input_type:
      greeting: str
    inputs:
      greeting:
        ref: vars.GREETING
"#,
    )
    .expect("parse workflow");

    let compiled = workflow.compile(dir.path()).expect("compile workflow");
    let tasks = build_workflow_tasks(&compiled);
    assert_eq!(
        tasks[0].params.get("workflow_vars"),
        Some(&serde_json::json!({
            "GREETING": "hello",
            "platform.lib_ext": if cfg!(target_os = "macos") {
                "dylib"
            } else if cfg!(windows) {
                "dll"
            } else {
                "so"
            }
        }))
    );
}

#[test]
fn test_compile_interpolates_runtime_string_fields_consistently() {
    let dir = TempDir::new().expect("temp dir");
    let workflow: Workflow = serde_yaml::from_str(
        r#"
name: string-fields
vars:
  ENTRYPOINT: run_release
  JOB_NAME: release-job
  CONDITION_TASK: discover
tasks:
  discover:
    executor: process
    command: "echo discover"
    output_type:
      ready: bool
  process:
    executor: python
    file: task.py
    module: tasks.${{ vars.ENTRYPOINT }}
    function: ${{ vars.ENTRYPOINT }}
    depends_on: [discover]
    when: tasks.${{ vars.CONDITION_TASK }}.output.ready == true
  deploy:
    executor: cloudrun
    job: ${{ vars.JOB_NAME }}
hooks:
  notify:
    on: task.failed
    executor: process
    command: "echo ${{ vars.JOB_NAME }}"
"#,
    )
    .expect("parse workflow");

    write_file(
        &dir,
        "task.py",
        "def run_release(input: dict[str, object], ctx: object) -> dict[str, object]:\n    return {}\n",
    );
    write_pyproject(&dir);

    let compiled = workflow.compile(dir.path()).expect("compile workflow");
    let process = compiled
        .tasks
        .iter()
        .find(|task| task.name == "process")
        .expect("process task");
    let deploy = compiled
        .tasks
        .iter()
        .find(|task| task.name == "deploy")
        .expect("deploy task");

    assert_eq!(process.module.as_deref(), Some("tasks.run_release"));
    assert_eq!(process.function.as_deref(), Some("run_release"));
    assert_eq!(
        process.when.as_deref(),
        Some("tasks.discover.output.ready == true")
    );
    assert_eq!(deploy.job.as_deref(), Some("release-job"));
    assert_eq!(
        compiled.hooks[0].command.as_deref(),
        Some("echo release-job")
    );
}
