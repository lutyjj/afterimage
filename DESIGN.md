---
name: Afterimage
description: The web UI of a self-hosted server for TRMNL e-paper displays, a quiet room of cool paper where each display shows at its true pixels and one teal-blue ink marks what a hand can act on.
colors:
  ink: "oklch(48% 0.1 218)"
  ink-dark: "oklch(78% 0.085 218)"
  ink-strong: "oklch(41% 0.1 218)"
  ink-strong-dark: "oklch(85% 0.07 218)"
  ink-weak: "oklch(94.8% 0.01 218)"
  ink-weak-dark: "oklch(32% 0.024 218)"
  on-ink: "oklch(99% 0.003 218)"
  on-ink-dark: "oklch(18% 0.02 218)"
  warn: "oklch(55% 0.115 72)"
  warn-dark: "oklch(79% 0.105 80)"
  danger: "oklch(51.5% 0.155 27)"
  danger-dark: "oklch(71% 0.13 25)"
  danger-strong: "oklch(45% 0.155 27)"
  danger-strong-dark: "oklch(78% 0.11 25)"
  danger-weak: "oklch(95.6% 0.014 27)"
  danger-weak-dark: "oklch(29.6% 0.016 19)"
  bg: "oklch(97.6% 0.0045 218)"
  bg-dark: "oklch(16.5% 0.008 218)"
  surface: "oklch(100% 0 218)"
  surface-dark: "oklch(20.5% 0.009 218)"
  sunken: "oklch(95.6% 0.006 218)"
  sunken-dark: "oklch(14% 0.007 218)"
  border: "oklch(91.5% 0.008 218)"
  border-dark: "oklch(28.5% 0.011 218)"
  border-strong: "oklch(85.5% 0.013 218)"
  border-strong-dark: "oklch(36% 0.015 218)"
  text: "oklch(23% 0.015 218)"
  text-dark: "oklch(93.5% 0.006 218)"
  text-secondary: "oklch(43% 0.02 218)"
  text-secondary-dark: "oklch(75% 0.013 218)"
  text-muted: "oklch(52% 0.02 218)"
  text-muted-dark: "oklch(64% 0.013 218)"
  display-white: "#ffffff"
typography:
  headline:
    fontFamily: "Schibsted Grotesk Variable, ui-sans-serif, system-ui, -apple-system, Segoe UI, sans-serif"
    fontSize: "1.625rem"
    fontWeight: 620
    lineHeight: 1.2
    letterSpacing: "-0.022em"
  title:
    fontFamily: "Schibsted Grotesk Variable, ui-sans-serif, system-ui, -apple-system, Segoe UI, sans-serif"
    fontSize: "1.25rem"
    fontWeight: 600
    lineHeight: 1.2
    letterSpacing: "-0.018em"
  section-title:
    fontFamily: "Schibsted Grotesk Variable, ui-sans-serif, system-ui, -apple-system, Segoe UI, sans-serif"
    fontSize: "1rem"
    fontWeight: 620
    lineHeight: 1.2
    letterSpacing: "-0.012em"
  body:
    fontFamily: "Schibsted Grotesk Variable, ui-sans-serif, system-ui, -apple-system, Segoe UI, sans-serif"
    fontSize: "0.875rem"
    fontWeight: 400
    lineHeight: 1.55
  label:
    fontFamily: "Schibsted Grotesk Variable, ui-sans-serif, system-ui, -apple-system, Segoe UI, sans-serif"
    fontSize: "0.8125rem"
    fontWeight: 540
    lineHeight: 1.55
  caption:
    fontFamily: "Schibsted Grotesk Variable, ui-sans-serif, system-ui, -apple-system, Segoe UI, sans-serif"
    fontSize: "0.75rem"
    fontWeight: 400
    lineHeight: 1.55
  code:
    fontFamily: "JetBrains Mono Variable, ui-monospace, SF Mono, Menlo, Consolas, monospace"
    fontSize: "13px"
    fontWeight: 400
    lineHeight: 1.65
  wordmark:
    fontFamily: "Schibsted Grotesk Variable, ui-sans-serif, system-ui, -apple-system, Segoe UI, sans-serif"
    fontSize: "1rem"
    fontWeight: 620
    letterSpacing: "-0.02em"
