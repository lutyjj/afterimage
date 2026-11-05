# Design

Afterimage is a self-hosted server for TRMNL e-paper displays. It onboards devices, keeps their firmware current, renders screens from data, and answers each device's check-in with the screen its schedule calls for. It reports device state over MQTT and keeps no history of it.

It does not run TRMNL community recipes, host plugins for other people, or implement TRMNL's cloud management API.

## Principles

- **Faithful.** The device API follows TRMNL's published contract exactly, so every device running TRMNL firmware works unchanged.
- **Generated.** Device models, palettes, firmware lists and design tokens come from TRMNL's published sources; nobody copies their values into code.
- **Generic.** Components, interfaces and screen elements serve every device and data source. Facts about one device live in data, never in a code path.

## Contract

The Smithy model in `model/` is the single source of truth. One build generates the Rust server stubs (smithy-rs), the TypeScript client (smithy-typescript) and an OpenAPI document from it. No component defines an operation, shape or error outside the model.

| Service | Contract owner | Caller |
|---|---|---|
| Device API | TRMNL firmware | Devices |
| Management API | Afterimage | Web UI |
| Render operation | Afterimage | Server |

The device API mirrors the four operations TRMNL firmware calls: `GET /api/setup`, `GET /api/display`, `GET /api/display/current` and `POST /api/log`. TRMNL publishes them in its OpenAPI document (`https://trmnl.com/api-docs/openapi.json`). A test compares the generated document with TRMNL's and fails on any difference in the paths, headers and response fields the model covers.

## Components

Each component builds on its own and depends on the others only through generated code.

| Path | Owns |
|---|---|
| `model/` | Smithy contract |
| `crates/server/` | Device and management APIs, schedule, SQLite persistence, MQTT publishing |
| `crates/render-typst/` | The render operation in Typst, with a component library generated from TRMNL's design tokens |
| `web/` | Web UI on the generated TypeScript client |
| `deploy/nix/` | Nix package, container image and NixOS module |

## Devices

1. **Setup.** The device joins Wi-Fi through its captive portal, configured with this server's URL, and calls `/api/setup` with its MAC (`ID`) and `Model`. The server creates an unclaimed device with an access key and a friendly ID, and returns a setup screen showing that ID. A user claims the device by entering the ID in the web UI.
2. **Model.** The reported `Model` selects the device's render profile from TRMNL's `GET /api/models` and its firmware board from TRMNL's firmware lists. TRMNL publishes no mapping from the name a firmware build reports to either list, so the server keeps that mapping as data. A user can change a device's render profile.
3. **Check-in.** The device calls `/api/display` with its `Access-Token` and its state (`Battery-Voltage`, `Percent-Charged`, `RSSI`, `FW-Version`). The server answers with the image URL of the screen due now and a `refresh_rate` that wakes the device at its refresh interval or the next scheduled change, whichever comes first. It publishes the reported state to MQTT.
4. **Firmware update.** TRMNL publishes over-the-air images for its own hardware at `GET /api/firmware`, and full flash images for every supported board at `GET /api/firmware/flash`. For a board with only a flash image, the server cuts the app image out at the offset the image's partition table gives, and checks its checksum and SHA-256 before serving it. The server downloads each image once and serves it from its own URL, so devices without internet access can update. An install request sets `update_firmware` and `firmware_url` in the device's next check-in answer.
5. **Logs.** The server writes `/api/log` entries to its own structured log and stores none.

## MQTT

The server publishes each device's battery, signal, firmware version, last check-in and current screen with Home Assistant MQTT discovery, plus a firmware update entity whose install command starts step 4. History belongs to whoever consumes the topics.

## Screens and schedule

A screen is a tree of generic components (title bar, value, label, table, chart and others) whose properties bind to named data sources. The components and their properties are shapes in the model, so the web UI edits screens without a template language. A data source is an HTTP endpoint returning JSON, such as a Home Assistant REST call; the screen refers to its credentials by name and never stores them. A schedule lists, per device, the screens it shows and when.

## Rendering

The render contract is one operation in the model: a screen, its resolved data and a render profile in, an image out. The render profile is what TRMNL's `GET /api/models` publishes for the device's model: width, height, bit depth, palette, MIME type and image size limit. The server caches each image on disk by screen and profile, and renders again when the data or the schedule changes.

Typst implements the operation in-process. Its component library is generated from the public tokens of TRMNL's design system (MIT): fonts, palette, gaps, type scale and dither patterns, so screens keep TRMNL's look. On one measured screen, Typst rendered in 13 ms with a 19 MiB memory peak; TRMNL's design system in headless Chromium took 1.3 s and about 1 GiB, most of it loading its 19 MB stylesheet. On the device, the two renders were hard to tell apart.

Another engine replaces Typst by implementing the same operation and every component, for example as a separate service that runs TRMNL's design system; screens stay unchanged. A screen may instead carry a raw Typst template for what the components cannot express. Such a screen works only with the Typst renderer.

## Storage and accounts

SQLite holds accounts, devices (MAC, access key, model, settings, owner), screens, data sources and schedules. Rendered images and firmware files live in a cache directory that the server rebuilds when it is lost.

Users sign in through OIDC or with a local password. Both implement one sign-in interface, so handlers and storage see an account, never the method. An account owns devices and screens; there are no roles.

## Open questions

1. **Token source.** Whether the generator reads the design system's tokens from its Sass source or from a compiled stylesheet, and how it resolves their per-mode values (1-bit, 2-bit, density).
2. **Component set.** Which of TRMNL's documented components the first release defines.
