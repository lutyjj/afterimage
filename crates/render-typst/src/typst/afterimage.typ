// Draws a resolved Afterimage screen: the model's ScreenDocument after the server
// has replaced every template with its value and expanded every `each`. Sizes,
// fonts and fills come from the design tokens of the display's profile, so one
// pt is one layout pixel of TRMNL's design system.

// A resolved value as text: nothing for a value a template could not produce.
#let shown(value) = if value == none { "" } else if type(value) in (str, int, float, bool) { str(value) } else { repr(value) }

// A token's value; lengths are pixels.
#let tok(t, name, default: none) = t.at(name, default: default)
#let px(t, name, default: 0) = tok(t, name, default: default) * 1pt

// A token for one size of a component, such as `--label-small-font-size`, or the
// component's base token when that size has none.
#let sized(t, component, size, suffix) = {
  let infix = if size == "base" { "" } else { "-" + size }
  tok(t, component + infix + suffix, default: tok(t, component + suffix))
}

// TRMNL's stylesheet names Inter "Inter Variable"; the font file calls it Inter.
#let family(name) = if name == none { "Inter" } else if name.starts-with("Inter") { "Inter" } else { name }

#let typeface(t, component, size) = (
  font: (family(sized(t, component, size, "-font-family")), "Inter"),
  size: sized(t, component, size, "-font-size") * 1pt,
  weight: int(sized(t, component, size, "-font-weight") / 100) * 100,
)

// A fill: a token's color, with its dither tile on displays of few grays.
#let fill(t, color, image: none) = {
  let base = rgb(tok(t, color, default: "#FFFFFF"))
  let svg = if image == none { none } else { tok(t, image) }
  if svg == none { return base }
  let size = px(t, "--dither-bg-size", default: 16)
  tiling(size: (size, size), {
    place(rect(width: size, height: size, fill: base, stroke: none))
    place(std.image(bytes(svg), format: "svg", width: size, height: size))
  })
}

// Gray ink for text: the gray itself where the palette has a gray, or black,
// since neither a mid gray nor a dither survives as small text without one.
#let gray-ink(t, grays) = if grays { rgb(tok(t, "--gray-50", default: "#999999")) } else { black }

// Text kept legible over a dithered fill: a white outline under the ink, as
// TRMNL's title bar draws it (`--title-bar-text-stroke-width`).
#let haloed(t, body) = {
  let width = px(t, "--title-bar-text-stroke-width", default: 3.5)
  box({
    place(text(stroke: width + white, body))
    body
  })
}

#let gap(t, name) = if name == "none" { 0pt } else if name == "medium" {
  px(t, "--gap", default: 10)
} else {
  px(t, "--gap-" + name, default: 10)
}

// Text in `face`, cut to `lines` lines with an ellipsis, as TRMNL's clamp: the
// longest start of the text that fits, found by halving.
#let clamped(face, leading, words, lines) = {
  let set-text(body) = {
    set par(leading: (leading - 1) * face.size)
    text(..face, body)
  }
  if lines == none or words == "" { return set-text(words) }
  layout(region => {
    let height(body) = measure(block(width: region.width, set-text(body))).height
    // A line as tall as any line of this text: the tallest of a Latin letter and
    // the text's own non-ASCII characters, which may come from a fallback font
    // whose lines are taller than the face's.
    let samples = ("X",) + words.clusters().filter(cluster => cluster.len() > 1).dedup()
    let line = samples.fold("X", (tallest, sample) => if height(sample) > height(tallest) { sample } else { tallest })
    let limit = height(range(int(lines)).map(_ => [#line]).join(linebreak())) + 0.5pt
    if height(words) <= limit { return set-text(words) }
    let clusters = words.clusters()
    let (low, high) = (0, clusters.len())
    while low < high {
      let middle = calc.ceil((low + high) / 2)
      if height(clusters.slice(0, middle).join() + "…") <= limit { low = middle } else { high = middle - 1 }
    }
    let kept = if low == 0 { "" } else { clusters.slice(0, low).join().trim() }
    set-text(kept + "…")
  })
}

// A value as a number: numbers, and text that reads as one (Home Assistant
// states are text); `none` for anything else, which charts leave out.
#let number(value) = if type(value) in (int, float) { float(value) } else if type(value) == str and value.trim().match(regex("^-?[0-9]+(\\.[0-9]+)?([eE][-+]?[0-9]+)?$")) != none { float(value.trim()) } else { none }

// Whether a component takes a share of the space left along its parent.
#let grows(node) = {
  let kind = node.keys().first()
  let value = node.at(kind)
  if type(value) != dictionary { return false }
  value.at("grow", default: kind in ("image", "lineChart", "barChart"))
}

#let text-component(t, component, value) = {
  let size = value.at("size", default: "base")
  let face = typeface(t, component, size)
  let leading = sized(t, component, size, "-line-height")
  let leading = if leading == none or leading < 0.8 { 1.2 } else { leading }
  clamped(face, leading, shown(value.text), value.at("clamp", default: none))
}

