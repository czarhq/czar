# czar-store-file

_Updated at: 2026-05-22_

File-backed state implementation for local development and tests.

## Owns

- [`FileDatabase`](../../../crates/czar-store-file/src/lib.rs), implementing the `czar-core` repository traits.
- JSON storage under workflow, run, task, and workflow-task directories.
- File-backed task lifecycle, fanout, deferred job, and workflow state behavior.

## Notes

- This crate has no SQLx dependency.
- Applications that want the usual facade API can enable the `file` feature on `czar-store`.