rounded:
  inner: "5px"
  control: "8px"
  card: "12px"
  dialog: "16px"
  pill: "999px"
spacing:
  "1": "4px"
  "2": "8px"
  "3": "12px"
  "4": "16px"
  "5": "24px"
  "6": "32px"
  "7": "48px"
  "8": "64px"
  gutter: "16px"
  gutter-wide: "24px"
components:
  button:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.text}"
    typography: "{typography.label}"
    rounded: "{rounded.control}"
    padding: "0 14px"
    height: "34px"
  button-hover:
    backgroundColor: "{colors.ink-weak}"
    textColor: "{colors.text}"
  button-primary:
    backgroundColor: "{colors.ink}"
    textColor: "{colors.on-ink}"
    typography: "{typography.label}"
    rounded: "{rounded.control}"
    padding: "0 14px"
    height: "34px"
  button-primary-hover:
    backgroundColor: "{colors.ink-strong}"
    textColor: "{colors.on-ink}"
  button-quiet:
    textColor: "{colors.text-secondary}"
    typography: "{typography.label}"
    rounded: "{rounded.control}"
    padding: "0 14px"
    height: "34px"
  button-quiet-hover:
    backgroundColor: "{colors.ink-weak}"
    textColor: "{colors.text}"
  button-danger:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.danger}"
    typography: "{typography.label}"
    rounded: "{rounded.control}"
    padding: "0 14px"
    height: "34px"
  button-danger-hover:
    backgroundColor: "{colors.danger-weak}"
    textColor: "{colors.danger}"
  button-danger-solid:
    backgroundColor: "{colors.danger}"
    textColor: "{colors.on-ink}"
    typography: "{typography.label}"
    rounded: "{rounded.control}"
    padding: "0 14px"
    height: "34px"
  button-danger-solid-hover:
    backgroundColor: "{colors.danger-strong}"
    textColor: "{colors.on-ink}"
  button-disabled:
    backgroundColor: "{colors.sunken}"
    textColor: "{colors.text-muted}"
  icon-button:
    textColor: "{colors.text-secondary}"
    rounded: "{rounded.control}"
    size: "34px"
  input:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.text}"
    typography: "{typography.body}"
    rounded: "{rounded.control}"
    padding: "0 12px"
    height: "34px"
  card:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.text}"
    rounded: "{rounded.card}"
    padding: "{spacing.gutter}"
  sheet:
    backgroundColor: "{colors.surface}"
    rounded: "{rounded.dialog}"
    padding: "32px"
    width: "23rem"
  notice:
    backgroundColor: "{colors.ink-weak}"
    textColor: "{colors.text}"
    rounded: "{rounded.control}"
    padding: "12px 16px"
  rail:
    backgroundColor: "{colors.sunken}"
    width: "15rem"
  rail-place:
    textColor: "{colors.text-secondary}"
    rounded: "{rounded.control}"
    padding: "0 8px"
    height: "36px"
  rail-place-current:
    backgroundColor: "{colors.ink-weak}"
    textColor: "{colors.ink-strong}"
  tab:
    textColor: "{colors.text-secondary}"
    rounded: "{rounded.control}"
    padding: "0 12px"
    height: "32px"
  tab-current:
    backgroundColor: "{colors.ink-weak}"
    textColor: "{colors.ink-strong}"
  segment-checked:
    backgroundColor: "{colors.ink-weak}"
    textColor: "{colors.ink-strong}"
    typography: "{typography.label}"
    rounded: "{rounded.inner}"
  day-toggle-pressed:
    backgroundColor: "oklch(87.5% 0.024 218)"
    textColor: "{colors.ink-strong}"
    typography: "{typography.caption}"
    rounded: "{rounded.inner}"
    width: "32px"
    height: "26px"
  state-badge:
    backgroundColor: "oklch(93.7% 0.016 72)"
    textColor: "oklch(48% 0.087 73)"
    typography: "{typography.caption}"
    rounded: "{rounded.pill}"
    padding: "0 8px"
    height: "24px"
  tag:
    backgroundColor: "{colors.sunken}"
    textColor: "{colors.text-secondary}"
    typography: "{typography.caption}"
    rounded: "{rounded.pill}"
    padding: "0 8px"
    height: "22px"
  code-cell:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.text}"
    typography: "{typography.title}"
    rounded: "{rounded.control}"
  display-screen:
    backgroundColor: "{colors.display-white}"
    rounded: "{rounded.inner}"
    width: "800px"
