# dbt

_Updated at: 2026-05-22_

Czar can import a dbt project and expand runnable dbt nodes into ordinary workflow tasks.

## Examples Covered

1. [dbt first-class workflow example](../../examples/workflows/dbt-first-class/)
2. [dbt reference snippets](../../examples/reference/future-dbt/)

The imported nodes are first-class Czar tasks:

- models
- tests
- seeds
- snapshots

The graph keeps dbt dependencies, supports normal Czar task dependencies around the dbt subgraph, and exposes generated flows/task groups for project-level and path-level execution.

## Example

See the runnable example in [../../examples/workflows/dbt-first-class/](../../examples/workflows/dbt-first-class/). That example vendors a tiny `bin/dbt` shim so it can stay runnable under the repo's license policy, while the product integration still targets the ordinary dbt CLI contract.

## Basic Import

<!-- doc-snippet: path=examples/workflows/dbt-first-class/workflow.yaml sha256=f09b9412de1d1c8f9126b7d40b6abb6879f5102372a8c7e8576ebfbc9085612e lines=1-23 -->
```yaml
name: dbt_first_class

imports:
  - dbt: transform
    label: analytics
    profiles_dir: profiles
    profile: czar_example
    target: dev
    depends_on: [prepare_seed]

tasks:
  prepare_seed:
    executor: python
    file: tasks.py
    function: prepare_seed

  publish_report:
    executor: python
    file: tasks.py
    function: publish_report
    depends_on:
      - dbt.analytics.marts.daily_revenue
      - dbt.analytics.test.assert_daily_revenue
```

`imports:` also accepts the singular alias `import:`.

## Naming

Imported task names are namespaced by the import label.

- models: `dbt.<label>.<path>`
- seeds: `dbt.<label>.seed.<path>`
- tests: `dbt.<label>.test.<name>`
- snapshots: `dbt.<label>.snapshot.<path>`

Examples from the runnable example:

- `dbt.analytics.seed.raw_orders`
- `dbt.analytics.staging.stg_orders`
- `dbt.analytics.marts.daily_revenue`
- `dbt.analytics.test.assert_daily_revenue`

## Generated Flows And Groups

Each import generates selection helpers as both flows and task groups.

- `dbt.analytics.all`
- `dbt.analytics.path.staging`
- `dbt.analytics.path.marts`
- `dbt.analytics.resource.models`
- `dbt.analytics.resource.tests`
- `dbt.analytics.resource.seeds`

That lets you run the whole imported project or a subdirectory while still keeping each dbt node visible as its own task.

## Import Options

Supported dbt import fields:

- `dbt`: project directory
- `label`: namespace prefix for generated task names
- `select`: selector list for partial imports
- `models`: alias for selector-style partial imports
- `include_models`
- `include_tests`
- `include_seeds`
- `include_snapshots`
- `depends_on`: extra upstream Czar tasks added to imported dbt root nodes
- `profiles_dir`
- `profile`
- `target`
- `threads`

If `target/manifest.json` is missing, Czar runs `dbt parse` during workflow load to build it.

## Overrides

You can add lightweight overrides to imported dbt tasks by declaring the generated task name under `tasks:`.

Supported override fields:

- `depends_on`
- `timeout`
- `retries`
- `when`

Example:

<!-- doc-snippet: path=examples/reference/future-dbt/08-current-override.yaml sha256=7c5905d0b5d15b003be335a98d68295d2943ec5d1d8e45136508c56b3f789090 lines=1-3 -->
```yaml
tasks:
  dbt.analytics.marts.daily_revenue:
    depends_on: [audit]
```

Imported dbt tasks cannot override their executor or implementation fields.

## Execution Model

Imported dbt nodes run through the shared Python helper `czar.dbt.run_node`.

For each imported task, Czar runs the dbt CLI for that node only:

- models: `dbt run`
- tests: `dbt test`
- seeds: `dbt seed`
- snapshots: `dbt snapshot`

Each task gets its own `target_path`, so artifacts and run results stay isolated per node.
