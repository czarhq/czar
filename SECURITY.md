# Security Policy

## Reporting a vulnerability

Please report security issues privately, not in a public issue or pull request.

Use GitHub's private vulnerability reporting on the [czarhq/czar](https://github.com/czarhq/czar) repository: open the **Security** tab and choose **Report a vulnerability**. Include the affected version or commit, how to reproduce the problem, and what an attacker could do with it. Follow-up happens in that private advisory.

## Supported versions

Czar is an early preview. Only the latest release receives security fixes.

## Security model

Anyone who can call the Czar API can run commands on the machine, because a workflow can run any shell command. `czar serve` therefore binds to `127.0.0.1:4000` by default, where it needs no login. To listen on any other address you must choose an `--auth` mode (`token` or `none`); see [Securing the API](docs/deployment/api-auth.md). Remote workers (`czar-worker`) also run arbitrary commands, so a worker reachable beyond localhost should run with `--auth-token` (or `CZAR_WORKER_AUTH_TOKEN`), with the same token set in the orchestrator's worker config; see [Remote Workers](docs/dev/remote-workers.md).
