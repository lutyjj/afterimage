---
name: Afterimage
description: The web UI of a self-hosted server for TRMNL e-paper displays, a quiet room of cool paper where each display shows at its true pixels and one teal-blue ink marks what a hand can act on.
colors:
  ink: "oklch(46% 0.11 218)"
  ink-dark: "oklch(76% 0.105 218)"
  bg: "oklch(98.3% 0.004 218)"
  bg-dark: "oklch(17.5% 0.008 218)"
  surface: "oklch(100% 0 218)"
  surface-dark: "oklch(21% 0.009 218)"
  sunken: "oklch(95.8% 0.006 218)"
  sunken-dark: "oklch(14.5% 0.007 218)"
  border: "oklch(91.5% 0.008 218)"
  border-dark: "oklch(28% 0.012 218)"
  border-strong: "oklch(86.5% 0.014 218)"
  border-strong-dark: "oklch(34% 0.016 218)"
  text: "oklch(22% 0.016 218)"
  text-dark: "oklch(93% 0.007 218)"
  text-secondary: "oklch(45% 0.02 218)"
  text-secondary-dark: "oklch(73% 0.014 218)"
  text-muted: "oklch(52.5% 0.02 218)"
  text-muted-dark: "oklch(61% 0.014 218)"
  warn: "oklch(53.5% 0.11 80)"
  warn-dark: "oklch(74% 0.09 82)"
  danger: "oklch(50% 0.14 27)"
  danger-dark: "oklch(68% 0.11 25)"
  positive: "oklch(52% 0.1 158)"
  positive-dark: "oklch(72% 0.09 158)"
  epaper: "#ffffff"
typography:
  screen-title:
    fontFamily: "Schibsted Grotesk Variable, ui-sans-serif, system-ui, sans-serif"
    fontSize: "20px"
    fontWeight: 600
    lineHeight: "28px"
    letterSpacing: "-0.01em"
  card-title:
    fontFamily: "Schibsted Grotesk Variable, ui-sans-serif, system-ui, sans-serif"
    fontSize: "16px"
    fontWeight: 600
    lineHeight: "24px"
  body:
    fontFamily: "Schibsted Grotesk Variable, ui-sans-serif, system-ui, sans-serif"
    fontSize: "14px"
    fontWeight: 400
    lineHeight: "22px"
  label:
    fontFamily: "Schibsted Grotesk Variable, ui-sans-serif, system-ui, sans-serif"
    fontSize: "13px"
    fontWeight: 500
    lineHeight: "20px"
  caption:
    fontFamily: "Schibsted Grotesk Variable, ui-sans-serif, system-ui, sans-serif"
    fontSize: "12px"
    fontWeight: 400
    lineHeight: "16px"
  code:
    fontFamily: "JetBrains Mono Variable, ui-monospace, monospace"
    fontSize: "13px"
    fontWeight: 400
    lineHeight: "1.65"
rounded:
  input: "6px"
  button: "8px"
  card: "12px"
  dialog: "16px"
spacing:
  base: "4px"
  gutter: "12px"
  gutter-wide: "16px"
components:
  button:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.text}"
    typography: "{typography.label}"
    rounded: "{rounded.button}"
    height: "32px"
  button-primary:
    backgroundColor: "{colors.ink}"
    textColor: "{colors.bg}"
    typography: "{typography.label}"
    rounded: "{rounded.button}"
    height: "32px"
  button-destructive:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.danger}"
    typography: "{typography.label}"
    rounded: "{rounded.button}"
    height: "32px"
  input:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.text}"
    rounded: "{rounded.button}"
    height: "32px"
  card:
    backgroundColor: "{colors.surface}"
    rounded: "{rounded.card}"
  display:
    backgroundColor: "{colors.epaper}"
    rounded: "{rounded.input}"
---

# Design System: Afterimage

## Overview

**Creative North Star: "The Quiet Room"**

The displays are the subject, and the room around them is quiet. Every screen a device shows sits at its true pixels, white as e-paper is, on cool paper; everything else steps back so a display's own black and white stay the loudest thing on the page.

