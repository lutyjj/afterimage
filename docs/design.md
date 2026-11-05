# Design

Afterimage is a self-hosted server for TRMNL e-paper displays. It onboards devices, keeps their firmware current, renders screens from data, and answers each device's check-in with the screen its schedule calls for. It reports device state over MQTT and keeps no history of it.

It does not run TRMNL community recipes, host plugins for other people, or implement TRMNL's cloud management API.

## Principles

- **Faithful.** The device API follows TRMNL's published contract exactly, so every device running TRMNL firmware works unchanged.
- **Generated.** Device models, palettes, firmware lists and design tokens come from TRMNL's published sources; nobody copies their values into code.
- **Generic.** Components, interfaces and screen elements serve every device and data source. Facts about one device live in data, never in a code path.

## Contract

The Smithy model in `model/smithy/` is the single source of truth. One build generates the Rust server stubs (smithy-rs), the TypeScript client (smithy-typescript) and an OpenAPI document from it. No component defines an operation, shape or error outside the model.

| Service | Contract owner | Caller |
|---|---|---|
| Device API | TRMNL firmware | Devices |
| Management API | Afterimage | Web UI |
| Render operation | Afterimage | Server |

The device API mirrors the four device operations TRMNL documents: `GET /api/setup`, `GET /api/display`, `GET /api/display/current` and `POST /api/log`; firmware 1.8.18 calls all but `/api/display/current`. TRMNL publishes them in its OpenAPI document (`https://trmnl.com/api-docs/openapi.json`); `model/trmnl/device-api.json` is the excerpt that `tools/trmnl-excerpt` takes from it, and the conformance test fails on any operation, header, body field or response field that the generated document and the excerpt do not share.

Where the document and the firmware disagree, the firmware decides. Headers are text, because the firmware sends values such as `3000/3500` where the document says number; the server parses what it reads and treats a value that does not parse as unknown. Status values follow the firmware's handling of them, which behaviour tests replay with the headers and bodies the firmware builds. The conformance test compares operations, headers, body fields and success fields; error responses are outside it, and each one the model adds is documented on its operation.

## Components

Each component builds on its own and depends on the others only through generated code, except that any Rust component may use the shared library `crates/trmnl`.

| Path | Owns |
|---|---|
| `model/smithy/` | Smithy contract |
| `model/conformance/` | The conformance test and the extractor behind `tools/trmnl-excerpt` |
| `crates/trmnl/` | TRMNL firmware protocol (MAC addresses, reported readings, status values) and the snapshot of TRMNL's model and palette lists that `tools/trmnl-catalog` refreshes |
| `crates/server/` | Device and management APIs, accounts and sessions, schedule, SQLite persistence, MQTT publishing |
| `crates/render-typst/` | The render operation in Typst, with a component library that reads design tokens |
| `tools/tokens/` | Build-time generator of design tokens per render profile |
| `web/` | Web UI on the generated TypeScript client |
| `deploy/nix/` | Nix package, container image and NixOS module |

## Devices

1. **Setup.** The device joins Wi-Fi through its captive portal, configured with this server's URL, and calls `/api/setup` with its MAC (`ID`) and `Model`. Every setup creates a new registration: an access key, stored only as a SHA-256 hash, and a six-character friendly ID, which the firmware shows. A user claims the device by entering the ID in the web UI. Setup never touches an existing registration, because the firmware's only way back from a lost key is status 500, which erases its saved Wi-Fi networks and every stored setting, the server URL included. Anyone on the network can present a MAC, so a spoofed setup only creates a registration of its own, whose code never appears on the real device's screen. A device that runs setup again shows its new code; claiming that code for a device of the account with the same MAC moves that device to the new registration with its name, settings and playlist, and drops the old registration and every other unclaimed one of that MAC. Removing a device without a reset returns its registration to unclaimed, so the device shows its code again. At most 32 registrations wait unclaimed. One whose key never checks in ends 24 hours after setup, and one that stops checking in ends after 7 days; a powered device checks in at least hourly. A host on the network can still hold those places with registrations that keep checking in, until a user removes them in the web UI.
2. **Model.** A claimed device starts with a display chosen from TRMNL's `GET /api/models` and `GET /api/palettes`: the model whose name the firmware reports, or else, by the panel size the firmware sends at check-in, TRMNL's own hardware with the most grays, at the palette its bit depth gives. TRMNL publishes no mapping from the name a firmware build reports to its lists, so the choice rests on those published fields alone, and a user can change it. The firmware board for updates comes from TRMNL's firmware lists the same way.
3. **Check-in.** The device calls `/api/display` with its `Access-Token`, its MAC and its state (`Battery-Voltage`, `Percent-Charged`, `RSSI`, `FW-Version`); the server keeps the last known value of each reading. A claimed device gets the image URL of the screen due now and a `refresh_rate` that wakes it at its refresh interval or the next scheduled change, whichever comes first. An unclaimed device gets status 202, on which the firmware shows its friendly ID and polls on its own schedule, every 5 seconds at first and backing off to hourly. A key the server does not know also gets 202, because 500 would wipe the device; such a device keeps showing a code the server has dropped and needs a reset at the device, and the server logs its MAC. Status 500 is sent only when a user removes a device in the web UI and chooses to reset it. A server failure is an HTTP error, which the firmware retries. The server publishes the reported state to MQTT.
4. **Firmware update.** TRMNL publishes over-the-air images for its own hardware at `GET /api/firmware`, and full flash images for every supported board at `GET /api/firmware/flash`. For a board with only a flash image, the server cuts the app image out at the offset the image's partition table gives, and checks its checksum and SHA-256 before serving it. The server downloads each image once and serves it from its own URL, so devices without internet access can update. An install request sets `update_firmware` and `firmware_url` in the device's next check-in answer.
5. **Logs.** The server writes `/api/log` entries to its own structured log and stores none. It reads at most 64 KiB per request, writes at most 32 entries and 512 characters per field, and writes device text escaped so that it cannot add lines to the log. Requests are bounded but their rate is not: a host on the network that holds a key can fill the log, which is the same trust the network already has over the device API.

