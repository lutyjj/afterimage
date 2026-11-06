---
name: Afterimage
description: The web UI of a self-hosted server for TRMNL e-paper displays, drawn as a proofroom.
colors:
  paper: "#f7f6f1"
  white: "#ffffff"
  ink: "#1b1b19"
  ink-soft: "#4b4a45"
  blue: "#2f6fd6"
  blue-text: "#2659b8"
  blue-deep: "#1f4d9f"
  blue-wash: "#e6eefb"
  blue-rule: "#c7d8f5"
  blue-edge: "#9bb8eb"
  blue-gray: "#5f6f8f"
  blue-faint: "#8aa6d9"
  red: "#e2483d"
  red-text: "#c2372d"
  red-wash: "#fbe9e7"
  red-deep: "#a82d24"
typography:
  wordmark:
    fontFamily: "Archivo Variable, Archivo, system-ui, sans-serif"
    fontSize: "1.25rem"
    fontWeight: 640
    letterSpacing: "-0.015em"
    fontVariation: "'wdth' 118"
  headline:
    fontFamily: "Archivo Variable, Archivo, system-ui, sans-serif"
    fontSize: "1.5rem"
    fontWeight: 620
    lineHeight: 1.15
    letterSpacing: "-0.01em"
  title:
    fontFamily: "Archivo Variable, Archivo, system-ui, sans-serif"
    fontSize: "1.25rem"
    fontWeight: 620
    lineHeight: 1.15
    letterSpacing: "-0.01em"
  body:
    fontFamily: "Archivo Variable, Archivo, system-ui, sans-serif"
    fontSize: "0.9375rem"
    fontWeight: 400
    lineHeight: 1.45
    fontFeature: "'tnum' 1"
  body-small:
    fontFamily: "Archivo Variable, Archivo, system-ui, sans-serif"
    fontSize: "0.8125rem"
    fontWeight: 400
    lineHeight: 1.45
  annotation:
    fontFamily: "Archivo Variable, Archivo, system-ui, sans-serif"
    fontSize: "0.6875rem"
    fontWeight: 560
    letterSpacing: "0.06em"
    fontVariation: "'wdth' 72"
  stamp:
    fontFamily: "Archivo Variable, Archivo, system-ui, sans-serif"
    fontSize: "0.6875rem"
    fontWeight: 720
    lineHeight: 1.3
    letterSpacing: "0.08em"
    fontVariation: "'wdth' 68"
  code-cell:
    fontFamily: "Archivo Variable, Archivo, system-ui, sans-serif"
    fontSize: "1.5rem"
    fontWeight: 640
    fontVariation: "'wdth' 75"
  axis:
    fontFamily: "Archivo Variable, Archivo, system-ui, sans-serif"
    fontSize: "0.625rem"
    fontWeight: 560
    letterSpacing: "0.04em"
    fontVariation: "'wdth' 72"
  code:
    fontFamily: "ui-monospace, SF Mono, Menlo, Cascadia Mono, Consolas, DejaVu Sans Mono, Liberation Mono, monospace"
    fontSize: "13.5px"
    lineHeight: 1.6
  code-small:
    fontFamily: "ui-monospace, SF Mono, Menlo, Cascadia Mono, Consolas, DejaVu Sans Mono, Liberation Mono, monospace"
    fontSize: "12px"
    lineHeight: 1.6
rounded:
  none: "0"
  hairline: "2px"
spacing:
  "1": "0.25rem"
  "2": "0.5rem"
  "3": "0.75rem"
  "4": "1rem"
  "5": "1.5rem"
  "6": "2rem"
  "7": "3rem"
  "8": "4rem"
