# czar-worker HTTP API

_Updated at: 2026-09-29_

All endpoints accept and return `application/json`. The worker binds on `--addr` (default `127.0.0.1:8081`).

If the worker was started with `--auth-token <TOKEN>`, `/compile` and `/execute` require `Authorization: Bearer <TOKEN>` and return `401` without it. `/health` needs no token.

---

## POST /health

Liveness check.

**Response**
```json
{ "status": "healthy", "service": "czar-worker" }
```

---

## POST /compile

Loads, validates, and compiles the worker's embedded `workflow.yaml`. Returns the task graph for the orchestrator to store.

**Request**: no body.

**Response**
```json
{
  "workflow": { },
  "tasks": [
    {
      "name": "task_name",
      "executor": "process",
      "depends_on": ["other_task"]
    }
  ]
}
```

`executor` is one of `container`, `process`, `python`, `cloudrun`, `library`, `wasm`, or a custom alias string.

---

## POST /execute

Runs a single task synchronously. Returns when the task finishes.

**Request**
```json
{
  "task_id": "uuid",
  "task_name": "task_name",
  "executor_type": "process",
  "params": { }
}
```

`executor_type`: same values as `executor` above.

`params`: executor-specific, see below.

**Response (success)**
```json
{
  "status": "success",
  "output": { },
  "deferred": [ ]
}
```

`deferred` is omitted when empty.

**Response (failure)**
```json
{
  "status": "failed",
  "error": "message"
}
```

### params by executor type

**process**
```json
{ "command": "echo hi", "env": { "KEY": "value" }, "secrets": [] }
```

**python**
```json
{ "task_file": "tasks/mytask.py", "python_path": "/optional/override" }
```

**library**
```json
{ "library_path": "tasks/lib.so" }
```

**wasm**
```json
{ "wasm_module_path": "tasks/app.wasm", "wasm_mode": "embedded" }
```

`wasm_mode` is `embedded` (default) or `cli`.

**cloudrun**: not supported; always returns a failure response.
