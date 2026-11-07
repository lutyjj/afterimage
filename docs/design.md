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
| Management API | Afterimage | Web UI, scripts, and agents through MCP |

The device API mirrors the four device operations TRMNL documents: `GET /api/setup`, `GET /api/display`, `GET /api/display/current` and `POST /api/log`; firmware 1.8.18 calls all but `/api/display/current`. TRMNL publishes them in its OpenAPI document (`https://trmnl.com/api-docs/openapi.json`); `model/trmnl/device-api.json` is the excerpt that `tools/trmnl-excerpt` takes from it, and the conformance test fails on any operation, header, body field or response field that the generated document and the excerpt do not share. The model adds one operation of Afterimage's own, `GET /api/images/{name}` for the images check-ins name, which the conformance test lists as an addition.

Where the document and the firmware disagree, the firmware decides. Headers are text, because the firmware sends values such as `3000/3500` where the document says number; the server parses what it reads and treats a value that does not parse as unknown. Status values follow the firmware's handling of them, which behaviour tests replay with the headers and bodies the firmware builds. The conformance test compares operations, headers, body fields and success fields; error responses are outside it, and each one the model adds is documented on its operation.

## Components

Each component builds on its own and depends on the others only through generated code, except that any Rust component may use the shared library `crates/trmnl`, and the server links the renderer library `crates/render-typst`, which draws in its process, and the MCP library `crates/mcp`, which reaches the management API in its process.

| Path | Owns |
|---|---|
| `model/smithy/` | Smithy contract |
| `model/conformance/` | The conformance test and the extractor behind `tools/trmnl-excerpt` |
| `crates/trmnl/` | TRMNL firmware protocol (MAC addresses, reported readings, status values) and the snapshot of TRMNL's model and palette lists that `tools/trmnl-catalog` refreshes |
| `crates/server/` | Device and management APIs, accounts and sessions, schedule, SQLite persistence, MQTT publishing |
| `crates/render-typst/` | The render operation in Typst, with a component library that reads design tokens |
| `crates/mcp/` | The MCP endpoint: the management API's tagged operations as tools |
| `tools/tokens/` | Build-time generator of design tokens per render profile |
| `web/` | Web UI on the generated TypeScript client, wearing linen, the design language pinned in `flake.lock` |
| `deploy/nix/` | Nix packages, the development shell and the NixOS module |

## Devices

Devices running TRMNL firmware go through setup, claim, check-ins, firmware updates and log uploads on TRMNL's device API. The server sends status 500, which makes the firmware erase its saved Wi-Fi networks and every stored setting, the server URL included, only when a user removes a device in the web UI and chooses to reset it.

### Setup and claim

Every setup creates a new registration, and a user claims it in the web UI. Setup never touches an existing registration, because the firmware's only way back from a lost key is status 500.

1. The device joins Wi-Fi through its captive portal, configured with this server's URL.
2. It calls `/api/setup` with its MAC (`ID`) and `Model`. Firmware before 1.8 sends `Model` empty; every check-in sends `Model` too, and the server keeps the latest.
3. The server creates a registration: an access key, stored only as a SHA-256 hash, and a six-character friendly ID, which the firmware shows.
4. A user claims the device by entering the friendly ID in the web UI.

- Anyone on the network can present a MAC, so a spoofed setup only creates a registration of its own, whose code never appears on the real device's screen.
- A device that runs setup again shows its new code. Claiming that code for a device of the account with the same MAC moves that device to the new registration with its name, settings and playlist, and drops the old registration. Other unclaimed registrations of that MAC end on their own.
- Removing a device without a reset returns its registration to unclaimed, so the device shows its code again.

| Unclaimed registrations | Limit |
|---|---|
| Waiting at once | At most 32 |
| One whose key never checks in | Ends 24 hours after setup |
| One that stops checking in | Ends after 7 days |

A powered device checks in at least hourly. A host on the network can still hold the 32 places with registrations that keep checking in, until a user removes them in the web UI.

### Display model

A claimed device starts with a display chosen from TRMNL's `GET /api/models` and `GET /api/palettes`, and a user can change it. The server picks, in order:

1. The model whose name the firmware reports.
2. Else, by the panel size the firmware sends at check-in, TRMNL's own hardware with the most grays, at the palette its bit depth gives.

TRMNL publishes no mapping from the name a firmware build reports to its lists, so the choice rests on those published fields alone. The firmware board for updates comes from TRMNL's firmware lists the same way (see [Firmware update](#firmware-update)).