components:
  button:
    backgroundColor: "{colors.white}"
    textColor: "{colors.blue-text}"
    typography: "{typography.body-small}"
    rounded: "{rounded.hairline}"
    padding: "0 1rem"
    height: "2.25rem"
  button-hover:
    backgroundColor: "{colors.blue-wash}"
    textColor: "{colors.blue-text}"
  button-primary:
    backgroundColor: "{colors.blue-text}"
    textColor: "{colors.white}"
    typography: "{typography.body-small}"
    rounded: "{rounded.hairline}"
    padding: "0 1rem"
    height: "2.25rem"
  button-primary-hover:
    backgroundColor: "{colors.blue-deep}"
    textColor: "{colors.white}"
  button-danger:
    backgroundColor: "{colors.white}"
    textColor: "{colors.red-text}"
    rounded: "{rounded.hairline}"
    padding: "0 1rem"
    height: "2.25rem"
  button-danger-hover:
    backgroundColor: "{colors.red-wash}"
    textColor: "{colors.red-text}"
  button-disabled:
    backgroundColor: "{colors.paper}"
    textColor: "{colors.blue-faint}"
  input:
    backgroundColor: "{colors.white}"
    textColor: "{colors.ink}"
    typography: "{typography.body}"
    rounded: "{rounded.hairline}"
    padding: "0 0.75rem"
    height: "2.25rem"
  field-label:
    textColor: "{colors.blue-text}"
    typography: "{typography.annotation}"
  stamp:
    textColor: "{colors.red-text}"
    typography: "{typography.stamp}"
    rounded: "{rounded.none}"
    padding: "0.125rem 0.4375rem"
  sheet:
    backgroundColor: "{colors.white}"
    rounded: "{rounded.none}"
    padding: "1.5rem"
  proof-plate:
    backgroundColor: "{colors.white}"
    rounded: "{rounded.none}"
  code-cell:
    backgroundColor: "{colors.white}"
    textColor: "{colors.ink}"
    typography: "{typography.code-cell}"
    rounded: "{rounded.hairline}"
---

# Design System: Afterimage

## Overview

**Creative North Star: "The Proofroom"**

Every screen a display shows is a proof before it goes to ink, and the web UI is the room where those proofs are checked. Two voices share the room and never mix. Everything a display will show is true ink on white, at its real pixels. Everything Afterimage itself says (navigation, labels, measurements, annotations) is drawn in non-repro blue, the pencil colour that a print camera does not see. A third voice, proofer's red, appears only to stamp a state or to mark an action that changes a device.

The room is calm and precise. Pages are warm proofing paper with hairline blue rules, generous whitespace and no shadows. The displays' images carry the visual weight; the chrome around them stays light, narrow and blue, so the eye lands on what a household will actually see. The UI is an Operate surface: patterns a visitor already knows (a top bar, tabs, forms, a code editor) keep their usual places, and the world shows in the details: crop marks, step wedges, stamps and a day strip.

PRODUCT.md rules out mascots, playful copy and anything that reads as a toy; the proofroom is a tool. It rejects the category's default of a dark sidebar with a grid of equal cards.

**Key Characteristics:**

- Warm paper ground, white sheets for anything a person fills in or a display shows.
- Ink only for display content and primary text; blue for everything Afterimage says.
- Red reserved for stamps and destructive actions.
- One variable grotesk (Archivo), with its narrow cut for annotations, stamps and codes.
- Display images at 1:1 wherever the viewport can hold them, framed with crop marks and the palette's step wedge.
- Flat: no shadows, no rounded cards.

## Colors

A warm neutral paper, one ink, one family of non-repro blues, and one proofer's red.

### Primary