---

# Design System: Afterimage

## Overview

**Creative North Star: "The Quiet Room"**

The displays are the subject, and the room around them is quiet. Every screen a device shows sits at its true pixels, white as e-paper is, on a cool paper ground; everything else steps back so a display's own black and white stay the loudest thing on the page. All neutrals share the ink's hue (218) at a whisper of chroma, so the room is one temperature, and the only colour that speaks is a single teal-blue ink that marks what a hand can act on. Amber appears only where a state wants attention; red only where something is removed or has gone wrong.

The density is that of a calm working tool. A sunken rail holds the wordmark, three places and the account; the content column holds white cards on hairlines, one card per concern, and text ranks by weight and colour before size. Light and dark are one system: each colour is a single `light-dark()` pair in `web/src/app/global.css`, the page follows the system scheme, and Settings can pin either theme.

The world refuses the category's dashboard of status tiles and pills. A device's facts are a quiet two-column list, its day is a grey strip with a line at now, and a badge appears only for a state that needs the person. There are no mascots, illustrations or playful flourishes. The ornament the system allows is its name made visible: the mark is a square and the fainter square it leaves behind, and when a display's image changes, the one before lingers under it and fades, as e-paper ghosts after a refresh.

**Key Characteristics:**
- Displays at 1:1 (up to 800 pixels wide) on white, edged by a hairline, with palette, size and scale beneath.
- One teal-blue ink at hue 218 for links, the current place, focus, the primary action and what a hand has set.
- Cool paper neutrals at the ink's hue; white cards on 1px hairlines with 12px corners.
- Schibsted Grotesk for everything; JetBrains Mono only for what is typed or copied verbatim.
- Flat at rest; the one shadow belongs to what floats.
- Light and dark from one `light-dark()` pair per colour.

## Colors

One ink on cool paper, with two state colours held in reserve. Each frontmatter key is the light value of a custom property in `web/src/app/global.css` (`ink` is `--ink`), and its `-dark` sibling is the same property's dark value; the two are always declared together in one `light-dark()`.

### Primary
- **Afterimage Teal** (`ink`): the hue an afterimage takes, the cool complement a warm light leaves behind. It fills the primary button, colours links, draws every focus outline, marks the active code cell, the editor's caret and layout guides, the finished legs of a firmware install, and the wordmark's square.
- **Deep Teal** (`ink-strong`): the ink one step firmer, for the primary button and links on hover, and for the text of the current place, the current settings tab and a checked segment.
- **Teal Wash** (`ink-weak`): the ink mixed into the surface (10% light, 20% dark, in oklch). The fill of the current place, of hovered buttons, places and rows, of notices, the token shown once, and the account initial.
- **On Ink** (`on-ink`): text on an ink or danger fill.

### Secondary
- **Attention Amber** (`warn`): a state that wants the person's attention and nothing else. It colours the dot and, mixed into the surface and the text in oklab, the wash and label of a state badge (not checked in, not seen lately, nothing scheduled), and the dot beside unsaved changes in the editor.

### Tertiary
- **Removal Red** (`danger`, `danger-strong`, `danger-weak`): removal and errors. The outlined danger button, the solid button that confirms a removal, the wash behind a destructive confirmation or a chosen removal option, invalid fields, error text, editor problems and a failed firmware leg.

