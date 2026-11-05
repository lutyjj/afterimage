---
version: 1
slug: "web"
primary_target: "web"
related_targets: []
---

# Web UI

Scope: the whole Afterimage web UI (`web/`): sign-in and first account, devices (claim, list, detail, playlist, remove), screens (list, document editor with live preview). Visitor mode: Operate.

Audience and job: the person who runs the server, at a desk writing screens and on a phone claiming a device next to it; household members glancing at their devices. Success: a claimed device shows the right screen with no further attention; a screen is written and checked without leaving the editor. Constraints: standard web controls; must not read as a toy or gimmick; the operator asked for "easy to use, clear, stylish and tasteful design with intent".

## Direction contract

THESIS: Every screen is a proof before it goes to ink. The UI is the proofroom: everything Afterimage itself says is drawn in non-repro blue, the pencil that never prints; everything a display will show is true ink on paper. It refuses the category default of a dark sidebar with a card grid of equal tiles.

OWN-WORLD: Warm proofing-paper ground (about #f7f6f1), true ink (#1b1b19) only for what displays show and for primary text, non-repro blue (about #2f6fd6, wash #e6eefb, rules #c7d8f5) for chrome, labels, metadata and navigation, one proofer's red (#e2483d) reserved for stamps and destructive actions. One sturdy grotesk for UI with tabular figures; a narrow cut for stamps and codes. Hairline blue rules, generous whitespace, no shadows, no rounded cards.

STORY: The visitor sees each display as its current proof and knows at a glance which ones need them (waiting, not seen, nothing scheduled); claims a new display by typing its code; writes a screen while watching its proof update; arranges when each screen shows on a day strip.

FIRST VIEWPORT: Top bar in blue on paper: wordmark, Devices and Screens tabs, account at the right. Devices list as rows: on the left the device's live image framed as a proof (crop marks at two corners, a step wedge of its palette's levels beneath, image at true pixels); on the right its name, display model and palette, battery, signal, last seen, and a day strip of its playlist where each entry's length is its time window. States as stamps in proofer's red that cancel rather than vanish. A claim panel at the right on desktop, at the top on a phone: a six-cell code field and one button.

FORM: Proof sheet (pre-press proof with non-repro blue annotation), candidate 6 of 7 on the ordered list; signature move: each display image framed as a proof with its palette's step wedge, states as stamps. Kept from declined challengers: pixel-exact previews (Ikeda), a layout-grid toggle in the preview (Crouwel), stamped states that cancel (ticket wallet), playlist length equals duration (labanotation). seed:5dc4707cb2b63149

FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, DESIGN.md, and every shipping raster carrying its provenance

## Unresolved

- The operator did not pick a card on the decision page; this direction is the roll's assignment, taken under their steer above. Revisit if the built result does not read as stylish and tasteful.
- Editor: CodeMirror 6 with YAML and codemirror-json-schema, the schema taken from `gen/management-api.openapi.json` (ScreenDocument and its components); problems mapped from JSON Pointer to YAML ranges with the `yaml` package.
