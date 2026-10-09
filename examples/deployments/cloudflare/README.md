# Cloudflare Deployment

Run czar on Cloudflare: a Worker in front, `czar serve` in a Cloudflare container, and D1 as the database. The deployable project is [deploy/cloudflare/full](../../../deploy/cloudflare/full/); the walkthrough and its limits are in [docs/deployment/cloudflare.md](../../../docs/deployment/cloudflare.md).

## Usage

Run the Worker and container locally, with Wrangler and a local D1:

```bash
just up-cloudflare-full
just down-cloudflare-full
```