### Neutral
- **Cool Paper** (`bg`): the page ground.
- **Card White** (`surface`): cards, fields, buttons, the editor and the sign-in sheet. In the dark theme it is the lifted near-black that cards sit on.
- **Sunken Paper** (`sunken`): below the page: the rail, an empty display, day-strip tracks, wells for inline code and copyable values, disabled buttons and tags.
- **Hairline** (`border`): card edges, row dividers and rules.
- **Strong Hairline** (`border-strong`): control borders, the ring around a display, the major hours of a day strip.
- **Deep Slate** (`text`): names, values and running text.
- **Slate** (`text-secondary`): descriptions, hints, field labels and places at rest.
- **Muted Slate** (`text-muted`): text about a thing rather than the thing: fact labels, captions, units, times, table headings.
- **Display White** (`display-white`): the ground of every display's screen in both themes. E-paper is white even when the room is dark.

### Named Rules
**The One Ink Rule.** The ink marks what a hand can act on or has set, and nothing else. If a surface needs ink to look finished, it is decorating.

**The One Temperature Rule.** Every neutral is hue 218 at chroma 0.02 or less. A new neutral takes the same hue; pure greys and warm greys do not belong in the room.

**The Held Reserve Rule.** Amber appears only where a state wants attention; red only for removal and errors. Neither colours a heading, an icon, a chart or a decoration.

## Typography

**Body Font:** Schibsted Grotesk Variable (with ui-sans-serif, system-ui, -apple-system, Segoe UI, sans-serif)
**Label/Mono Font:** JetBrains Mono Variable (with ui-monospace, SF Mono, Menlo, Consolas, monospace)

**Character:** A sturdy grotesk with a newspaper's firmness sets everything from the page title to the caption; there is no display face. Its variable weight carries the hierarchy in small steps. The mono is a tool, not a voice: it appears only where characters must be read exactly.

### Hierarchy
- **Headline** (620, 1.625rem, 1.2, -0.022em): the page title, one per page, with a line in Slate beneath it.
- **Title** (600, 1.25rem, 1.2, -0.018em): a display's name on its card, the screen's name in the editor (620), the title of an empty state.
- **Section title** (620, 1rem, 1.2, -0.012em): a card's title, the claim panel's and the sign-in form's.
- **Body** (400, 0.875rem, 1.55): running text, fact values and table cells. Forms and settings hold to a 44rem measure.
- **Label** (540, 0.8125rem, 1.55): buttons, field labels and table headings. Hints and notes take the same size at 400 in Slate or Muted Slate.
- **Caption** (400, 0.75rem, 1.55): a display's caption, a day strip's names and hours. Badges and tags take it at 560 to 580.
- **Code** (JetBrains Mono, 400, 13px, 1.65): the YAML editor; inline at 0.9em; a source's data at 12px.
- **Wordmark** (620, 1rem, -0.02em): the lowercase name after its mark.

### Named Rules
**The Weight Before Size Rule.** Hierarchy comes from weight and colour before size. The scale tops out at 26px, and a step down in importance is a step from Deep Slate to Slate to Muted Slate before it is a smaller size.

**The Verbatim Mono Rule.** JetBrains Mono sets only what is typed or copied exactly: YAML, addresses, commands, secret and source names, a source's data, a problem's location. Prose, labels and headings never take it.

**The Sentence Case Rule.** Everything is in sentence case. Letter-spacing only tightens headings and the wordmark; nothing is uppercased or tracked out.

Figures that are read in columns (times, hours, a claim code) are tabular; prose keeps proportional figures.

## Layout

The signed-in frame is a two-column grid: a 15rem rail on Sunken Paper, its right edge a 1px hairline that runs the full height, and the content column. The rail is sticky at full viewport height: the wordmark at the top, the places (Devices, Screens, Settings) beneath, and the account at the foot above a hairline. The screen editor folds the rail to 3.75rem of icons so the YAML and the preview share the width.

