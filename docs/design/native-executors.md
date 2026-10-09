# Native Executors

_Updated at: 2026-05-22_

This page documents the current local/native executor model in Czar: `process`, `library`, and `wasm`.

## Examples Covered

1. [polyglot](../../examples/workflows/polyglot/)

## Overview

| Executor | Artifact style | Typical target | Runtime model |
|---|---|---|---|
| `process` | Native executable or shell command | Host target | Spawn a local process |
| `library` | Host shared library | Host target | In-process host call through the library ABI |
| `wasm` | WebAssembly module | `wasm32-unknown-unknown` or `wasm32-wasip1` | Either embedded in-process or external CLI runtime |

The polyglot example at [examples/workflows/polyglot/polyglot.yaml](../../examples/workflows/polyglot/polyglot.yaml) demonstrates all of these paths.

## Process Executor

Use `executor: process` for normal binaries or shell commands.

<!-- doc-snippet: path=examples/workflows/polyglot/docs/process-executor.yaml sha256=ddbb8d5a43e0bb1ce7dcad0954020aed68c40a022cd5b27d219cecdc31d27d8c lines=1-4 -->
```yaml
rust_process_sdk:
  executor: process
  # Process executor: native binary that uses czar::run_task.
  command: rust_tasks/target/${{ vars.BUILD_PROFILE }}/process_sdk
```

Behavior:

- `command` is resolved relative to the workflow root when it looks like a path.
- Commands without path separators such as `bash`, `cargo`, or `python` are left alone and resolved by normal `PATH` lookup.
- On Windows, path-like commands without an extension are resolved with `.exe`.
- `args` is passed as structured arguments instead of forcing everything into one shell string.
- Input is provided via `CZAR_INPUT_JSON`.
- Output is read from stdout via the `CZAR_OUTPUT:<json>` convention.

## Library Executor

Use `executor: library` for host-native shared libraries produced from Rust or another language that can expose the Czar library ABI.

<!-- doc-snippet: path=examples/workflows/polyglot/docs/library-executor.yaml sha256=352615ee5aa8f24522e443f79d4e1007fe6687958c7e5403be6c8248df5b7569 lines=1-5 -->
```yaml
rust_library_native:
  executor: library
  # Native library executor: host-specific shared library with direct in-process calls.
  # Build these artifacts ahead of time with `just example-build polyglot`.
  file: rust_tasks/target/${{ vars.BUILD_PROFILE }}/libpolyglot_rust_lib.${{ platform.lib_ext }}
```

Behavior:

- The library is loaded in-process on the host.
- Rust tasks typically export the ABI with `czar::czar_task_library!(my_function)`.
- If the declared library path has no extension, Czar appends the platform extension automatically.
- `platform.lib_ext` is also available as a built-in interpolation variable when you want the extension explicit in YAML.

Platform mapping:

- Linux: `so`
- macOS: `dylib`
- Windows: `dll`

## Wasm Executor

`executor: wasm` supports two execution styles.

### Embedded / Library-Style Wasm

This is the default when `wasm_mode` is omitted.

<!-- doc-snippet: path=examples/workflows/polyglot/docs/wasm-embedded.yaml sha256=0684690b270bc77b6763f2cba563958c88c7a5cbceb9ede0a6190cb62de42cf0 lines=1-4 -->
```yaml
rust_wasm_embedded:
  executor: wasm
  # Embedded / library-style wasm: in-process via wasmtime and exported functions.
  file: rust_tasks/target/wasm32-unknown-unknown/${{ vars.BUILD_PROFILE }}/polyglot_rust_lib.wasm
```

Behavior:

- Runs in-process through wasmtime embedding.
- Uses the wasm ABI exported by `czar::czar_task_wasm!(my_function)`.
- Recommended target: `wasm32-unknown-unknown`.
- Requires the `wasm-embedded` feature in `czar-executors`.

### CLI / WASI-Style Wasm

<!-- doc-snippet: path=examples/workflows/polyglot/docs/wasm-cli.yaml sha256=ee336d066358cc8eac4806736419611738e3ddd23eb8b0f2ef6e03956c323060 lines=1-5 -->
```yaml
rust_wasm_cli:
  executor: wasm
  # CLI / WASI-style wasm: spawned through an external runtime such as `wasmtime run`.
  file: rust_tasks/target/wasm32-wasip1/${{ vars.BUILD_PROFILE }}/wasm_cli.wasm
  wasm_mode: cli
```

Behavior:

- Spawns an external wasm runtime such as `wasmtime run`.
- Recommended target: `wasm32-wasip1`.
- Input and output follow the same `CZAR_INPUT_JSON` / `CZAR_OUTPUT:` contract as `process`.
- The runtime binary defaults to `wasmtime` and can be overridden with `CZAR_WASM_RUNTIME`.

## Variables and Build Profiles

Native executor paths support interpolation through the workflow `vars` block:

<!-- doc-snippet: path=examples/workflows/polyglot/polyglot.yaml sha256=4346a90ba8276c4b76a15309d52fa56b8bfadff3aa16412de1ecb2f05d6f8c67 lines=7-8 -->
```yaml
vars:
  BUILD_PROFILE: debug
```

The process, library, and wasm task snippets above all use `${{ vars.BUILD_PROFILE }}` in their artifact paths.

Interpolated strings inside task inputs are also resolved at compile time.

Override precedence:

1. Workflow `vars`
2. `CZAR_VAR_<NAME>` environment variables
3. `czar run --var NAME=VALUE`

Interpolation is applied consistently across runtime string fields, not just native executor artifact paths. That includes hook `command`/`file` fields and task string constants inside `inputs`.

Example:

<!-- doc-snippet: path=examples/workflows/polyglot/README.md sha256=90ebe7f6da1bb1573dbe8faf75765a9a6667d4ec83a5a2dab34bb553ee6e79c7 lines=26-27 -->
```bash
just example-build polyglot release
CZAR_VAR_BUILD_PROFILE=release just example polyglot
```

Workflow-relative path rule:

- Relative `file:` paths and path-like `command:` values are resolved from the workflow YAML directory.
- This keeps workflows relocatable and consistent across `czar run`, `czar load`, and worker execution.

## Build Staging

Process, library, and wasm tasks no longer require their artifact paths to exist during workflow compilation. That allows local build-then-run workflows when you explicitly want them.

That flexibility should not be the default example pattern for deployable workflows. Native artifacts are usually built ahead of time by the user or their build system, then referenced from workflow YAML. The polyglot example now follows that prebuilt-artifact model and keeps build orchestration outside task execution.

## Platform Notes

- Linux and macOS are fully covered by the current polyglot example.
- Windows now resolves library extensions and path-like process binaries correctly, but any custom shell commands still need a Windows-compatible command runner.
- The polyglot example keeps its build logic out of workflow execution; use `just example-build polyglot [profile]` or your own build pipeline to produce the referenced artifacts.

## Feature Placement

Feature gating for local native execution should be enabled through the `local-executors` alias rather than repeating `process`, `library`, and `wasm-embedded` everywhere.

Current aliases:

- `czar-cli`: `local-executors = ["process", "library", "wasm-embedded"]`
- `czar-worker`: `local-executors = ["process", "library", "wasm-embedded"]`

Use the alias in install/dev scripts and installation docs so local-native execution stays enabled from one place.
