# Polyglot Workflow Example

This example demonstrates Python, native library, process, and both Wasm execution styles in one workflow.

Shared example setup and prerequisites live in [docs/guide/examples.md](../../../docs/guide/examples.md).

Extra requirements for this example:

- Rust targets:
  - `wasm32-unknown-unknown` for embedded/library-style wasm
  - `wasm32-wasip1` for CLI/WASI-style wasm
- Wasm host runtime for `wasm_mode: cli`:
  - `wasmtime` by default
  - or another runtime via `CZAR_WASM_RUNTIME`

Run it with:

```bash
just example-build polyglot
just example polyglot
```

Release artifacts can be selected without editing the workflow:

```bash
just example-build polyglot release
CZAR_VAR_BUILD_PROFILE=release just example polyglot
```

The workflow itself is in [polyglot.yaml](./polyglot.yaml). It expects prebuilt Rust artifacts, then runs:

- a host-native shared library via `executor: library`
- three native process binaries via `executor: process`
- an embedded/library-style wasm module via `executor: wasm`
- a CLI/WASI-style wasm module via `executor: wasm` with `wasm_mode: cli`

Rust sources live under [rust_tasks](./rust_tasks):

- embedded/library-style wasm and native library: [lib.rs](./rust_tasks/src/lib.rs)
- process tasks: [process_raw.rs](./rust_tasks/src/bin/process_raw.rs), [process_sdk.rs](./rust_tasks/src/bin/process_sdk.rs), and [process_macro.rs](./rust_tasks/src/bin/process_macro.rs)
- CLI/WASI-style wasm: [wasm_cli.rs](./rust_tasks/src/bin/wasm_cli.rs)
- build entrypoint: [build.py](./build.py)
