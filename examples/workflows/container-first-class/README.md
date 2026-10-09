# container First-Class Example

This example shows `executor: container` running a local Docker or Podman image as a normal Czar task.

What it includes:

- a host `process` task that produces typed output
- a first-class `container` task that depends on that output through `inputs`
- normal env injection and `CZAR_OUTPUT:` result parsing inside the container

Useful commands from this directory:

```bash
czar validate-workflow workflow.yaml
czar run workflow.yaml
```

Requirements:

- Docker or Podman installed locally
- network access the first time the runtime pulls `alpine:3.20`