#let label(t, value, grays) = {
  let size = value.at("size", default: "base")
  let face = typeface(t, "--label", size)
  let leading = sized(t, "--label", size, "-line-height")
  let leading = if leading == none or leading < 0.8 { 1.2 } else { leading }
  let body = clamped(face, leading, shown(value.text), value.at("clamp", default: none))
  let inset = (x: px(t, "--gap-xsmall", default: 5), y: 1pt)
  let radius = px(t, "--rounded-xsmall", default: 5)
  let style = value.at("style", default: "plain")
  if style == "outline" {
    box(stroke: 1pt + black, inset: inset, radius: radius, body)
  } else if style == "underline" {
    underline(offset: 2pt, body)
  } else if style == "gray" {
    text(fill: gray-ink(t, grays), body)
  } else if style == "filled" {
    box(fill: black, inset: inset, radius: radius, text(fill: white, body))
  } else {
    body
  }
}

#let value(t, value) = {
  let size = value.at("size", default: "base")
  let base = sized(t, "--value", size, "-font-size") * 1pt
  let face(size) = (
    font: (family(tok(t, "--value-font-family")), "Inter"),
    size: size,
    weight: int(sized(t, "--value", value.at("size", default: "base"), "-font-weight") / 100) * 100,
    number-width: if value.at("tabular", default: false) { "tabular" } else { "proportional" },
  )
  let body(size) = text(..face(size), shown(value.text))
  if not value.at("fit", default: false) { return body(base) }
  layout(region => {
    let width = measure(body(base)).width
    if width > region.width and width > 0pt { body(base * (region.width / width)) } else { body(base) }
  })
}

// Nice steps for an axis: 1, 2 or 5 times a power of ten.
#let step-for(span, ticks) = {
  let raw = span / ticks
  let magnitude = calc.pow(10.0, calc.floor(calc.log(calc.max(raw, 1e-9), base: 10)))
  for factor in (1, 2, 5, 10) {
    if factor * magnitude >= raw { return factor * magnitude }
  }
  10 * magnitude
}

