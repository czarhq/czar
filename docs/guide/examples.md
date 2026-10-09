# Examples Guide

_Updated at: 2026-05-22_

This is the single place for example workflow prerequisites and run instructions.

## Examples Covered

1. [polyglot](../../examples/workflows/polyglot/)
2. [deferrables](../../examples/workflows/deferrables/)
3. [simple_args](../../examples/workflows/simple_args/)

## Running Examples

Use the shared recipe:

```bash
just example <name>
```

Example:

```bash
just example polyglot
```

The recipe will:

- initialize the local SQLite database if needed
- use an isolated SQLite database under `.czar/examples/<name>/`
- prepare Python environments for examples that ship a `pyproject.toml`
- keep each example's Python environment isolated in its own `.venv`
- execute the example workflow through the normal `czar` CLI path

Some examples also ship a separate build step:

```bash
just example-build <name>
```

To see which examples are safe to run automatically and which are validate-only:

```bash
just examples-list-all
```

Policy:

- runnable examples are local and deterministic enough to execute as part of the example matrix
- deployment examples that could touch external infrastructure are validate-only by default
- docs-reference bundles are validate-only and checked through the docs/snippet lint path

To run the full safe example matrix:

```bash
just examples-check-all
```

That command validates every example in the manifest and only executes those marked runnable.

## Common Prerequisites

Most workflow examples assume:

- `just`
- Python 3.12
- Rust toolchain
- `uv` for Python-backed examples

Check installed tools with:

```bash
command -v just
command -v uv
python3 --version
rustc --version
```

## Extra Requirements By Example

| Example | Path | Extra requirements beyond the common prerequisites |
|---|---|---|
| `container-first-class` | `examples/workflows/container-first-class` | Docker or Podman installed locally; network access the first time the runtime pulls `alpine:3.20` |
| `custom-executor` | `examples/workflows/custom-executor` | None |
| `dbt-first-class` | `examples/workflows/dbt-first-class` | None; the example bundles a minimal `dbt` CLI shim in `bin/dbt` and only needs the common Python prerequisites |
| `polyglot` | `examples/workflows/polyglot` | Rust targets `wasm32-unknown-unknown` and `wasm32-wasip1`; a Wasm runtime host such as `wasmtime` for `wasm_mode: cli`; prebuild the example artifacts with `just example-build polyglot` |
| `deferrables` | `examples/workflows/deferrables` | None for the simulated demo workflow |
| `simple_args` | `examples/workflows/simple_args` | None |

Examples without a row here do not currently need anything beyond the common prerequisites.

## Wasm Example Requirements

For examples that compile and run Wasm tasks, install:

```bash
rustup target add wasm32-unknown-unknown
rustup target add wasm32-wasip1
```

And install a Wasm runtime host for CLI/WASI-style execution. The current shared executor defaults to `wasmtime`:

```bash
wasmtime --version
```

If you want to use a different runtime binary, set:

```bash
export CZAR_WASM_RUNTIME=/path/to/your/runtime
```

## Notes

- Python examples rely on `uv`, and the repo wrapper applies the current `exclude-newer` policy when `just example` sources `scripts/prepare-example-env.sh`.
- `just example-build` is the place for example-specific build steps; workflow tasks themselves should not compile deployable artifacts at runtime.
- The polyglot example demonstrates both wasm modes:
  - embedded/library-style wasm via `wasm32-unknown-unknown`
  - CLI/WASI-style wasm via `wasm32-wasip1`
- Override workflow vars during example runs with `CZAR_VAR_<NAME>=...`, for example `CZAR_VAR_BUILD_PROFILE=release just example polyglot`.
