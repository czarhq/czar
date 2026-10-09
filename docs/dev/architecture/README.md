# How czar works

czar runs workflows: graphs of tasks written in YAML, where each task is a command, a Python function, a container, a Rust library or a call to an outside service. You deploy a workflow, start it by hand or on a schedule, and czar runs each task once the tasks it depends on are done, passing outputs along, retrying failures, and keeping every result in a database.

In one sentence: **the CLI and the API write rows, the Scheduler's loop turns rows into work, Executors run the work and report back, and the database holds everything in between.**

![Who talks to whom](diagrams/concepts-processes.png)

## Read in this order

1. **[Life of a run](flows/life-of-a-run.md):** one workflow from its file to a finished Run, in five phases. The quickest way to see all the parts working together.
2. **[Building blocks](building-blocks.md):** the parts (CLI, API, Compiler, Scheduler, Triggerer, Executors, Worker, Store), the processes they run in, what each owns, and the crates that hold them.
3. **[Domain model](domain-model.md):** the entities (Workflow, Snapshot, Run, Task, Deferred job, …), how they relate, who may change each one, and their lifecycles.
4. **[Concepts](concepts.md):** the rules every part relies on: the database as the only shared state, the Scheduler's tick, one Scheduler per database, reports instead of writes, pinned versions.
5. **[Flows](flows/README.md):** each thing czar does, step by step, with diagrams: define, start, schedule, execute (locally, on a Worker, on Cloud Run), deferred jobs, failures, control, observing, access.
6. **[Known issues](known-issues.md):** where the code doesn't yet do what these pages say it should.

Deeper reference for single parts: [storage](storage.md), [task SDKs](task-sdks.md), [the Worker API](../czar-worker-api.md), [crates](../crates.md), [schema](../schema.md).

## The words

These pages use one vocabulary: the building blocks are the participants in every diagram, and the entities are written with a capital letter (a Run, a Task). Both are defined in [writing.md](writing.md), which also says how to change these pages.
