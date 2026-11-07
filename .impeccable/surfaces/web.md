---
version: 1
slug: "web"
primary_target: "web"
related_targets: []
---

# Web UI

Scope: the whole Afterimage web UI (`web/`): sign-in and first account, devices (claim, list, detail, playlist, firmware, remove), screens (list, document editor with live preview), settings (preferences, access tokens and MCP, Home Assistant broker, server facts). Visitor mode: Operate.

Audience and job: the person who runs the server, at a desk writing screens and on a phone claiming a device next to it; household members glancing at their devices. Success: a claimed device shows the right screen with no further attention; a screen is written and checked without leaving the editor. Constraints: standard web controls; must not read as a toy or gimmick; the vibe of linen's other apps (calm, stylish, thoughtful: a quiet rail, hairline cards, one ink, light and dark) with Afterimage's own identity.

## Direction contract

THESIS: The displays are the subject; the room around them is quiet. Every screen a device shows sits at its true pixels on a calm, cool paper, and the only colour that speaks is the one ink a hand can act on. It refuses the category's dashboard of status tiles and pills.

OWN-WORLD: Cool paper neutrals at hue 218 with a whisper of chroma, white cards on hairlines (12px radius), one teal-blue ink (oklch 48% 0.1 218 light, 78% 0.085 dark) for links, the current place, focus and the primary action; amber only for a state that wants attention, red only for removal. Schibsted Grotesk for everything, JetBrains Mono for YAML and names. Light and dark from one light-dark() per colour; no shadows except what floats.

STORY: The visitor sees each display as it is now, with its facts and today's playlist beside it, and knows which need them; claims a new display by typing its code; writes a screen while the preview redraws; finds tokens, the MCP address and the broker in Settings.

FIRST VIEWPORT: A sunken rail at the left: the wordmark (a square and its fainter afterimage), Devices, Screens, Settings, and the account at the foot. The content column holds "Devices", a line naming the playlists' time zone, and "Claim a display" at the right; under it one card per display: its screen at 1:1 (800 pixels) on the left, its name with any state badge, its facts, and today's timeline on the right.

FORM: Pinned: linen's calm working-tool vibe, with Afterimage's own palette and mark. Signature move: the afterimage: when a panel's image changes, the one before lingers under it, multiplied and fading over two seconds, as e-paper ghosts after a refresh.

FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, DESIGN.md, and every shipping raster carrying its provenance

## Unresolved

- Editor: CodeMirror 6 with YAML and codemirror-json-schema, the schema taken from `gen/management-api.openapi.json` (ScreenDocument and its components); problems mapped from JSON Pointer to YAML ranges with the `yaml` package.
