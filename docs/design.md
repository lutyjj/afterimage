# Design

Afterimage onboards TRMNL devices, updates their firmware, draws screens from data, and answers each check-in with the screen its schedule calls for. It publishes device state over MQTT and keeps no history.

Out of scope: TRMNL's community recipes, hosting plugins for others, TRMNL's cloud management API.

## Principles

- **Faithful.** The device API follows TRMNL's published contract, so stock firmware works unchanged.
- **Generated.** Models, palettes, firmware lists and design tokens come from TRMNL's published sources, never copied into code.
- **Generic.** Components serve every device and data source. Device facts live in data, never in code paths.

## Contract

The Smithy model in `model/smithy/` defines both APIs and generates the Rust server stubs, the TypeScript client and the OpenAPI documents.

| Service | Owner | Callers |
|---|---|---|
| Device API | TRMNL firmware | Devices |
| Management API | Afterimage | Web UI, scripts, MCP |

- The device API mirrors TRMNL's `GET /api/setup`, `GET /api/display`, `GET /api/display/current` and `POST /api/log`. `model/trmnl/device-api.json` is the excerpt of [TRMNL's OpenAPI document](https://trmnl.com/api-docs/openapi.json); the conformance test fails on any operation, header or field the two do not share.
- Afterimage adds `GET /api/images/{name}` and `GET /api/firmware-images/{name}`.
- Where the document and the firmware disagree, the firmware wins: headers are text, and an unparsable value is unknown.

## Components

Components depend on each other only through generated code, except that Rust crates may use `crates/trmnl`, and the server links `crates/render-typst` and `crates/mcp` in-process.

| Path | Owns |
|---|---|
| `model/smithy/` | Contract |
| `model/conformance/` | Conformance test, `tools/trmnl-excerpt` extractor |
| `crates/trmnl/` | Firmware protocol, snapshot of TRMNL's models and palettes |
| `crates/server/` | APIs, accounts, schedules, SQLite, MQTT |
| `crates/render-typst/` | Typst renderer |
| `crates/mcp/` | MCP endpoint |
| `tools/tokens/` | Design token generator |
| `web/` | Web UI on [linen](https://github.com/lutyjj/linen) |
| `deploy/nix/` | Packages, release image, dev shell, NixOS module |

## Devices

### Setup and claim

1. The device joins Wi-Fi through its portal, given this server's URL.
2. It calls `/api/setup` with its MAC and `Model`.
3. The server creates a registration: an access key (stored as SHA-256) and a six-character code the device shows.
4. A user enters the code in the web UI.

- Setup always creates a new registration and never touches an existing one: the firmware's only recovery from a lost key is status 500, which wipes it.
- Claiming a new code for a device of the same MAC in the same account moves that device, with its settings, to the new registration.
- Removing a device without a reset returns it to its code.

| Unclaimed registrations | Limit |
|---|---|
| At once | 32 |
| Never checked in | Expires after 24 hours |
| Stopped checking in | Expires after 7 days |

### Display model

The server picks the model the firmware reports, else TRMNL's own hardware with the most grays at the reported panel size. Users can change it.

### Check-in

`/api/display` carries the key, MAC, battery, signal and firmware version. The server keeps the latest readings, publishes them to [MQTT](#mqtt), and answers:

| Status | When | Firmware |
|---|---|---|
| 0 | Claimed | Shows the image, wakes after `refresh_rate` |
| 202 | Unclaimed or unknown key | Shows its code, polls |
| 500 | Removed with reset | Erases Wi-Fi and settings |

- The image is the next screen of the scheduled playlist, or the sleep screen while [asleep](#sleep). Unchanged images are not downloaded again.
- `refresh_rate` wakes the device at the earliest of its interval, the next playlist change, and the start of sleep.
- An unknown key gets 202, never 500.
- A screen that fails to draw leaves the last image in place.
- Without `Percent-Charged` (all boards but TRMNL X), charge comes from the voltage by TRMNL's formula.

### Firmware update

The server installs TRMNL's releases on every board TRMNL builds for, serving images itself.

1. Fetch the release's flash image (`GET /api/firmware/flash`).
2. Cut out and verify the app image.
3. Serve it at `/api/firmware-images/<SHA-256>.bin`.
4. Send `update_firmware` at the next check-in, once.
5. Settle the request by the version the device reports next.

- The board is the one a user chose, else the one named like the reported model.
- A board's newest release is its highest version without a variant suffix.
- The request is sent once because firmware 1.6 restarts on every failed attempt.

### Logs

`/api/log` entries go to the server's log, escaped and capped at 64 KiB, 32 entries and 512 characters per field.

## MQTT

With a broker set, each claimed device appears in Home Assistant through MQTT discovery.

| Entity | Kind |
|---|---|
| Battery, voltage, signal, last check-in, current screen, wakes at | Sensor |
| Asleep, not seen lately | Binary sensor |
| Firmware | Update |
| Sleep (hold asleep) | Switch |
| Showing (schedule, or a pinned playlist or screen) | Select |
| Wakes every | Number |
| Button | Event |
| Image | Image |

- Devices are keyed by MAC. Discovery, state and image topics are retained; commands are not, and retained commands are ignored.
- The broker's access control is the security boundary.
- The broker comes from `AFTERIMAGE_MQTT_*`, else from the web UI. Moving to another broker or topic clears the old topics.

## MCP

`/mcp` exposes the management operations tagged `mcp` as tools, described from the embedded OpenAPI document. Calls run in-process with the caller's bearer token; cookies are ignored. Images in answers reach the agent as images.

## Screens, playlists and schedules

### Screens

A screen is a document of data sources and components. It reads the same as JSON, YAML and in the API.

- A data source is an HTTP request answering JSON. It can read `secrets.NAME` from `AFTERIMAGE_SECRET_NAME`; every account can use every secret.
- Text properties are MiniJinja templates reading each source, `now()` and `device`. A single-expression template keeps its type.
- `choose` picks one of two components by a template's truth.

| Group | Components |
|---|---|
| Layout | Row, column, grid |
| Content | Title bar, item, title, label, value, description, image, divider |
| Charts | Line, bar |
| Design system | Table, progress bar, progress dots, rich text |

### Playlists

Screens in order, shown one per check-in. Each device keeps its own place and resumes it when it returns. Changing a playlist's screens restarts every device on it.

### Schedules

| Part | Rule |
|---|---|
| Usual playlist | Shown when no slot is current |
| Slot | Another playlist on chosen weekdays and times; may cross midnight |
| Overlap | Refused |
| Pin | A playlist or screen shown instead of the schedule until set back |

### Sleep

Sleep windows use the same days and times and override the schedule. Asleep, a device shows its sleep screen, keeps its playlist position, and wakes when the window ends (at most a day later). A device can also be held asleep. The server cannot wake a device.

## Rendering

1. Send the data source requests.
2. Render templates, expand `each` and `table`, decide `choose`.
3. Fetch images.
4. Render with the device's model and palette.

Secret values are replaced by their names in answers and problems.

| Limit | Value |
|---|---|
| Fetching sources and images | 11 s |
| Running templates | 2 s |
| Starting templates or the render | Within 13 s of the drawing's start (firmware waits 15 s) |
| Components | 2,000 |
| Images | 16 |
| Items in `each` or `table` without `limit` | 100 |

### Typst renderer

`crates/render-typst` lays out components with TRMNL's design tokens, rasterizes at the model's scale, rotates, maps to the palette, and writes the smallest PNG. It renders in 13 ms and 19 MiB, against 1.3 s and about 1 GiB for TRMNL's design system in Chromium.

### Token generator

`tools/tokens` loads the pinned release of TRMNL's stylesheet (`tools/trmnl-release.json`) in headless Chromium and records each token's computed value per display profile, plus what each component paints. Chromium runs only at build time.

### Parity harness

`nix run .#parity` renders each case in `tools/parity/cases/` with TRMNL's framework and with Afterimage, and fails cases whose pixel difference exceeds their limit.

## Storage and accounts

SQLite holds records; the data directory holds images, the cache directory firmware.

- Sign-in is a password or OIDC (authorization code with PKCE). The first account administers the server.
- Sessions are httpOnly, `SameSite=Lax` cookies. Passwords are argon2id hashes; sessions, tokens and device keys are SHA-256 hashes.
- Personal access tokens act as their owner, cannot manage tokens, and win over cookies. Read-only tokens may call only `@readonly` operations.
- Another account's resource answers as missing.
