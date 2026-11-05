# Afterimage

A self-hosted server for TRMNL e-paper displays. An e-paper screen keeps its last image with the power off, an afterimage of what it last showed.

See [the design](docs/design.md).

## Running

`nix build` produces `result/bin/afterimage`, which serves TRMNL's device API under `/api/` and the management API under `/manage/`. It listens on `AFTERIMAGE_BIND` (default `0.0.0.0:8080`) and keeps its database in `AFTERIMAGE_DATA_DIR` (default `./data`); `afterimage --help` lists every setting, each a flag or a `AFTERIMAGE_*` environment variable. Data sources read secrets from `AFTERIMAGE_SECRET_<NAME>` variables. In a device's Wi-Fi setup portal, set the server URL to this server's address.

## Development

`nix develop` provides every tool at its pinned version.

| Command | Does |
|---|---|
| `tools/generate` | Generates the contract into `gen/` from `model/smithy/`: both services' Rust server crates and OpenAPI documents, and the management API's TypeScript client |
| `cargo test --workspace` | Runs every test, the conformance test included |
| `tools/prepare-queries` | Refreshes `.sqlx/` after a query or migration changes |
| `tools/trmnl-excerpt` | Refreshes the excerpt of TRMNL's published device contract |
| `tools/trmnl-catalog` | Refreshes the snapshot of TRMNL's display models and palettes |
| `tools/tokens/refresh` | Refreshes the renderer's design tokens and fonts from the pinned release of TRMNL's design system |
| `nix flake check` | Builds the server from the pinned contract, runs every test, and runs clippy and rustfmt |
