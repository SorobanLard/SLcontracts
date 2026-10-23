# Deployments

This file records the SorobanLard contract deployments for each environment so
that the backend and frontend repos can point at a known, verified contract ID
instead of passing addresses around informally.

> **Runbook:** update this file as part of every deployment. The `deploy`
> Makefile target prints the contract ID and Wasm hash — copy them into the
> table below and add a row to the changelog before opening the PR.

## Environments

| Network | Contract ID | Wasm hash | Deployed | Notes |
|---------|-------------|-----------|----------|-------|
| testnet | _not yet deployed_ | — | — | Fill in after `make deploy` |
| mainnet | _not yet deployed_ | — | — | Fill in after `make deploy` |

## Changelog

Record which Wasm hash/version is currently live per network. Newest first.

| Date | Network | Wasm hash | Contract ID | Change |
|------|---------|-----------|-------------|--------|
| — | — | — | — | Initial record pending first deployment |

## How to record a deployment

1. Build the optimized Wasm: `make build`.
2. Deploy: `make deploy` (see the `deploy` target in the `Makefile`).
3. Copy the resulting **contract ID** and **Wasm hash** into the table above.
4. Add a row to the changelog with the date and a short description of the change.
5. Commit `DEPLOYMENTS.md` alongside the deployment PR.
