# Design

Afterimage is a self-hosted server for TRMNL e-paper displays. It onboards devices, keeps their firmware current, renders screens from data, and answers each device's check-in with the screen its schedule calls for. It reports device state over MQTT and keeps no history of it.

It does not run TRMNL community recipes, host plugins for other people, or implement TRMNL's cloud management API.

## Contract

The Smithy model in `model/` is the single source of truth. One build generates the Rust server stubs (smithy-rs), the TypeScript client (smithy-typescript) and an OpenAPI document from it. No component defines an operation, shape or error outside the model.

| Service | Contract owner | Caller |
|---|---|---|
| Device API | TRMNL firmware | Devices |
| Management API | Afterimage | Web UI |
| Renderer API | Afterimage | Server |

The device API mirrors the four operations TRMNL firmware calls: `GET /api/setup`, `GET /api/display`, `GET /api/display/current` and `POST /api/log`. TRMNL publishes them in its OpenAPI document (`https://trmnl.com/api-docs/openapi.json`). A test compares the generated document with TRMNL's and fails on any difference in the paths, headers and response fields the model covers.

## Components

Each component builds on its own and depends on the others only through generated code.

| Path | Owns |
|---|---|
| `model/` | Smithy contract |
| `crates/server/` | Device and management APIs, schedule, SQLite persistence, MQTT publishing |
| `renderer/` | Renderer API: screen template and data to device image |
| `web/` | Web UI on the generated TypeScript client |
| `deploy/nix/` | Nix package, container image and NixOS module |

## Devices

1. **Setup.** The device joins Wi-Fi through its captive portal, configured with this server's URL, and calls `/api/setup` with its MAC (`ID`) and `Model`. The server creates an unclaimed device with an access key and a friendly ID, and returns a setup screen showing that ID. A user claims the device by entering the ID in the web UI.
2. **Check-in.** The device calls `/api/display` with its `Access-Token` and its state (`Battery-Voltage`, `Percent-Charged`, `RSSI`, `FW-Version`). The server answers with the image URL of the screen due now and a `refresh_rate` that wakes the device at its refresh interval or the next scheduled change, whichever comes first. It publishes the reported state to MQTT.
3. **Firmware update.** The server reads the latest stable firmware for each model from TRMNL's `GET /api/firmware`, downloads it once and serves it from its own URL, so devices without internet access can update. An install request sets `update_firmware` and `firmware_url` in the device's next check-in answer.
4. **Logs.** The server writes `/api/log` entries to its own structured log and stores none.

## MQTT

The server publishes each device's battery, signal, firmware version, last check-in and current screen with Home Assistant MQTT discovery, plus a firmware update entity whose install command starts step 3. History belongs to whoever consumes the topics.

## Screens and schedule

A screen is a template plus named data sources. A data source is an HTTP endpoint returning JSON, such as a Home Assistant REST call; the screen refers to its credentials by name and never stores them. A schedule lists, per device, the screens it shows and when.

The renderer turns a template and its data into an image for one device model: the width, height, bit depth and palette that TRMNL's `GET /api/models` publishes, in the model's MIME type and under its image size limit. The server caches each image on disk by screen and model, and renders again when the data or the schedule changes.

## Storage and accounts

SQLite holds accounts, devices (MAC, access key, model, settings, owner), screens, data sources and schedules. Rendered images and firmware files live in a cache directory that the server rebuilds when it is lost.

Users sign in through OIDC or with a local password. Both implement one sign-in interface, so handlers and storage see an account, never the method. An account owns devices and screens; there are no roles.

## Open questions

1. **Renderer.** Playwright with TRMNL's design system (MIT; its CSS and JavaScript runtime, pinned to a release from `GET /api/framework`), or Typst rendered to an image in Rust. A spike renders the same screen both ways and compares appearance on a device, render time, memory and code size. The answer also picks the template language and the renderer's implementation language.
