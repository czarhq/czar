# czar-macros

_Updated at: 2026-09-29_

Proc-macro crate backing the `#[task]` attribute. Use it through the [czar](czar.md) facade.

## Owns

- [`#[task]`](../../../crates/czar-macros/src/lib.rs): turns a `fn(input)` or `fn(input, ctx)` into a task binary. It generates `main`, which reads typed input, builds `Context::from_env()`, calls the function, and writes the output.

## Notes

- `async fn` tasks are not supported yet.
- The return type may be `T` or `Result<T, E>`; errors exit the process with status 1.