### Check-in

The device calls `/api/display` with its `Access-Token`, its MAC and its state (`Battery-Voltage`, `Percent-Charged`, `RSSI`, `FW-Version`). The server keeps the last known value of each reading, publishes the reported state to [MQTT](#mqtt), and answers with a status:

| Status | Sent when | The firmware |
|---|---|---|
| 0 | The device is claimed | Shows the image the answer names and wakes again after `refresh_rate` |
| 202 | The device is unclaimed, or its key is one the server does not know | Shows its friendly ID and polls on its own schedule: every 5 seconds at first, backing off to hourly |
| 500 | A user has removed the device in the web UI and chosen to reset it | Erases its saved Wi-Fi networks and every stored setting, the server URL included |
| Another HTTP error | The server fails | Retries |

An unknown key gets 202 because 500 would wipe the device. Such a device keeps showing a code the server has dropped and needs a reset at the device; the server logs its MAC.

Only TRMNL X sends `Percent-Charged`. For every other board the server estimates the charge from the voltage with TRMNL's own formula, which firmware 1.8.18 documents as the server's `percent_charged_calculation`.

Status 0 carries the device's next screen:

- The image is the next current entry of the device's playlist, drawn at the check-in. With nothing current, the device shows a screen saying so.
- `refresh_rate` wakes the device at its refresh interval or when the set of current entries changes, whichever comes first.
- The image URL (`/api/images/<name>`) uses the host the device checked in at. The firmware downloads it with its `Access-Token`, and only that device may.
- An image is named by a hash of its content, and the firmware caches by `filename`, so an unchanged screen costs no download.
- A screen that cannot be drawn as written leaves the device on its last image, or shows what went wrong when it has none. The next check-in moves on to the entry after it.

### Firmware update

The server updates every board TRMNL builds firmware for and serves the image itself, so devices without internet access update too.

- TRMNL publishes full flash images for every board it builds firmware for at `GET /api/firmware/flash`, newest release first. Its over-the-air images (`GET /api/firmware`) cover only its own hardware, and their bytes are the app images inside the flash images of the same boards, so the flash list serves every board.
- A device's board is the one a user chose, or else the board whose name is the model the firmware reports. TRMNL publishes no other link between the two: the names differ for most boards (the Seeed kit's `xiao_epaper_display` is built as `seeed_OG_DIY_Kit`), and a board's model name changes between releases.
- The newest release of a board is its highest version without a variant suffix such as `-sensors`.

An install request goes through these steps:

1. The server fetches the release's flash image.
2. The server cuts the app image out at the app partition the flash image's partition table names, ends it where the app image's own header says, and checks its checksum and SHA-256 as the bootloader does.
3. The server keeps the app image in its cache directory under its SHA-256 and serves it at `/api/firmware-images/<SHA-256>.bin`. The firmware downloads it without a key or redirects (1.6.10 and 1.8.18), and the image is public anyway.
4. The device's next check-in answer carries the request once, with `update_firmware` and `firmware_url` beside its screen.
5. The check-in after that decides the request by the version the device reports.

The request goes out only once because a request sent at every check-in would restart a failing 1.6 device over and over:

| Firmware | After installing the release | After a failure |
|---|---|---|
| 1.8 | Restarts and checks in at once | Checks in at its next wake and waits a day before it tries again |
| 1.6 | Restarts and checks in | Restarts and checks in, and never waits |

- A check-in on a special function gets no request, since firmware 1.6 then ignores it.
- A variant release, such as `1.8.18-push-to-refresh`, counts as installed when the device reports the version it varies, which is all its build reports.
- Choosing another board withdraws a request the device has not received, including one whose image is still being fetched.

### Logs

The server writes `/api/log` entries to its own structured log and stores none.

| Limit | Value |
|---|---|
| Bytes read per request | 64 KiB |
| Entries written per request | 32 |
| Characters written per field | 512 |

The server writes device text escaped so that it cannot add lines to the log. Requests are bounded but their rate is not: a host on the network that holds a key can fill the log, which is the same trust the network already has over the device API.

## MQTT

With a broker configured, the server publishes each claimed device to Home Assistant through its device-based MQTT discovery. History belongs to whoever consumes the topics.

### Entities

