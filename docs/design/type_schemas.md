# Type Schemas

_Updated at: 2026-05-22_

<!-- Status: implemented in workflow validation, runtime output validation, Python contract checks, and schema export. -->

## Examples Covered

1. [Quickstart guide example](../../examples/workflows/quickstart-guide/)
2. [Workflow schema reference](../../examples/workflows/workflow-schema-reference/)

## Scope

Type schemas in Czar are declared directly in workflow YAML and enforced during workflow validation and task completion.

Current schema fields:

- workflow `types`
- task `input_type`
- task `output_type`

## Declaring Types

<!-- doc-snippet: path=examples/reference/future-type-schemas/11-current-declaring-types.yaml sha256=bd280dcf1dfe4f062d2fff092eebf647a204f76c53b46254731540b1a0a20fa3 lines=1-8 -->
```yaml
types:
  User:
    id: int
    email: str

  ExtractOutput:
    users: [types.User]
    count: int
```

Tasks reference these aliases through `types.<Alias>`:

<!-- doc-snippet: path=examples/reference/future-type-schemas/12-current-task-reference.yaml sha256=3ef2b1e293d491d9bc70091e6af182977e0d92332e69aa49c1c1cd03beb36da0 lines=1-5 -->
```yaml
tasks:
  extract:
    executor: python
    file: tasks_extract.py
    output_type: types.ExtractOutput
```

## Supported Builtins

- `str`
- `int`
- `float`
- `bool`
- `date`
- `datetime_notz`
- `datetime_tz`
- `datetime_utc`

Supported collection forms:

- object types: `{ field: <type> }`
- arrays: `[<type>]`
- shorthands: `list[<type>]`, `List[<type>]`, `array[<type>]`, `Array[<type>]`, `<type>[]`

## Important Current Rules

- `input_type` must resolve to an object type
- `inputs` must match the fields declared in `input_type`
- downstream `ref:` bindings are checked against upstream `output_type` when known
- successful task outputs are validated against declared `output_type` before the task is finalized
- legacy spellings such as `datetime`, `string`, `integer`, `boolean`, `T?`, and `output:` are rejected

## Runtime Output Validation

If a task declares `output_type`, a `success` result is only accepted when the returned JSON matches that schema.

That means:

- mismatched primitive or object shapes fail the task
- missing required output on a successful completion fails the task
- invalid outputs are not persisted as successful task output

## Validation Controls

Disable runtime output enforcement for one task:

<!-- doc-snippet: path=examples/reference/future-type-schemas/09-disable-validation-task.yaml sha256=a2716c89bae91081b94c09811999d8ae0a7c1060c8384c77ac96c44ec8146352 lines=1-6 -->
```yaml
tasks:
  extract:
    executor: python
    file: tasks/extract.py
    output_type: types.ExtractOutput
    validate_output: false
```

Disable contract validation for a whole project through `czar.yaml`:

<!-- doc-snippet: path=examples/reference/future-type-schemas/10-disable-validation-global.yaml sha256=cea773fc5a63aa5c75d9f9ce8205cbdb7c38c2e02bd14a56563124c4169c53f3 lines=1-3 -->
```yaml
# czar.yaml
validation:
  enabled: false
```

When project validation is disabled, Czar still parses workflows and enforces structural requirements such as executor fields and dependency integrity. The relaxed mode only skips type-contract validation.

## Python Contract Validation

For file-backed Python tasks, `czar validate-workflow` compares declared workflow contracts against function annotations.

That includes:

- task parameters versus `input_type`
- return annotation versus `output_type`
- ignoring `Context` parameters in the Python signature

## Schema Export

Inspect a Python task file:

```bash
czar schema export tasks_fetch.py
```

Current output shape:

<!-- doc-snippet: path=examples/reference/future-type-schemas/13-current-export-output.yaml sha256=9903e788eeb396c63aa1867067afeb26ff51e6eb5c8d1457102e3be9f5d993f2 lines=1-4 -->
```yaml
inputs:
  url: str
output:
  data: str
```

This command reports the inferred Python contract. You then copy the relevant pieces into workflow `input_type` and `output_type`.
