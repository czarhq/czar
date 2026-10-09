# Deployment Overview

_Updated at: 2026-05-22_

This page maps the deployment material that exists in the repo today.

## Examples Covered

1. [Local deployment example](../../examples/deployments/local/)

## Local

- [local.md](local.md): run Czar locally with `czar serve` (SQLite by default, or Postgres). This is the supported, working path.
- [Running Czar Locally](../dev/running.md): the Dockerized contributor stack (Postgres + API + React dev frontend).
- [Securing the API](api-auth.md): logging in when czar is reachable beyond localhost.

## Serverless / hosted

- [cloudflare.md](cloudflare.md): a Cloudflare Worker in front, `czar serve` in a Cloudflare container, D1 as the database ([`deploy/cloudflare/full`](../../deploy/cloudflare/full)).

## Not yet documented

- AWS and Azure deployment
- A generic self-hosted production guide

These remain future work until dedicated, accurate guides exist.