// A chart fills what its parent fixes; on an axis sized to content it takes at
// most 320 pixels of width, and half its width but at most 200 pixels of height.
#let chart(t, value, kind, bounds) = layout(region => {
  let series = value.series
    .filter(s => type(s.values) == array)
    .map(s => (..s, values: s.values.map(number)))
    .filter(s => s.values.any(v => v != none))
  if series.len() == 0 { return text(..typeface(t, "--label", "small"), "No data") }
  let all = series.map(s => s.values).flatten().filter(v => v != none)
  let low = value.at("min", default: none)
  let high = value.at("max", default: none)
  let low = if low == none { calc.min(..all) } else { low }
  let high = if high == none { calc.max(..all) } else { high }
  let low = if kind == "bar" { calc.min(low, 0) } else { low }
  let (low, high) = if high == low { (low - 1, high + 1) } else { (low, high) }
  let step = step-for(high - low, 4)
  let low = calc.floor(low / step) * step
  let high = calc.ceil(high / step) * step
  let small = typeface(t, "--label", "small")
  let labels = value.at("labels", default: none)
  let axis-width = 36pt
  let label-height = if labels == none { 0pt } else { small.size + 6pt }
  let available = if region.height == float.inf * 1pt { 200pt } else { region.height }
  let outer-width = if bounds.w { region.width } else { calc.min(region.width, 320pt) }
  let outer-height = if bounds.h { available } else { calc.min(available, outer-width / 2, 200pt) }
  let width = outer-width - axis-width
  let height = outer-height - label-height - 4pt
  let count = calc.max(..series.map(s => s.values.len()))
  let x(i) = if kind == "bar" {
    axis-width + width * (i + 0.5) / count
  } else {
    axis-width + width * i / calc.max(count - 1, 1)
  }
  let y(v) = 2pt + height * (1 - (v - low) / (high - low))
  box(width: outer-width, height: height + label-height + 4pt, {
    let tick = low
    while tick <= high + step / 2 {
      let dash = if tick == low { none } else { "dotted" }
      place(dx: axis-width, dy: y(tick), line(length: width, stroke: (thickness: 1pt, dash: dash)))
      let shown = if calc.abs(tick - calc.round(tick)) < 1e-9 { str(int(calc.round(tick))) } else { str(calc.round(tick, digits: 2)) }
      place(dy: calc.round((y(tick) - small.size / 2) / 1pt) * 1pt, box(width: axis-width - 8pt, align(right, text(..small, shown))))
      tick += step
    }
    if labels != none and labels.len() > 0 {
      let every = calc.max(1, calc.ceil(labels.len() * 48pt / width))
      for (i, label) in labels.enumerate() {
        if calc.rem(i, every) == 0 and i < count {
          // One line, clipped; whole pixels, so that pixel fonts land on the
          // device's grid.
          let slot = calc.max(every * width / calc.max(count, 1), 24pt)
          place(dx: calc.round((x(i) - slot / 2) / 1pt) * 1pt, dy: calc.round((height + 8pt) / 1pt) * 1pt,
            box(width: slot, height: small.size * 1.3, clip: true, align(center, text(..small, shown(label)))))
        }
      }
    }
    let dashes = (none, "dashed", "dotted", "dash-dotted")
    for (n, s) in series.enumerate() {
      let values = s.values
      if kind == "bar" {
        let slot = width / count / series.len()
        for (i, v) in values.enumerate() {
          if v == none { continue }
          let top = y(calc.max(v, 0))
          let bottom = y(calc.min(v, 0))
          let shade = if n == 0 { black } else { fill(t, "--framework-slot-item-meta-emphasis-2-bg-color", image: "--framework-slot-item-meta-emphasis-2-bg-image") }
          place(dx: axis-width + width * i / count + slot * n + slot * 0.15, dy: top,
            rect(width: slot * 0.7, height: calc.max(bottom - top, 1pt), fill: shade, stroke: none))
        }
      } else {
        // A missing value breaks the line; each run of values is its own curve.
        let runs = ((),)
        for (i, v) in values.enumerate() {
          if v == none { if runs.at(-1) != () { runs.push(()) } } else { runs.at(-1).push((x(i), y(v))) }
        }
        let stroke = (thickness: if n == 0 { 4pt } else { 3pt }, cap: "round", join: "round", dash: dashes.at(calc.rem(n, 4)))
        for points in runs.filter(run => run.len() > 0) {
          if points.len() == 1 {
            place(dx: points.at(0).at(0) - 2pt, dy: points.at(0).at(1) - 2pt, circle(radius: 2pt, fill: black, stroke: none))
            continue
          }
          let segments = (curve.move(points.at(0)),)
          for i in range(points.len() - 1) {
            let (p0, p1, p2, p3) = (points.at(calc.max(i - 1, 0)), points.at(i), points.at(i + 1), points.at(calc.min(i + 2, points.len() - 1)))
            let c1 = (p1.at(0) + (p2.at(0) - p0.at(0)) / 6, p1.at(1) + (p2.at(1) - p0.at(1)) / 6)
            let c2 = (p2.at(0) - (p3.at(0) - p1.at(0)) / 6, p2.at(1) - (p3.at(1) - p1.at(1)) / 6)
            segments.push(curve.cubic(c1, c2, p2))
          }
          place(curve(stroke: stroke, ..segments))
        }
      }
    }
  })
})