## MQTT

The server publishes each device's battery, signal, firmware version, last check-in and current screen with Home Assistant MQTT discovery, plus a firmware update entity whose install command starts step 4. History belongs to whoever consumes the topics.

## Screens and schedule

A screen is a tree of generic components whose properties bind to named data sources. The components and their properties are shapes in the model, so the web UI edits screens without a template language. A data source is an HTTP endpoint returning JSON, such as a Home Assistant REST call; the screen refers to its credentials by name and never stores them. A schedule lists, per device, the screens it shows and when.

The first release defines the components that TRMNL's 30 published plugins use most: view and layout (rows, columns, grid spans, alignment, gaps), title bar, item, title, label, value, description, image and divider, plus line and bar charts and three behaviours: fitting a value to its box, clamping text to a line count, and ending an overflowing list with a count of what is hidden. Table, progress, rich text and maps follow when a screen needs them; no published plugin uses table or progress.

## Rendering

The render contract is one operation in the model: a screen, its resolved data and a render profile in, an image out. The render profile is what TRMNL's `GET /api/models` publishes for the device's model: width, height, bit depth, palette, MIME type and image size limit. The server caches each image on disk by screen and profile, and renders again when the data or the schedule changes.

Typst implements the operation in-process, with a component library that reads the design tokens of TRMNL's design system (MIT), so screens keep TRMNL's look. A build-time generator produces those tokens. It loads a pinned release of the compiled stylesheet in headless Chromium once per render profile, applies each public token (the 278 that the release documents, plus its 21 fill patterns) to a probe element, and records the computed value: lengths in pixels, colors, font families, weights, and the dither patterns as SVG. Chromium runs only in the generator, never during a render. Reading the Sass source or the compiled stylesheet directly would mean reimplementing the cascade: the stylesheet declares 5,254 custom properties across 62,375 selectors, with `:is()`, `:where()`, cascade layers and color, dark and theme modes. The generator's inputs are pinned: a framework release and a snapshot of TRMNL's model and palette lists, which one command refreshes.

On one measured screen, Typst rendered in 13 ms with a 19 MiB memory peak; TRMNL's design system in headless Chromium took 1.3 s and about 1 GiB, most of it loading its 19 MB stylesheet. On the device, the two renders were hard to tell apart.

Another engine replaces Typst by implementing the same operation and every component, for example as a separate service that runs TRMNL's design system; screens stay unchanged. A screen may instead carry a raw Typst template for what the components cannot express. Such a screen works only with the Typst renderer.

## Storage and accounts

SQLite holds accounts, registrations (MAC, model, access key hash, friendly ID, end time, latest readings), devices (owner, name, settings, current registration), screens, data sources and schedules. Rendered images and firmware files live in a cache directory that the server rebuilds when it is lost.

Users sign in through OIDC or with a local password; either way the result is an account, and handlers and storage never see the method. The first account signs up with a password; further accounts come from the OIDC provider, whose client assignment decides who may sign in. A session is a random token in an httpOnly, `SameSite=Lax` cookie, `Secure` with the `__Host-` prefix over TLS; the database holds its SHA-256 and passwords as argon2id hashes. Every management operation but the five that start a session needs one, and a resource of another account answers as a missing one does. An account owns devices and screens; there are no roles.
