# Securing the API

_Updated at: 2026-10-02_

## Examples Covered

1. [Local deployment example](../../examples/deployments/local/)

## In short

- On `127.0.0.1` (the default), no login is needed.
- On any other address, `czar serve` won't start until you pick a mode with `--auth`: usually a token.

```bash
CZAR_AUTH_TOKEN=$(openssl rand -hex 32) czar serve --addr 0.0.0.0:4000
```

Why: anyone who can call the API can run commands, since a workflow can run any shell command.

## Modes

| `--auth` | Use it when |
|---|---|
| `token` | czar is reachable from other machines: a VM, a container, your LAN |
| `none` | you've made sure nothing else can reach it; czar prints a warning at startup |

`--auth-token` (or `CZAR_AUTH_TOKEN`) picks `token` for you. The same options work for `czar dev` and `czar-web`.

## Logging in with a token

- The web UI asks for the token once and keeps a session cookie for 30 days. Changing the token logs every browser out.
- The TUI and scripts send `Authorization: Bearer <token>`.