| Entity | Home Assistant kind | Note |
|---|---|---|
| Battery | Sensor | Estimated from the voltage, as under [Check-in](#check-in), where the firmware reports none |
| Voltage, signal, last check-in, current screen | Sensor | |
| Firmware | Update | Its install command starts a [firmware update](#firmware-update) to the board's newest release |

- A device is keyed by its MAC, so it stays the same Home Assistant device across setups. Its link back leads to its page in the web UI.
- When accounts hold devices of one MAC, as after a device set up again is claimed elsewhere, the topics belong to the one that checked in last.
- Whoever may publish to the broker may start an install, so the broker's access control is the boundary. A retained install command is ignored, since the broker would replay it at every connection.

### Topics

| Event | The server |
|---|---|
| It connects to the broker | Publishes every device, and clears the retained topics of devices removed while the broker was out of reach |
| Home Assistant announces itself on its status topic | Publishes every device |
| A device checks in or changes | Publishes that device |
| A device is removed | Clears its topics |

- Discovery, state and firmware topics are retained.
- A fact the server does not know is left out, since Home Assistant drops a payload with a null where it reads text.
- Every entity's availability follows the server's own status topic, which its last will sets offline.

### Broker settings

The broker is the deployment's (`AFTERIMAGE_MQTT_URL` and the settings beside it), or else the one the administrator saves through the management API (the web UI's Home Assistant settings), which takes effect at once. Where the deployment sets one, the API only shows it, so a declarative deployment stays the one place it changes.

| Saved change | Effect |
|---|---|
| Another broker or other topics | The server clears every topic it published and disconnects cleanly, so the broker does not publish the last will, then connects with the new settings. Where the old broker cannot be reached at that moment, what was published there stays. |
| Other credentials | What was published stays, but the server marks the displays unavailable until it is connected again, and for as long as the broker refuses the credentials. |
| The same settings | Nothing |

- A saved password goes only to the broker and username it was given for: naming another asks for it again.
- The saved password stays in the database as written, since the server sends it to the broker, and no answer carries it.

## MCP

The server serves the Model Context Protocol at `/mcp`, so an agent writes screens, previews them and arranges playlists through the same management API as the web UI.

### Tools

The tools are the management operations the model tags `mcp`, read from the OpenAPI document the build embeds.

| Tool part | Source |
|---|---|
| Name | The operation's name |
| Description | The operation's documentation |
| Input | The operation's input structure, with every shape it refers to |
| Endpoint instructions | The service's documentation |

- A model validator requires each tagged operation to be documented and to take a token.
- Operations that start or end sessions, manage tokens, change server settings or remove a device are not tagged.

### Calls

- Before the protocol sees a request, the endpoint asks `GetAccount` with its credential and returns the API's 401 and challenge when the API refuses it.
- A call is a request to the management API in the server's own process, carrying the caller's `Authorization` and never a cookie, so a personal access token is the only credential. The API decides what the token may do, a read-only one included, and answers each call as it answers a script.
- The endpoint is stateless and answers JSON, since every tool is one request and one answer.
- An answer is text, except that an image the API answers, alone or as a base64 member of its JSON such as a preview's, goes to the client as an image the agent can look at.

## Screens and playlists

A screen document holds its data sources and a tree of generic components. A device's playlist decides which screen it shows at each check-in.

### Screen documents

The components and their properties are shapes in the model, so the document reads the same as JSON, as YAML in the web UI's editor, and in the API. The server stores it in the model's serde form.

### Data sources and secrets

- A data source is an HTTP request answering JSON, such as a Home Assistant REST call.
- Data source requests read `secrets.NAME`, values the deployment provides as `AFTERIMAGE_SECRET_NAME` environment variables. Screens name secrets and never hold them.
- Every account may use every secret in its requests, so a deployment gives its secrets to everyone it lets sign in.

### Templates and styles

- Text properties are MiniJinja templates (Jinja syntax, as in Home Assistant, with its `multiply` filter) that read each source by name, `now()` and `device`.
- A template that is a single expression keeps its value's type, so a chart's series can be a list.
- Properties that set a style (sizes, emphasis, label styles) are typed values the editor completes.
- A template chooses between styles with `choose`, which draws one of two components as a template's value tests true or not, as Jinja's `if` does.

### Screen components

The components are those TRMNL's 30 published plugins use most. The design system's table, progress bar and dots, and rich text stand beside them.

| Group | Members |
|---|---|
| View and layout | Rows, columns, grid spans, alignment, gaps |
| Content | Title bar, item, title, label, value, description, image, divider |
| Charts | Line and bar charts |
| Behaviours | Fitting a value to its box; clamping text to a line count; ending an overflowing list with a count of what is hidden |
| Design system | Table (a row for each element of a list), progress bar and dots, rich text (Markdown) |

- Rows, columns and items take TRMNL's inverse, padding and rounded corners, so a `choose` between an inverse cell and a plain one marks today in a calendar's week.
- Maps follow when a screen needs them.

### Playlists

A device's playlist lists the screens it shows, each on some weekdays and between two times of day in the server's time zone. At each check-in the device shows the next current entry.

## Rendering

The server draws a screen at each check-in and for each preview, bounded by the time the firmware waits for the check-in's answer.

### Drawing steps

1. The server sends each data source's request, with its templates rendered and the deployment's secrets available to them.
2. It renders every other template against the answers, expanding each `each` and `table` and deciding each `choose`.
3. It fetches the images the screen names.
4. The renderer gets the resolved screen, the images' bytes, and the device's model and palette from TRMNL's `GET /api/models` and `GET /api/palettes`: width, height, scale factor, rotation, MIME type, image size limit and colors. That interface is the render boundary.

Secret values are replaced by their names in every answer a source gives and in every problem, so neither the screen nor the editor shows one.

### Drawing limits

Firmware 1.8.18 waits 15 seconds for the check-in's answer, so a drawing is bounded:

| Limit | Value |
|---|---|
| Fetching data sources and images, in parallel and each image once | 11 seconds |
| Running templates | 2 seconds |
| Starting templates or the render | Within 13 seconds of the drawing's start; past that the drawing gives up and the device stays on its last image |
| Components a document expands to | 2,000, counting each table cell as a component |
| Images in a document | 16 |
| Elements of an `each` or `table` without a `limit` | 100 |

### Typst renderer

Typst implements the renderer in-process (`crates/render-typst`). Its component library lays a screen out at the model's size divided by its scale factor and draws every component from the design tokens of TRMNL's design system (MIT) for the device's profile, so screens keep TRMNL's look. The tokens give fonts, sizes, gaps, radii, and TRMNL's dither tiles for gray areas on 1-bit and 2-bit displays.

- Text sits in CSS's line boxes: the token's line height, with the font's own ascent and descent (read from the font files) centred in it, so lines stack and sit where a browser puts them.
- Each run of text starts on a whole device pixel, as a browser places it, so TRMNL's pixel fonts stay on the pixel grid and render crisp.
- Dividers are tracks one rule thick, filled with the rule.

The renderer then:

1. Rasterizes the layout at the scale factor.
2. Turns it by the model's rotation.
3. Maps it to the palette's colors. On two-color palettes everything darker than 75% luminance is ink, so the thin strokes of pixel fonts survive; where more of a pixel's surroundings are dark than light, as on an inverse element, everything lighter than 25% is paper, so light strokes survive too.
4. Writes it as a PNG of the fewest bits per pixel that hold the palette.

Another engine replaces Typst by implementing the same interface and every component, for example as a separate service that runs TRMNL's design system; screens stay unchanged.

### Token generator

A build-time generator (`tools/tokens`) produces the design tokens into `gen/design/` with the rest of the generated code, so the tree keeps only the pins of the release it reads (`tools/trmnl-release.json`, the SHA-256 of every file). A build that fetches the pinned files gives the same tokens every time.

The generator:

1. Loads the pinned release of the compiled stylesheet in headless Chromium.
2. For every profile of the catalogue snapshot, applies each public token (the 278 that the release documents) to a probe element and records the computed value: lengths in pixels, colors, font families, weights, and line heights as ratios of their font size.
3. Lays out the release's own markup for each component and records what each part paints, on the screen and inside an inverse element, so the stylesheet's cascade chooses the slot for each profile: a color, a dither tile as SVG, or a row of pixels for the rules that tables and dividers draw.
4. Copies the release's fonts with their licence notices.

Chromium runs only in the generator, never during a render. Reading the Sass source or the compiled stylesheet directly would mean reimplementing the cascade: the stylesheet declares 5,254 custom properties across 62,375 selectors, with `:is()`, `:where()`, cascade layers and color, dark and theme modes.

### Parity harness

A parity harness (`tools/parity`, run on demand as `nix run .#parity`) measures how far the renderer is from TRMNL's own, so a change to the renderer shows whether it closed a gap or opened one. Each case holds one screen twice:

| Side | Written as | Drawn |
|---|---|---|
| TRMNL | Markup of TRMNL's framework | As TRMNL captures a plugin: in Firefox with the release's stylesheet, script and fonts from the same pins as the token generator, then reduced to the palette with ImageMagick as TRMNL's quantizer does |
| Afterimage | An Afterimage screen document | As `PreviewScreen` answers it from a server the harness starts on an empty data directory |

The two sides meet only at the management API, whose published wire format the harness speaks as any script does. Each case names the display profiles it is drawn on, each with the share of pixels that may differ, and the harness fails a case that goes over it.

### Measurements

On one measured screen:

| Renderer | Time | Memory |
|---|---|---|
| Typst | 13 ms | 19 MiB peak |
| TRMNL's design system in headless Chromium | 1.3 s | About 1 GiB |

Chromium spent most of that loading the design system's 19 MB stylesheet. On the device, the two renders were hard to tell apart.

## Storage and accounts

SQLite holds the server's records, and the data and cache directories hold its images. Every user signs in to an account, and the first account administers the server.

### Storage

| SQLite record | Contents |
|---|---|
| Accounts, sessions, personal access tokens | Their secrets only as hashes (see [Sessions and tokens](#sessions-and-tokens)) |
| Registrations | MAC, model, access key hash, friendly ID, end time, panel size, latest readings |
| Devices | Owner, name, display, refresh interval, current registration, playlist position, the image it shows, the firmware board a user chose |
| Firmware install requests | Each device's latest |
| Screens and playlists | Screen documents and playlist entries |
| MQTT broker | The one the administrator saved (see [Broker settings](#broker-settings)) |

| Files | Location | Lifecycle |
|---|---|---|
| Device images | `images/` in the data directory | Each check-in draws its image anew; one whose file is lost is drawn again even while its screen fails |
| Firmware images | The cache directory | One lost before its device downloads it is cut from TRMNL's flash image again, at most once every ten minutes for a device. The cache drops an image once its request is decided, withdrawn or replaced, or its device removed. |

The server reads TRMNL's firmware list when first needed and again once it is six hours old. A device page never waits for it and shows the firmware facts the list on hand gives; listing the boards, choosing one and installing wait for a list when the server has none.

### Sign-in

Users sign in through OIDC or with a local password. Either way the result is an account, and handlers and storage never see the method.

- The first account comes from a password sign-up or a first OIDC sign-in. Further accounts come from the OIDC provider, whose client assignment decides who may sign in.
- An OIDC sign-in is the authorization-code flow with PKCE and a nonce, bound to its browser by a cookie for ten minutes.
- The server reads the provider's keys at each sign-in, so a rotation needs no restart, and judges the ID token's expiry at its own clock.
- The provider's issuer and subject identify the account, which takes its name at the first sign-in from the profile's name, else its username.
- The server keeps the newest 64 OIDC sign-in starts, so a flood of starts can push out a real one, as a host on the network can hold the places of unclaimed registrations (see [Setup and claim](#setup-and-claim)).

### Sessions and tokens

Every management operation but the five that start a session needs a session or a personal access token. A resource of another account answers as a missing one does.

A session is a random token in an httpOnly, `SameSite=Lax` cookie, `Secure` with the `__Host-` prefix over TLS. The database holds its SHA-256, and passwords as argon2id hashes.

Scripts and agents use personal access tokens instead:

- A person signed in creates one with a name. The secret is a random token like a session's, shown once; the database holds its SHA-256.
- A request sends it as `Authorization: Bearer <secret>`, which wins over a session cookie. An `Authorization` of another scheme, such as a reverse proxy's Basic credentials, leaves the cookie to decide.

| Token | Access |
|---|---|
| Any token | Acts as its owner until it is revoked, but may not create or revoke a token or sign out, so a leaked one cannot outlive its revocation |
| Read-only token | May make only the requests the model marks `@readonly`, which are its GET requests, since Smithy's validation pairs the two |

The model declares bearer authentication, optional on every operation that takes a session too, so the generated client sends a token when it has one and the OpenAPI document tells scripts how. Two model validators keep every operation declaring one of the server's two modes and pair GET with `@readonly` for every operation a token may call.

### Ownership and administration

An account owns devices, screens and tokens. The first account, whether it signed up with a password or signed in through the OIDC provider, administers the server: only it reads and changes server settings such as the MQTT broker. There are no other roles.