The content column caps at 74rem plus 32px of padding each side, padded 32px at the top and 64px at the foot. A page's head holds its title and a line about it at the left and its actions at the right, aligned at the bottom, and its sections follow 24px apart. A page of forms keeps the same left edge at a 44rem measure. Within a card, a gutter of 16px (24px from 48rem) is the inset every title and value starts on; rows run edge to edge under hairlines.

A display's card sets its screen (up to 800px, 1:1) beside a details column of at least 15rem, 24px apart; below 72rem the two stack, screen first. The screen editor fills the viewport: its head, then the YAML (at least 24rem) beside a preview column as wide as the display plus room for its scrollbar, each scrolling on its own; below 64rem they stack, the YAML at 60% of the viewport height and the preview centred beneath.

Below 52rem the rail becomes a sunken bar along the top with a hairline under it: the mark, then the places, which scroll sideways before they would widen the page; signing out moves to Settings, and the page padding drops to 24px 16px 48px. Below 30rem the places keep only their icons. Below 40rem two-column forms become one and the settings tabs scroll sideways under a fade.

Spacing runs on a 4px base: 4, 8, 12, 16, 24, 32, 48 and 64px.

## Elevation & Depth

The system is flat and tonal. Depth is three levels of paper: Sunken Paper below the Cool Paper page below Card White, with 1px hairlines where a level meets the next. A card does not lift; it is white on a hairline. The one shadow belongs to what floats over the page: the editor's completion list and hover tooltips. `box-shadow` otherwise serves as a line, never as depth: the 1px ring that edges a display and the ink halo on the active cell of a claim code.

### Shadow Vocabulary
- **Overlay** (`box-shadow: 0 10px 28px -10px light-dark(oklch(22% 0.03 218 / 0.22), oklch(0% 0 0 / 0.55))`): only for what floats over the page.
- **Display edge** (`box-shadow: 0 0 0 1px var(--border-strong)`): the hairline around a display's screen, so a blank one still reads as an object.
- **Cell focus** (`box-shadow: 0 0 0 3px color-mix(in oklch, var(--ink) 18%, transparent)`): the active cell of the claim code.

### Named Rules
**The Only What Floats Rule.** Nothing at rest casts a shadow. The overlay shadow is for what floats over the page and for nothing else.

## Shapes

Corners follow one ladder: a control and the field beside it share 8px; a card takes 12px; what sits inside a control (a segment, a day toggle, an inline code well, a day-strip track) and a display's screen take 5px; the sign-in sheet takes 16px. Badges, tags and the spans of a day strip are full pills; the account initial and step counters are circles. Edges are 1px hairlines throughout; the only heavier line is the 3px top rule of each firmware leg.

The mark is two 9 by 9 squares with 1.5px corners: a filled one in the ink, and behind it, 4px down and to the right, its outline in the ink at 45% with a 1.25px stroke.

## Components

### Buttons
Quiet and solid: a hairline box that tints toward the ink under the hand.
- **Shape:** gently rounded (8px), 34px tall, 14px side padding, 13px label at 540, a 16px Lucide icon leading when it helps (a plus to create).
- **Default:** Card White on a Strong Hairline border, Deep Slate text. Hover fills Teal Wash and moves the border 35% toward the ink; pressed fills the ink at 16%.
- **Primary:** an ink fill with On Ink text; hover to Deep Teal. One per page head, card or form, for the action it exists for (Claim a display, New screen, Save playlist).
- **Quiet:** no fill or border, Slate text; hover fills Teal Wash. For Cancel and secondary tools.
- **Danger:** Removal Red text on Card White with a border 45% red; hover fills the red wash. It opens a confirmation, never acts on its own.
- **Danger solid:** a red fill with On Ink text, hover darker; only the final button of a confirmation that has said what it does to the device.
- **Disabled:** Sunken Paper fill, Hairline border, Muted Slate text.
- **Icon button:** a 34px square, transparent, Slate icon; hover fills Teal Wash.
- **Focus:** a 2px ink outline at 2px offset. Colour transitions take 150ms on the ease-out curve.