- **Non-repro Blue** (#2f6fd6): marks and lines: crop marks, the active tab's underline, focus rings, the day strip's "now" marker, the layout grid over a proof, the code field's active cell.
- **Pencil Blue** (#2659b8): blue as text and as a solid fill: navigation, labels, annotations, links, secondary buttons and the primary button's fill. It holds 6.1:1 on paper and 6.6:1 on white.
- **Deep Pencil Blue** (#1f4d9f): the hover state of blue text and of the primary button.
- **Blue Wash** (#e6eefb): hover fill of quiet controls, the active editor line, text selection on the page.
- **Hairline Blue** (#c7d8f5): every rule, input border, the step wedge's outline, and the editor's selection.
- **Edge Blue** (#9bb8eb): the border of an input or a starter on hover, and of a filled code cell.
- **Blue Gray** (#5f6f8f): quiet text that is not ink: placeholders, editor comments and punctuation, line numbers. It holds 4.7:1 on paper and 5.1:1 on white.
- **Faint Blue** (#8aa6d9): the text of a disabled control, and nothing else.

### Secondary

- **Proofer's Red** (#e2483d): a stamp's double rule, the editor's error underline.
- **Red Pencil** (#c2372d): red as text: stamp lettering, destructive buttons, error messages (5.0:1 on paper).
- **Red Wash** (#fbe9e7): hover fill of a destructive button.
- **Deep Red Pencil** (#a82d24): hover fill of the solid destructive button that confirms a removal.

### Neutral

- **Proofing Paper** (#f7f6f1): the page ground and the top bar.
- **Sheet White** (#ffffff): plates, the claim sheet, the sign-in sheet, inputs, the editor.
- **True Ink** (#1b1b19): what displays show, primary text, values in the sources tree.
- **Soft Ink** (#4b4a45): secondary text: hints, asides such as "3.71 V", numbers and booleans in the sources tree.

### Named Rules

**The Two Voices Rule.** Ink belongs to what a display shows and to the primary text a person reads; blue belongs to what Afterimage says about it. A label, a measurement or a navigation item is never ink; a screen's content is never blue.

**The Red Is Earned Rule.** Red appears only on a stamp or on an action that changes a device or deletes a screen. No red for emphasis, decoration or validation hints.

## Typography

**Display Font:** none; Afterimage has no display voice beyond its wordmark.
**Body Font:** Archivo Variable (self-hosted through `@fontsource-variable/archivo`, with its width axis), falling back to system-ui.
**Label/Mono Font:** Archivo's narrow cut (width 68 to 75%) for annotations, stamps and codes; the platform monospace stack for YAML only.

**Character:** one sturdy grotesk at three widths: wide for the wordmark, normal for reading, narrow for the measurements and stamps a proofer writes in the margin. Every figure is tabular so readings line up.

### Hierarchy

- **Wordmark** (640, 1.25rem, width 118%): "Afterimage" in the top bar and on the sign-in sheet.
- **Headline** (620, 1.5rem, 1.15): a page's name, such as "Devices". A device's own page sets its name at 2rem.
- **Title** (620, 1.25rem, 1.15): a device's name in the list, section headings such as "Playlist".
- **Body** (400, 0.9375rem, 1.45): reading text and form values.
- **Body small** (400, 0.8125rem): facts, hints, buttons, error text.
- **Annotation** (560, 0.6875rem, width 72%, 0.06em, uppercase): field labels, fact labels, a proof's caption, "Times in Europe/Berlin".
- **Stamp** (720, 0.6875rem, width 68%, 0.08em, uppercase): device states only.
- **Code cell** (640, 1.5rem, width 75%): the six characters of a claim code.
- **Axis** (560, 0.625rem, width 72%, 0.04em): the hour figures under a day strip.
- **Code** (13.5px, 1.6): the YAML editor.
- **Code small** (12px, 1.6): the sources tree and the secrets' names beside it.

### Named Rules

**The Narrow Margin Rule.** Anything written about a thing (its label, size, scale, time zone) uses the narrow annotation style in blue; anything that is the thing uses the normal width.

## Layout

The page sits inside a gutter of `clamp(1rem, 3vw, 2.5rem)` under a sticky top bar 3.25rem tall. Spacing follows one scale from 0.25rem to 4rem; groups sit tight (0.25 to 0.75rem) and sections stand apart (1.5 to 4rem), each section opened by a hairline rule rather than a box.

The devices list is a proof sheet, not a card grid. Each display is a row: its plate in a shared track `800px + 1.25rem` wide, so an 800-pixel display shows at 1:1 on a 1440-pixel screen, its facts in a single column of at least 13rem beside it, and its day strip under the plate. Every row starts its facts and its strip's track at one edge. The claim sheet takes a 17.5rem column at the right, level with the first plate, and stays in view while the list scrolls.

Below 72rem a row stacks: plate, facts in two columns, then the strip. Below 52rem the claim sheet moves above the list. Below 40rem crop marks shrink to fit the narrower gutter.

The screen editor fills the viewport: the YAML on the left, the proof on the right at the device's real pixels, and the data sources' answers below the proof in a column that scrolls on its own.

## Elevation & Depth

The proofroom is flat. Nothing casts a shadow. Depth comes from material: white sheets and plates lie on warm paper, and a plate carries a 1px outline of ink at 8% so a white screen still reads as an object. The sticky top bar is paper over paper, separated by its hairline rule.

**The Flat Table Rule.** Proofs lie on the table; nothing floats above it. Use a white sheet, a rule or a wash, never a shadow, to separate one thing from another.

## Shapes

Controls have hairline corners (2px): buttons, inputs, selects, code cells. Sheets, plates and stamps are square. The signature forms are drawn geometry: crop marks (two 1px blue strokes at a plate's top-left and bottom-right corners, 0.875rem long and stopping 0.375rem short of the trim), the stamp's double rule, the step wedge's row of patches, and the crossed box of a plate that has no image yet.

## Components

### Buttons

Quiet and exact: a hairline frame that names its action.

- **Shape:** hairline corners (2px), 2.25rem tall, 1rem side padding, a 16px lucide icon before the label when it helps.
- **Default:** white with Pencil Blue text and a Pencil Blue border.
- **Primary:** Pencil Blue fill with white text; one per form, for the action that saves.
- **Danger:** white with Red Pencil text and border; the solid red variant confirms a removal after its consequences are spelled out.
- **Hover / Focus:** hover fills with Blue Wash (Red Wash for danger; Deep Pencil Blue for primary) over 150ms; focus draws a 2px Non-repro Blue ring 2px outside.
- **Disabled:** paper fill, Hairline Blue border, light blue text.
- **Icon buttons:** 2rem square, transparent until hovered.

### Inputs / Fields

- **Style:** white, 1px Hairline Blue border, hairline corners, 2.25rem tall; the label above in the annotation style.
- **Hover:** the border turns Edge Blue.
- **Focus:** a 2px Non-repro Blue ring inset over the border.
- **Error:** the border turns Proofer's Red and a Red Pencil message follows the field.

### Navigation

The top bar is paper with a hairline rule beneath it: the wordmark, then Devices and Screens as text tabs in Pencil Blue, the account name in Soft Ink and "Sign out" at the right. The current tab is heavier (640) and underlined by a 2px Non-repro Blue bar on the rule. On a phone the account name hides; the tabs and "Sign out" stay. Back links on inner pages lead with a lucide arrow.

### Proof plate (signature)

A display's image framed as a press proof. The image sits on white at its true pixels, rendered pixelated when scaled up; crop marks stand at two corners. Under it the caption carries the palette's step wedge (one patch per level, up to 16, or a continuous band for full colour), the palette's name, the size, and the scale: "1:1" or an honest "at 42%". A plate with no image keeps its display's footprint, crossed corner to corner with "No image yet", and drops the scale.

### Stamp (signature)

A device state in proofer's red: narrow capitals between a 1.5px rule and a 1px outline 1.5px outside it. States are named plainly: "Not checked in", "Not seen lately", "Nothing scheduled". A state that ends while the page is open is cancelled rather than removed: its rules fade, a Red Pencil stroke draws through it over 320ms, and its lettering stays legible.

### Day strip (signature)

One day from midnight to midnight, each playlist entry a lane whose ink bars are as long as the time the entry is current. Lane names stand in a 6.5rem column of narrow blue text before the track, hour ticks fall every 3 hours (stronger every 6), and a Non-repro Blue marker shows now.

### Sheets

The claim panel, the sign-in form and the first-display guide are white sheets with crop marks on paper, padded 1.5 to 2rem. The claim sheet holds the six-cell code field: white cells in the narrow cut, the active one ringed in Non-repro Blue with a blinking caret.

### Editor

CodeMirror on white: keys in Pencil Blue at 560, strings in ink, numbers and booleans in Soft Ink, comments and punctuation in Blue Gray, line numbers in Blue Gray on paper. Long lines wrap under their own indentation. Problems carry a wavy Proofer's Red underline. The layout grid over the proof draws Non-repro Blue lines every 40 device pixels, stronger every 200, with dashed centre lines.

## Do's and Don'ts

### Do:

- **Do** show a display's image at 1:1 wherever the viewport holds it, and label the scale honestly where it does not.
- **Do** write labels, sizes, times and other annotations in the narrow blue style (0.6875rem, 560, width 72%, uppercase).
- **Do** separate sections with a 1px Hairline Blue (#c7d8f5) rule and whitespace.
- **Do** keep text at 4.5:1 or better: Blue Gray (#5f6f8f) is the lightest text colour on paper.
- **Do** spell out what a destructive action does to the device before it happens, and put that action in red.
- **Do** use lucide icons at one stroke weight; never a Unicode glyph standing in for one.

### Don't:

- **Don't** add shadows, rounded cards or a card grid of equal tiles.
- **Don't** use red for anything but stamps and destructive actions (The Red Is Earned Rule).
- **Don't** set Afterimage's own labels in ink or a display's content in blue (The Two Voices Rule).
- **Don't** introduce a second paper or a gray surface; a thing is either paper or a white sheet.
- **Don't** add mascots, illustrations or playful copy.