// Each component knows whether its parent fixes its width and its height (`w`
// and `h` of `bounds`): the page fixes both, and a stack fixes the length of a
// child that grows along it and, when its own is fixed, the breadth of every
// child. An axis that is not fixed follows the content, so a component fills
// only what is fixed: a stack that fills an axis its parent sizes to content
// would take the whole page.

// A column or row: children size to their content, and those that grow share
// what is left when the stack's length is fixed. Without growing children,
// `justify` places the free space. A divider is an empty track of no size with
// a line along it, as long as the stack is broad: the gaps fall on both of its
// sides, and it never sizes the stack itself.
#let stack-of(t, v, direction, bounds, draw) = {
  let divider(child) = child.keys().first() == "divider"
  let children = v.children
  let (length, breadth) = if direction == "column" { (bounds.h, bounds.w) } else { (bounds.w, bounds.h) }
  let tracks = children.map(child => if length and not divider(child) and grows(child) { 1fr } else { auto })
  let spacing = gap(t, v.at("gap", default: "medium"))
  let justify = v.at("justify", default: "start")
  // Absent for an item's children, which keep the alignment around the item.
  let cross = v.at("align", default: none)
  let free = length and not tracks.contains(1fr)
  let gutter = if free and justify == "between" and children.len() > 1 { 1fr } else { spacing }
  let cells = children.zip(tracks).map(((child, track)) => {
    if divider(child) { return [] }
    let shared = track == 1fr
    draw(child, if direction == "column" { (w: breadth, h: shared) } else { (w: shared, h: breadth) })
  })
  let dividers = children.enumerate().filter(((_, child)) => divider(child)).map(((at, _)) => at)
  let across = if breadth { 100% } else { auto }
  let (lead, tracks, cells) = if free and justify in ("end", "center") {
    let edge = if justify == "center" { ((1fr,), ([],)) } else { ((), ()) }
    (1, (1fr,) + tracks + edge.at(0), ([],) + cells + edge.at(1))
  } else if free and justify == "start" {
    (0, tracks + (1fr,), cells + ([],))
  } else {
    (0, tracks, cells)
  }
  if direction == "column" {
    let horizontal = if cross == "center" { center } else if cross == "end" { right } else { left }
    let aligned = if cross == none { (:) } else { (align: horizontal) }
    let rules = dividers.map(at => grid.hline(y: at + lead, stroke: 1pt + black))
    grid(columns: (across,), rows: tracks, row-gutter: gutter, ..aligned, ..cells, ..rules)
  } else {
    let vertical = if cross == "center" { horizon } else if cross == "end" { bottom } else { top }
    let aligned = if cross == none { (:) } else { (align: vertical) }
    let rules = dividers.map(at => grid.vline(x: at + lead, stroke: 1pt + black))
    grid(rows: (across,), columns: tracks, column-gutter: gutter, ..aligned, ..cells, ..rules)
  }
}

#let item(t, v, bounds, draw) = {
  let emphasis = int(v.at("emphasis", default: 1))
  let slot = if emphasis == 1 { "--framework-slot-item-meta" } else { "--framework-slot-item-meta-emphasis-" + str(emphasis) }
  let meta = fill(t, slot + "-bg-color", image: slot + "-bg-image")
  // Its children are a column, so a divider among them is one as a column draws it.
  let content = stack-of(t, (children: v.children, gap: "small"), "column", (w: bounds.w, h: false), draw)
  if v.at("plain", default: false) { return content }
  let index = v.at("index", default: none)
  let face = typeface(t, "--item-index", "base")
  grid(
    columns: (if index == none { px(t, "--item-meta-width", default: 10) } else { auto }, if bounds.w { 1fr } else { auto }),
    column-gutter: px(t, "--gap-small", default: 7),
    grid.cell(fill: meta, inset: if index == none { 0pt } else { (x: 4pt, y: 2pt) },
      if index == none { [] } else { align(center + horizon, text(..face, shown(index))) }),
    content,
  )
}

// The bytes of the image a resolved `url` names, or `none`.
#let image-of(images, key) = if type(key) == str { images.at(key, default: none) } else { none }