### Chips
- **State badge:** a 24px pill in an amber wash with amber-on-slate text at 12px 580 and a 6px amber dot leading. It names a device state that wants attention, beside the device's name. A state that ends while the page is open stays, struck through, at 55% opacity with a grey dot (a 320ms fade), rather than vanishing under the reader's eye.
- **Tag:** a 22px pill on Sunken Paper with Slate text at 12px 560, for a token's scope or an account's role.
- **Segmented control:** a Strong Hairline outline with 8px corners and 3px padding around 5px-cornered options; the checked one fills Teal Wash with Deep Teal text at 620. The week's day toggles in a playlist entry use the same frame; a pressed day fills the ink at 24% into the surface, at 680.

### Cards / Containers
- **Corner Style:** 12px.
- **Background:** Card White on the Cool Paper page.
- **Shadow Strategy:** none; see Elevation & Depth.
- **Border:** a 1px Hairline.
- **Internal Padding:** the gutter, 16px (24px from 48rem). A card's title (Section title) and a hint in Slate head it; its body follows with 16px gaps. Rows run edge to edge under a hairline, a label at 560 and a hint at the left, the control or value at the right.
- **Notice:** a Teal Wash fill with 8px corners for a message the page wants read (a display just claimed, a token's secret shown once). A confirmation for a non-destructive step sits on Sunken Paper; a destructive one sits on the red wash with a 40% red border.
- **Table:** the screens list is a table on a card: headings at 13px 540 in Muted Slate over a hairline, names at 580 in Deep Slate, other cells in Slate; a row's hover fills Teal Wash at 60%.

### Inputs / Fields
- **Style:** 34px tall, 8px corners, a 1px Strong Hairline border on Card White, 14px text, 12px side padding. The label sits 6px above in 13px 540 Slate; a hint follows below in 13px Slate. A select draws a Lucide chevron at its right.
- **Focus:** the border turns to the ink inside a 2px outline of the ink at 45%. Hover moves the border 30% toward the ink.
- **Error / Disabled:** an invalid field takes a red border and red 13px error text beneath.
- **Claim code:** six cells at a 5:6 ratio drawn over one real input that takes every keystroke and paste; 20px figures at 620, tabular. A filled cell's border moves 40% toward the ink; the active cell takes the ink border and the cell-focus halo; an invalid code turns every cell red.
- **YAML editor:** CodeMirror on a 12px card, 13px JetBrains Mono at 1.65. Keys in Deep Slate at 600, values in Slate, comments italic in Muted Slate; a 2px ink caret, the active line in Teal Wash at 55%, problems underlined in a red wave. The card's border moves 55% toward the ink while it has focus.

### Navigation
- **Rail places:** 36px rows with 8px corners, a 16px Lucide icon (1.75 stroke) and the name at 520 in Slate. Hover fills Teal Wash with Deep Slate text; the current place fills Teal Wash with Deep Teal text at 600.
- **Account:** a 28px circle with the initial in Deep Teal on Teal Wash (12px, 650), the name, and a sign-out icon button, above a hairline at the rail's foot.
- **Settings tabs:** the rail's treatment laid along the top of the page, 32px tall, over a hairline.
- **Back link:** 13px Slate with a 14px arrow, above the page title; hover to Deep Teal.
- **Mobile:** below 52rem the rail is a sunken top bar and the places scroll sideways; below 30rem they are icons with their names kept for assistive technology.

### Display Panel (signature)
A display's image as the display holds it, and the afterimage it leaves.
- **Screen:** Display White in both themes, 5px corners, edged by the display-edge ring. It is as wide as the image's natural width where the column allows (1:1 at up to 800px) and narrower otherwise, holding the display's aspect ratio before the image loads; at 1:1 it renders pixelated so every device pixel stays crisp.
- **Caption:** 12px in Muted Slate beneath: the palette's levels as 10px swatches with a faint inset ring (up to 16; a rainbow band for full colour), the palette's name, the size in pixels, and the scale, "1:1" or "at 41%".
- **Empty:** Sunken Paper with the mark at 40px and 55% opacity above "No image yet" in 13px Muted Slate.
- **Busy:** while the image is replaced, the screen dims to 60% over 240ms.
- **Afterimage:** when the image changes, the one before is laid over the new one with `mix-blend-mode: multiply`, so it darkens the new image only where it was dark, and fades from 40% opacity to none while blurring to 1.5px over 2000ms on `cubic-bezier(0.45, 0, 0.55, 1)`; it is removed when the animation ends. Under reduced motion it lasts 1ms.
- **Guides:** in the editor's preview, a layout grid can be drawn over the image: lines every 40 device pixels in the ink at 22%, every 200 at 50%, and dashed centre lines at 75%.

### Day Strip
- **Track:** a day from midnight to midnight on Sunken Paper with 5px corners, one 20px lane per playlist entry, a line every three hours in Hairline at 60% and every six hours in Strong Hairline.
- **Entries:** grey pills (Muted Slate at 70% into the surface), inset 5px in their lane, as long as the entry is current. The strip reports; it never takes the ink.
- **Now:** a 2px line in Deep Slate reaching 4px past the track.
- **Scale:** 00, 06, 12, 18 and 24 beneath in 12px tabular Muted Slate; entry names in a 6.5rem column at the left when the strip shows several.

### Firmware Progress
An install shows its three legs (requested, sent, installed) as columns under 3px top rules: a finished leg in the ink, the current one dashed in the ink at 45%, a waiting one in Hairline with Muted Slate text, a failed one in red with its label at 640.

### Wordmark
The lowercase name at 16px 620 after the 16px mark. The mark alone stands for the product where the name does not fit: the folded rail, and an empty display.

## Do's and Don'ts

### Do:
- **Do** show every display image at its true pixels where the column allows (1:1 up to 800px), on Display White with the display-edge ring, with palette, size and scale beneath it.
- **Do** keep the ink (oklch(48% 0.1 218) light, oklch(78% 0.085 218) dark) for links, the current place, focus, the primary action and what a hand has set.
- **Do** declare every colour as one `light-dark()` pair at hue 218 and check both themes.
- **Do** put each concern on its own white card: 12px corners, a 1px hairline, a 16px gutter that widens to 24px from 48rem.
- **Do** rank text by weight and by Deep Slate, Slate and Muted Slate before size; keep body text at 14px on 1.55 and forms at a 44rem measure.
- **Do** show a device state that wants attention as an amber badge beside its name, and keep a state that ends while the page is open, struck through at 55% opacity.
- **Do** open a destructive action with an outlined danger button, say in the confirmation what it does to the device, and finish it with a solid danger button beside a quiet Cancel.
- **Do** reserve the afterimage for display images: the image before, multiplied over the new one, fading from 40% to none and blurring to 1.5px over 2000ms, and 1ms under reduced motion.

### Don't:
- **Don't** build dashboards of status tiles, stat cards or rows of status pills; a device's facts are a two-column list of Muted Slate labels and plain values.
- **Don't** use the ink as decoration: no ink headings, bands, gradients or tinted icons; an icon takes the colour of its text.
- **Don't** introduce pure or warm greys; every neutral is hue 218 at chroma 0.02 or less.
- **Don't** give anything at rest a shadow for depth; the overlay shadow is only for what floats.
- **Don't** use amber or red for anything but a state that wants attention, removal and errors.
- **Don't** set prose, labels or headings in JetBrains Mono, and don't uppercase or track out a label.
- **Don't** draw on, tint or crop a display's image, or scale it above its true pixels; the editor's optional guides are the only marks laid over it.
- **Don't** add mascots, cute illustration or playful copy; Afterimage is a calm tool.
