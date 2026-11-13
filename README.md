# Afterimage

Lightweight [TRMNL](https://trmnl.com) BYOS server, written in Rust and TypeScript. The goal of the project is to keep it simple, performant, and hella easy to maintain. We don't reinvent the wheel: displays run TRMNL's stock firmware, and screens are built from TRMNL's own components and design system.

- Everything is defined in [Smithy](https://smithy.io). One model generates the server, the web UI's client, the OpenAPI docs and the MCP tools, so they can't drift apart
- Proper MQTT controls for every display, OTA updates included, so Home Assistant automations can put a display to sleep, pin a screen, install firmware or react to its button
- Screens are small YAML docs rendered natively with [Typst](https://typst.app). No headless browser, ~13 ms a screen
- OTA firmware for every TRMNL board, served locally
- Playlists, weekly schedules, and sleep windows that save battery
- Scripts and agents get the same API as the UI, plus an MCP endpoint
- Password or OIDC sign-in

How it works under the hood: [docs/design.md](docs/design.md).

## Screenshots

![Devices](docs/screenshots/devices.png)

![Screen editor](docs/screenshots/editor.png)

![Schedule](docs/screenshots/schedule.png)

<img src="docs/screenshots/phone.png" alt="Devices on a phone, dark theme" width="320">

## Installation

### Docker

```sh
docker run -d -p 8080:8080 -v afterimage:/data -e TZ=Europe/Berlin ghcr.io/lutyjj/afterimage
```

### NixOS

```nix
imports = [ inputs.afterimage.nixosModules.default ];
services.afterimage = {
  enable = true;
  openFirewall = true;
};
```

| Option | Default |
|---|---|
| `listenAddress`, `port` | `0.0.0.0`, `8080` |
| `openFirewall` | `false` |
| `settings` | `{ }`, the variables below |
| `environmentFile` | A file of secret variables |

### First display

1. Open `http://<host>:8080` and make the first account (it's the admin).
2. In the display's Wi-Fi portal, set the server URL to `http://<host>:8080`.
3. Enter the code the display shows under **Claim a display**.

## Configuration

Everything is an environment variable (`afterimage --help` shows the matching flags).

| Variable | Default | Sets |
|---|---|---|
| `AFTERIMAGE_BIND` | `0.0.0.0:8080` | Listen address |
| `AFTERIMAGE_DATA_DIR` | `data` | Database and images |
| `AFTERIMAGE_CACHE_DIR` | `<data>/cache` | Firmware images |
| `AFTERIMAGE_URL` | | Public URL, such as `https://afterimage.example`; needed for OIDC |
| `AFTERIMAGE_TIME_ZONE` | `TZ`, else the host's | Time zone of schedules and screens |
| `AFTERIMAGE_WEB_DIR` | The packaged web UI | Built web UI |
| `AFTERIMAGE_TRMNL_URL` | `https://trmnl.com` | Where firmware comes from |
| `AFTERIMAGE_PASSWORD_SIGN_IN` | `true` | Password sign-in; `false` needs OIDC |
| `AFTERIMAGE_OIDC_ISSUER` | | OIDC provider; redirect URL is `<AFTERIMAGE_URL>/manage/auth/oidc/callback` |
| `AFTERIMAGE_OIDC_CLIENT_ID`, `_CLIENT_SECRET` | | OIDC client |
| `AFTERIMAGE_OIDC_NAME` | `Single sign-on` | Sign-in button label |
| `AFTERIMAGE_MQTT_URL` | | `mqtt://` or `mqtts://` broker; leave unset to set it in the web UI |
| `AFTERIMAGE_MQTT_USERNAME`, `_PASSWORD` | | Broker credentials |
| `AFTERIMAGE_MQTT_TOPIC` | `afterimage` | Base topic |
| `AFTERIMAGE_MQTT_DISCOVERY_PREFIX` | `homeassistant` | Home Assistant discovery prefix |
| `AFTERIMAGE_SECRET_<NAME>` | | A secret data sources read as `{{ secrets.<NAME> }}` |

For scripts and agents, make a token in **Settings → Tokens and MCP** and send it as `Authorization: Bearer <token>`. The API lives at `/manage/` (its OpenAPI doc is attached to each [release](https://github.com/lutyjj/afterimage/releases)), MCP at `/mcp`.

## License

[MIT](LICENSE)