Afterimage wears [linen](https://github.com/lutyjj/linen), the design language it shares with its sibling apps. linen's `LANGUAGE.md` owns the shared rules: one ink, paper neutrals, type voices, the shell, cards, setting rows, controls, dialogs and motion. This file records only what Afterimage adds.

**Key Characteristics:**

- Hue 218, teal-blue: the cool complement a warm light leaves behind.
- Schibsted Grotesk for every word; JetBrains Mono for what is typed or copied verbatim.
- Displays at 1:1 (up to 800 pixels) on e-paper white in both themes.
- The mark: a square and the fainter square it leaves behind.
- When a display's image changes, the one before lingers and fades.

## Colors

`web/src/app/app.css` sets `--hue: 218` and the faces; linen mixes every other colour from them. The frontmatter lists each token's light value, and its `-dark` sibling the dark one.

| Token | Marks |
|---|---|
| `ink` | What a hand can act on or has set: links, the current place, focus, the primary action, chosen days, the caret, layout guides, a finished firmware leg |
| `warn` | A device state that wants the person: not checked in, not seen lately, nothing scheduled, an empty playlist |
| `danger` | Removal and failure: destructive buttons, editor problems, a failed firmware leg |
| `positive` | A broker that is connected |
| `epaper` | The ground of every display's screen, in both themes |

## Typography

| Voice | Use |
|---|---|
| Screen title (20px, 600) | A screen's name in its top bar, a display's name on its card |
| Card title (16px, 600) | A card's head |
| Body (14px) and label (13px, 500) | Everything else; fact labels in `text-muted` |
| Code (JetBrains Mono, 13px) | YAML, addresses, commands, topics, secret and source names |

## Layout

- linen's `Shell` holds every signed-in screen: the rail on a desk, and on a phone a bar along the top with the wordmark and the places, which scrolls away and leaves the whole screen to the content. The screen editor holds the rail folded so the YAML and a 1:1 preview share the width.
- A display's card sets its screen beside its facts (at least 15rem) from 72rem, stacked below.
- The editor is `wide`: its YAML (at least 24rem) beside a preview column as wide as the display, each scrolling on its own; below 64rem they stack.
- A display, a playlist and a screen name the list above them in a small trail over their title ("Devices /" over "Living room").

## Components

| Component | What it is |
|---|---|
| Display panel | The image as the display holds it: e-paper white, 6px corners, a 1px ring in `border-strong`, pixelated at 1:1. Beneath: palette swatches, palette name, size and scale. Empty: the mark at 40px over "No image yet". Busy: 60% opacity. |
| Afterimage | When the image changes, the previous one is laid over it with `mix-blend-mode: multiply` and fades from 40% to none while blurring to 1.5px over 2000ms (`animate-afterimage`); 1ms under reduced motion. |
| Day track | A day from midnight to midnight on `sunken`, each stretch as long as it lasts and named where it fits: a playlist or pinned screen lit as paper (`surface` with a `border-strong` hairline), sleep as the bare track, nothing scheduled in dashed `warn`; a line at now in `text`. A display's week is seven tracks, today's day in `text`. It reports; it never takes the ink. |
| Claim code | Six 5:6 cells over one real input that takes typing and paste; the active cell takes the ink border and a 3px halo, an invalid code turns every cell red. |
| Firmware legs | Requested, sent, installed as columns under 3px rules: done in ink, current dashed, waiting in `border`, failed in `danger`. |
| State badge | A row chip in `warn` with a dot. A state that ends while the page is open stays, struck through at 55% opacity. |
| Wordmark | The lowercase name at 16px after the 16px mark; the mark alone heads the folded rail. Either leads to the first screen. |
| Save button | The editor's one primary act. While there is something to save it is in ink and reads "Save"; once saved it turns quiet and reads "Saved", keeping its width. Delete stands beside it as a trash icon. |

## Do's and Don'ts

### Do:

- **Do** show every display image at its true pixels where the column allows, on e-paper white, with palette, size and scale beneath it.
- **Do** take every control, card, dialog and motion from linen; add a component here only for what is Afterimage's own.
- **Do** say in a confirmation what an act does to the device before it happens.

### Don't:

- **Don't** draw on, tint or crop a display's image, or scale it above its true pixels; the editor's optional guides are the only marks laid over it.
- **Don't** build dashboards of status tiles; a device's facts are a two-column list.
- **Don't** add mascots, cute illustration or playful copy; Afterimage is a calm tool.