#let node(t, images, grays, n, direction: "column", bounds: (w: true, h: true)) = {
  let kind = n.keys().first()
  let v = n.at(kind)
  if kind == "column" or kind == "row" {
    stack-of(t, v, kind, bounds, (child, bounds) => node(t, images, grays, child, direction: kind, bounds: bounds))
  } else if kind == "grid" {
    // Equal columns across a fixed width; rows as tall as their cells.
    let columns = int(v.columns)
    let cells = (w: bounds.w, h: false)
    grid(
      columns: (if bounds.w { 1fr } else { auto },) * columns,
      gutter: gap(t, v.at("gap", default: "medium")),
      ..v.cells.map(cell => grid.cell(colspan: calc.min(int(cell.at("span", default: 1)), columns), node(t, images, grays, cell.content, bounds: cells)))
    )
  } else if kind == "item" {
    item(t, v, bounds, (child, bounds) => node(t, images, grays, child, bounds: bounds))
  } else if kind == "title" {
    text-component(t, "--title", v)
  } else if kind == "description" {
    text-component(t, "--description", v)
  } else if kind == "label" {
    label(t, v, grays)
  } else if kind == "value" {
    value(t, v)
  } else if kind == "image" {
    let source = image-of(images, v.at("url", default: none))
    if source == none { text(..typeface(t, "--label", "small"), "Image unavailable") } else {
      // Fills what its parent fixes; along an axis sized to content it keeps its
      // proportions.
      std.image(source, width: if bounds.w { 100% } else { auto }, height: if bounds.h { 100% } else { auto }, fit: v.at("fit", default: "contain"))
    }
  } else if kind == "divider" {
    // Outside a stack, such as alone in a grid cell: as long as its parent fixes,
    // and nothing along an axis sized to content.
    let fixed = if direction == "row" { bounds.h } else { bounds.w }
    if not fixed { [] } else if direction == "row" { line(angle: 90deg, length: 100%) } else { line(length: 100%) }
  } else if kind == "lineChart" {
    chart(t, v, "line", bounds)
  } else if kind == "barChart" {
    chart(t, v, "bar", bounds)
  } else {
    text(..typeface(t, "--label", "small"), "Unknown component " + kind)
  }
}

#let title-bar(t, images, bar) = {
  let face = typeface(t, "--title-bar", "base")
  let ink = rgb(tok(t, "--framework-slot-title-bar-text-color", default: "#000000"))
  let icon = bar.at("icon", default: none)
  let icon = if image-of(images, icon) == none { none } else {
    std.image(image-of(images, icon), height: px(t, "--title-bar-image-height", default: 24))
  }
  block(
    width: 100%,
    height: px(t, "--title-bar-height", default: 40),
    fill: fill(t, "--framework-slot-title-bar-bg-color", image: "--framework-slot-title-bar-bg-image"),
    radius: px(t, "--title-bar-border-radius", default: 10),
    inset: (x: px(t, "--gap", default: 10)),
    grid(
      columns: (auto, auto, 1fr, auto),
      column-gutter: px(t, "--gap-small", default: 7),
      align: horizon,
      rows: (100%,),
      if icon == none { [] } else { icon },
      haloed(t, text(..face, fill: ink, shown(bar.title))),
      [],
      haloed(t, text(..face, weight: "regular", fill: ink, shown(bar.at("instance", default: none)))),
    ),
  )
}

// The whole screen, `width` by `height` layout pixels; `grays` tells whether the
// palette has a gray for gray text.
#let screen(document, t, images, grays, width, height) = {
  set page(width: width * 1pt, height: height * 1pt, margin: px(t, "--gap", default: 10), fill: white)
  set text(font: ("Inter",), size: 16pt, fill: black)
  let bar = document.at("titleBar", default: none)
  let content = node(t, images, grays, document.content)
  if bar == none {
    block(width: 100%, height: 100%, content)
  } else {
    grid(rows: (1fr, auto), row-gutter: px(t, "--gap", default: 10), block(width: 100%, height: 100%, content), title-bar(t, images, bar))
  }
}
