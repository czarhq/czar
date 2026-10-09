# Writing the architecture docs

These pages are living docs: they describe the code on this branch and change in the same pull request as the code. They are the only description of how czar works; there is no second copy elsewhere.

## One vocabulary

Every page uses the same names for the same things. When a page needs something that isn't here, add it here first.

### Building blocks

These are the participants in every diagram. Use the exact name and, in sequence diagrams, the alias in brackets.

| Name (alias) | What it is | Code |
|---|---|---|
| User (`User`) | A person: a workflow author or an operator | |
| CLI (`CLI`) | The `czar` command | `czar-cli` |
| Server (`Server`) | The `czar serve` process itself, for what it does on its own: startup, and timers such as Worker sync | `czar-cli`: `commands/serve` |
| Web UI (`UI`) | The React app in the browser | `czar-web-react` |
| TUI (`TUI`) | The terminal client | `czar-tui` |
| API (`API`) | The HTTP API that serves the clients | `czar-web` |
| Compiler (`Compiler`) | Turns workflow YAML into workflow tasks and a snapshot | `czar-core`: `workflow`, `compiled`, `services::workflow_yaml` |
| Scheduler (`Scheduler`) | The loop that advances runs and tasks | `czar-core`: `scheduler` |
| Triggerer (`Triggerer`) | Polls deferred jobs | `czar-core`: `triggerer`, trackers in `czar-gcp` |
| Executor (`Executor`) | Starts a task's work and reports on it. Chosen per Task from its `executor` value (`process`, `container` and custom executors share one) or, for a Workflow synced from a Worker, always the Worker executor | `czar-executors` |
| Worker (`Worker`) | A remote worker: an HTTP server next to the task code | `czar-worker` |
| Task process (`Proc`) | The task's own code, using an SDK | user code, `czar` crate, `czar` Python package |
| Store (`Store`) | The database: SQLite, Postgres, D1 or files | `czar-store*` |
| Job service (`Service`) | Outside work a task hands off: BigQuery, Dataproc, Cloud Run, an HTTP endpoint | |
| Notifier (`Notify`) | Slack webhook or email | `czar-core`: `notifications` |

Never use a function name as a participant. Functions go in the "In the code" section.

### Entities

The things czar stores. Write them capitalised in prose (a Run, the Task) so a reader can tell the entity from the everyday word. Each links to its definition in [domain-model.md](domain-model.md).

| Entity | Table | One sentence |
|---|---|---|
| Workflow | `workflows` | A named workflow and its schedule; points at its current Snapshot |
| Workflow task | `workflow_tasks` | One compiled task of the Workflow's current version |
| Snapshot | `workflow_snapshots` | An immutable copy of a Workflow's tasks; every Run pins one |
| Run | `runs` | One execution of a Workflow (or part of it) |
| Task | `tasks` | One task of one Run: status, attempts, output, logs |
| Deferred job | `deferred_jobs` | Outside work a Task waits on |
| Task state | `workflow_task_states` | The last result of a Workflow task, across Runs |
| Worker registration | config file, in memory; the URL is stamped on the synced Workflow | A remote worker's URL and token; tokens match on the exact URL |
| Hook | rows named `__hook__:*` in Workflow tasks and the Snapshot | A task started on a Run or Task event; fires from the Run's Snapshot |

### Status words

Statuses are written in code style: `pending`, `running`. Their meaning and every transition are in [domain-model.md](domain-model.md#lifecycles); a flow page says which transitions it causes, not what the statuses mean.

## A flow page

Every page under [flows/](flows/README.md) has the same parts, in this order:

1. **Title and one paragraph:** what starts the flow, what it ends with, and where it sits in the [life of a run](flows/life-of-a-run.md).
2. **At a glance:** a table with these rows:
   - Starts: who or what starts it
   - Blocks: the building blocks taking part
   - Reads and writes: the entities touched
   - Ends: the end state
3. **The diagram:** a sequence diagram with the building blocks as participants. Use `alt`, `opt`, `loop` and `par` where the flow branches, repeats or runs in parallel; a flow is rarely a straight line. Use `rect` to mark phases that have their own page, with a note naming that page.
4. **State changes:** a table with Entity, From, To and Caused by, for every transition the flow makes. It must agree with the state diagrams in [domain-model.md](domain-model.md).
5. **Rules:** the decisions the flow makes. Write them as rules ("a Task is ready when …"), not steps. Use a flowchart when a decision has more than two outcomes.
6. **Failure and edge cases:** what goes wrong and what the user sees. Known bugs link to [known-issues.md](known-issues.md).
7. **In the code:** the functions and files, as relative links, in the order the flow uses them.
8. **Related:** the parent flow, the child flows, the neighbours.

## Diagrams

Each diagram is a `<page>-<what>.diagram.yaml` file in [diagrams/](diagrams/): boxes and arrows (with product icons, decisions and states) or a sequence of messages. They are drawn by [in-ctrl](https://github.com/gajop/in-ctrl), which documents the format; the shared names (`is: Scheduler`) are in [in-ctrl.config.yaml](../../../in-ctrl.config.yaml). A page shows the PNG rendered next to it:

```markdown
![The life of a run](diagrams/life-of-a-run.png)
```

So the Markdown stays readable on its own, GitHub shows the picture, and the [docs site](https://czarhq.github.io/czar/) draws the YAML itself, interactive and in light or dark. After changing a diagram, run `just docs-diagrams`; `just docs-check` fails when a PNG is missing or older than its source.

Keep each diagram to one idea a reader takes in at a glance:

- **Size:** a sequence has at most about 12 messages and one level of frames inside frames; a graph has about 12 nodes or fewer.
- **Hierarchy:** when a flow is bigger, draw an overview with the parts as boxes (each with a `link` to its page), then one small diagram per part under its own heading, each introduced by a sentence. Branches belong on the part's page, not in the overview.
- **Same names, same icons:** participants and nodes that are building blocks say `is: Scheduler` (or `Store`, `Executor`, …), so they look the same in every diagram.

## Style

- Markdown is not hard-wrapped: one line per paragraph or list item.
- Short sentences, everyday words. A concrete example beats a definition.
- Link code with relative links (`../../../crates/czar-core/src/scheduler.rs`).
- Say what is true now. Plans go in the roadmap, bugs in [known-issues.md](known-issues.md).
