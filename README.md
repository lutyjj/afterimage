# Afterimage

A self-hosted server for TRMNL e-paper displays. An e-paper screen keeps its last image with the power off, an afterimage of what it last showed.

See [the design](docs/design.md).

## Running

`nix build` produces `result/bin/afterimage`, which serves TRMNL's device API. It listens on `AFTERIMAGE_BIND` (default `0.0.0.0:8080`) and keeps its database in `AFTERIMAGE_DATA_DIR` (default `./data`). In a device's Wi-Fi setup portal, set the server URL to this server's address.

## Development

`nix develop` provides every tool at its pinned version.

| Command | Does |
|---|---|
| `tools/generate` | Generates the contract into `gen/` from `model/smithy/` |
| `cargo test --workspace` | Runs every test, the conformance test included |
| `tools/prepare-queries` | Refreshes `.sqlx/` after a query or migration changes |
| `tools/trmnl-excerpt` | Refreshes the excerpt of TRMNL's published device contract |
| `nix flake check` | Builds the server from the pinned contract, runs every test, and runs clippy and rustfmt |
