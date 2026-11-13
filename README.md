# Afterimage

A self-hosted server for TRMNL e-paper displays. An e-paper screen keeps its last image with the power off, an afterimage of what it last showed. [The design](docs/design.md) explains how it works.

## Run it

`nix build` produces `result/bin/afterimage`. In a display's Wi-Fi setup portal, set the server URL to its address.

| Path | Serves |
|---|---|
| `/api/` | TRMNL's device API, for the displays |
| `/manage/` | The management API, for the web UI, scripts and agents |
| `/mcp` | The MCP endpoint, over streamable HTTP |
| Everything else | The web UI |

Scripts and agents act with a personal access token, made in the web UI's settings and sent as `Authorization: Bearer <token>`. `tools/generate` writes the management API's OpenAPI document to `gen/management-api.openapi.json`.

## Settings

Each setting is an environment variable or the flag `afterimage --help` lists beside it.

| Variable | Default | Sets |
|---|---|---|
| `AFTERIMAGE_BIND` | `0.0.0.0:8080` | The address to listen on; displays must reach it |
| `AFTERIMAGE_DATA_DIR` | `data` | The database's directory |
| `AFTERIMAGE_CACHE_DIR` | `cache` in the data directory | Files the server can fetch again, such as firmware images |
| `AFTERIMAGE_URL` | | The address browsers use, such as `https://afterimage.example`. OIDC needs it; an `https` address makes cookies `Secure` |
| `AFTERIMAGE_WEB_DIR` | | The built web UI |
| `AFTERIMAGE_TIME_ZONE` | The host's | The time zone of schedules and of dates on screens, such as `Europe/Berlin` |
| `AFTERIMAGE_TRMNL_URL` | `https://trmnl.com` | TRMNL's site, which publishes the firmware displays install |
| `AFTERIMAGE_PASSWORD_SIGN_IN` | `true` | Whether people may sign in with a password; `false` needs OIDC |
| `AFTERIMAGE_OIDC_ISSUER` | | The OIDC provider. Its client allows the redirect `<AFTERIMAGE_URL>/manage/auth/oidc/callback` |
| `AFTERIMAGE_OIDC_CLIENT_ID`, `AFTERIMAGE_OIDC_CLIENT_SECRET` | | The OIDC client |
| `AFTERIMAGE_OIDC_NAME` | `Single sign-on` | What the sign-in button calls the provider |
| `AFTERIMAGE_MQTT_URL` | | The broker that carries every display to Home Assistant, `mqtt://` or `mqtts://`. Unset, the first account sets it in the web UI |
| `AFTERIMAGE_MQTT_USERNAME`, `AFTERIMAGE_MQTT_PASSWORD` | | The broker's credentials |
| `AFTERIMAGE_MQTT_TOPIC` | `afterimage` | The topic every display publishes under |
| `AFTERIMAGE_MQTT_DISCOVERY_PREFIX` | `homeassistant` | Home Assistant's discovery prefix |
| `AFTERIMAGE_SECRET_<NAME>` | | A secret data sources read as `secrets.<NAME>` |

## NixOS

The flake's `nixosModules.default` runs the server as `services.afterimage`, with the database in the service's state directory and firmware images in its cache directory.

| Option | Default | Sets |
|---|---|---|
| `listenAddress`, `port` | `0.0.0.0`, `8080` | Where the server listens |
| `openFirewall` | `false` | Opens `port` for clients that reach the server without a reverse proxy |
| `settings` | `{ }` | The `AFTERIMAGE_*` variables. The module sets the bind address, data and cache directories, and the time zone from `time.timeZone` |
| `environmentFile` | | A file of secrets: the OIDC client, the MQTT password and the `AFTERIMAGE_SECRET_*` values |

## Development

`nix develop` provides every tool at its pinned version.

| Command | Does |
|---|---|
| `tools/generate` | Generates `gen/`: from `model/smithy/` both services' Rust server crates and OpenAPI documents and the management API's TypeScript client, and the renderer's design tokens and fonts from the pinned release of TRMNL's design system (`tools/trmnl-release.json`) |
| `pnpm --filter afterimage-web add <tarball URL>` | Moves the web UI to another [linen release](https://github.com/lutyjj/linen/releases). Then set `hash = lib.fakeHash;` in `deploy/nix/web.nix`, run `nix build .#web` and copy the hash it reports |
| `cargo test --workspace` | Runs every Rust test, the conformance test included |
| `pnpm install` | Installs the web UI's dependencies, after `tools/generate` |
| `pnpm --filter afterimage-web dev` | Serves the web UI with hot reload at `http://127.0.0.1:5173`, sending `/api`, `/manage` and `/mcp` to a server on port 8080, or to the one `AFTERIMAGE_SERVER` names (an OIDC sign-in returns to that server's own address, so sign in there with a password) |
| `pnpm --filter afterimage-web check` | Checks the web UI's formatting, type-checks it with the generated client and runs its tests; `format` fixes the formatting |
| `nix run .#parity [CASE...]` | Draws the cases in `tools/parity/cases/` with TRMNL's framework and with a server of its own, writes them side by side to `build/parity/`, and fails where a case differs from TRMNL by more than its limit |
| `tools/trmnl-excerpt` | Refreshes the excerpt of TRMNL's published device contract |
| `tools/trmnl-catalog` | Refreshes the snapshot of TRMNL's display models and palettes |
| `nix flake check` | Builds the server and the web UI from the pinned contract, runs every test, runs clippy and rustfmt, and boots the NixOS module in a VM |
