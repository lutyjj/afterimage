# Afterimage

A self-hosted server for TRMNL e-paper displays. An e-paper screen keeps its last image with the power off, an afterimage of what it last showed.

See [the design](docs/design.md).

## Running

`nix build` produces `result/bin/afterimage`, which serves TRMNL's device API under `/api/`, the management API under `/manage/` and the web UI everywhere else. It listens on `AFTERIMAGE_BIND` (default `0.0.0.0:8080`), keeps its database in `AFTERIMAGE_DATA_DIR` (default `./data`) and the firmware images it serves in `AFTERIMAGE_CACHE_DIR` (default `cache` in the data directory), and fetches firmware from `https://trmnl.com` and the images it lists; `afterimage --help` lists every setting, each a flag or a `AFTERIMAGE_*` environment variable. Data sources read secrets from `AFTERIMAGE_SECRET_<NAME>` variables. With `AFTERIMAGE_MQTT_URL` (and `AFTERIMAGE_MQTT_USERNAME` and `AFTERIMAGE_MQTT_PASSWORD` where the broker asks), it publishes every claimed device to Home Assistant through MQTT discovery. In a device's Wi-Fi setup portal, set the server URL to this server's address.

On NixOS, the flake's `nixosModules.default` runs it as `services.afterimage`: `settings` takes the `AFTERIMAGE_*` variables, `environmentFile` the secrets (`AFTERIMAGE_OIDC_CLIENT_ID`, `AFTERIMAGE_OIDC_CLIENT_SECRET`, `AFTERIMAGE_MQTT_PASSWORD` and the `AFTERIMAGE_SECRET_*` values), the database lives in the service's state directory and firmware images in its cache directory.

## Development

`nix develop` provides every tool at its pinned version.

| Command | Does |
|---|---|
| `tools/generate` | Generates the contract into `gen/` from `model/smithy/`: both services' Rust server crates and OpenAPI documents, and the management API's TypeScript client |
| `cargo test --workspace` | Runs every Rust test, the conformance test included |
| `pnpm install` | Installs the web UI's dependencies, after `tools/generate` |
| `pnpm --filter afterimage-web dev` | Serves the web UI with hot reload at `http://127.0.0.1:5173`, sending `/api` and `/manage` to a server on port 8080 |
| `pnpm --filter afterimage-web check` | Type-checks the web UI with the generated client and runs its tests |
| `tools/prepare-queries` | Refreshes `.sqlx/` after a query or migration changes |
| `tools/trmnl-excerpt` | Refreshes the excerpt of TRMNL's published device contract |
| `tools/trmnl-catalog` | Refreshes the snapshot of TRMNL's display models and palettes |
| `tools/tokens/refresh` | Refreshes the renderer's design tokens and fonts from the pinned release of TRMNL's design system |
| `nix flake check` | Builds the server and the web UI from the pinned contract, runs every test, runs clippy and rustfmt, and boots the NixOS module in a VM |
