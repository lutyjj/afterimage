// Draws a resolved Afterimage screen: the model's ScreenDocument after the server
// has replaced every template with its value and expanded every `each`. Sizes,
// fonts and fills come from the design tokens of the display's profile, so one
// pt is one layout pixel of TRMNL's design system.

// A resolved value as text: nothing for a value a template could not produce.
// `str` takes no boolean; `repr` writes one as Jinja does, `true` or `false`.
#let shown(value) = if value == none { "" } else if type(value) in (str, int, float) { str(value) } else { repr(value) }

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

// A line height that is a ratio of its font size; a value the tokens lack falls
// back to 1.2.
#let leading-of(t, component, size) = {
  let ratio = sized(t, component, size, "-line-height")
  if ratio == none or ratio <= 0 { 1.2 } else { ratio }
}

// CSS's line box as the text's edges: `ratio` times the size tall, with the
// font's ascent and descent (`@metrics`, read from the font files) centred in
// it, so lines stack and sit on their baselines where a browser puts them.
#let line-box(t, name, size, ratio) = {
  let metrics = t.at("@metrics")
  let (ascent, descent) = metrics.at(name, default: metrics.at("Inter"))
  let half = (ratio - ascent - descent) / 2
  (top-edge: (ascent + half) * size, bottom-edge: -(descent + half) * size)
}

#let typeface(t, component, size) = {
  let name = family(sized(t, component, size, "-font-family"))
  let em = sized(t, component, size, "-font-size") * 1pt
  (
    font: (name, "Inter"),
    size: em,
    weight: int(sized(t, component, size, "-font-weight") / 100) * 100,
    ..line-box(t, name, em, leading-of(t, component, size)),
  )
}

// What the stylesheet paints for one part of a component (`tools/tokens` reads
// it from the release's own markup): its fill as a color, a dither tile or a
// row of pixels, its ink and its border. Inside an inverse element, its inverse.
#let paint(t, name) = {
  let scheme = if t.at("@inverse", default: false) { "paint/inverse/" } else { "paint/" }
  let found = tok(t, scheme + name)
  if found == none { (:) } else { found }
}

// A paint's fill: its color under its dither tile, or its rule: a row of the
// display's own pixels repeating over its CSS period, which runs down instead of
// across where `down` is set. Pixels the row leaves transparent show what is
// beneath.
#let fill-of(t, p, down: false) = {
  let base = if "color" in p { rgb(p.color) } else { none }
  if "tile" in p {
    let size = px(t, "--dither-bg-size", default: 16)
    return tiling(size: (size, size), {
      if base != none { place(rect(width: size, height: size, fill: base, stroke: none)) }
      place(std.image(bytes(p.tile), format: "svg", width: size, height: size))
    })
  }
  if "pattern" in p {
    let row = p.pattern.pixels
    let pixel-size = p.pattern.period * 1pt / row.len()
    let (across, along) = (p.pattern.period * 1pt, p.pattern.thickness * 1pt)
    return tiling(size: if down { (along, across) } else { (across, along) }, {
      for (at, pixel) in row.enumerate() {
        if pixel != none {
          let (dx, dy) = if down { (0pt, at * pixel-size) } else { (at * pixel-size, 0pt) }
          let (width, height) = if down { (along, pixel-size) } else { (pixel-size, along) }
          place(dx: dx, dy: dy, rect(width: width, height: height, fill: rgb(pixel), stroke: none))
        }
      }
    })
  }
  base
}

// The ink text and lines take: black, or white inside an inverse element.
#let ink(t) = rgb(paint(t, "canvas").at("ink", default: "#000000"))

// How thick a rule the stylesheet draws: one device pixel on most displays.
#let thickness-of(t, name) = paint(t, name).at("pattern", default: (thickness: 1)).thickness * 1pt

// A rule as the stylesheet draws a divider or a table's row line, across a
// column or down a row.
#let rule(t, name, length: 100%, down: false) = {
  let fill = fill-of(t, paint(t, name), down: down)
  let thick = thickness-of(t, name)
  if down { rect(width: thick, height: length, fill: fill, stroke: none) } else { rect(width: length, height: thick, fill: fill, stroke: none) }
}

// Gray ink for text: the gray the stylesheet paints gray labels with, where the
// palette has a gray, or ink, since neither a mid gray nor a dither survives as
// small text without one. Inside an inverse element, the inverse's gray.
#let gray-ink(t, grays) = if not grays { ink(t) } else {
  let gray = paint(t, "label-gray")
  rgb(gray.at("ink", default: gray.at("color", default: "#999999")))
}

// Text kept legible over a dithered fill: an outline of `color` under the ink,
// as TRMNL's title bar and index digits draw it (`--title-bar-text-stroke-width`).
#let haloed(t, body, color: white) = {
  let width = px(t, "--title-bar-text-stroke-width", default: 3.5)
  box({
    place(text(stroke: width + color, body))
    body
  })
}

#let gap(t, name) = if name == "none" { 0pt } else if name == "medium" {
  px(t, "--gap", default: 10)
} else {
  px(t, "--gap-" + name, default: 10)
}

// Text in `face`, cut to `lines` lines with an ellipsis, as TRMNL's clamp: the
// longest start of the text that fits, found by halving. Its edges are the line
// box, so lines stack with no leading of their own.
#let clamped(face, words, lines) = {
  let set-text(body) = {
    set par(leading: 0pt)
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
  clamped(face, shown(value.text), value.at("clamp", default: none))
}

#let label(t, value, grays) = {
  let size = value.at("size", default: "base")
  let face = typeface(t, "--label", size)
  let body = clamped(face, shown(value.text), value.at("clamp", default: none))
  let inset = (x: px(t, "--gap-xsmall", default: 5), y: 1pt)
  let radius = px(t, "--rounded-xsmall", default: 5)
  let style = value.at("style", default: "plain")
  if style == "outline" {
    box(stroke: 1pt + ink(t), inset: inset, radius: radius, body)
  } else if style == "underline" {
    // TRMNL's underline: a one-pixel bar along the bottom of the label's box,
    // where layout puts it, rather than a stroke under the glyphs.
    context {
      let width = measure(body).width
      stack(body, rect(width: width, height: 1pt, fill: ink(t), stroke: none))
    }
  } else if style == "gray" {
    text(fill: gray-ink(t, grays), body)
  } else if style == "filled" {
    let canvas = paint(t, "canvas").at("color", default: "#FFFFFF")
    box(fill: ink(t), inset: inset, radius: radius, text(fill: rgb(canvas), body))
  } else {
    body
  }
}

#let value(t, value) = {
  let size = value.at("size", default: "base")
  let base = sized(t, "--value", size, "-font-size") * 1pt
  let name = family(tok(t, "--value-font-family"))
  let ratio = leading-of(t, "--value", size)
  let face(size) = (
    font: (name, "Inter"),
    size: size,
    ..line-box(t, name, size, ratio),
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
      place(dx: axis-width, dy: y(tick), line(length: width, stroke: (paint: ink(t), thickness: 1pt, dash: dash)))
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
          let shade = if n == 0 { ink(t) } else { fill-of(t, paint(t, "item-meta-emphasis-2")) }
          place(dx: axis-width + width * i / count + slot * n + slot * 0.15, dy: top,
            rect(width: slot * 0.7, height: calc.max(bottom - top, 1pt), fill: shade, stroke: none))
        }
      } else {
        // A missing value breaks the line; each run of values is its own curve.
        let runs = ((),)
        for (i, v) in values.enumerate() {
          if v == none { if runs.at(-1) != () { runs.push(()) } } else { runs.at(-1).push((x(i), y(v))) }
        }
        let stroke = (paint: ink(t), thickness: if n == 0 { 4pt } else { 3pt }, cap: "round", join: "round", dash: dashes.at(calc.rem(n, 4)))
        for points in runs.filter(run => run.len() > 0) {
          if points.len() == 1 {
            place(dx: points.at(0).at(0) - 2pt, dy: points.at(0).at(1) - 2pt, circle(radius: 2pt, fill: ink(t), stroke: none))
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
  // A divider is a track one rule thick, filled with the rule: a stroke would be
  // centred on a track's edge and cover two device rows.
  let thick = thickness-of(t, "divider")
  let tracks = children.map(child => if divider(child) { thick } else if length and grows(child) { 1fr } else { auto })
  let spacing = gap(t, v.at("gap", default: "medium"))
  let justify = v.at("justify", default: "start")
  // Absent for an item's children, which keep the alignment around the item.
  let cross = v.at("align", default: none)
  let free = length and not tracks.contains(1fr)
  let gutter = if free and justify == "between" and children.len() > 1 { 1fr } else { spacing }
  let cells = children.zip(tracks).map(((child, track)) => {
    if divider(child) { return grid.cell(fill: fill-of(t, paint(t, "divider"), down: direction == "row"), []) }
    let shared = track == 1fr
    draw(child, if direction == "column" { (w: breadth, h: shared) } else { (w: shared, h: breadth) })
  })
  let across = if breadth { 100% } else { auto }
  let (tracks, cells) = if free and justify in ("end", "center") {
    let edge = if justify == "center" { ((1fr,), ([],)) } else { ((), ()) }
    ((1fr,) + tracks + edge.at(0), ([],) + cells + edge.at(1))
  } else if free and justify == "start" {
    (tracks + (1fr,), cells + ([],))
  } else {
    (tracks, cells)
  }
  if direction == "column" {
    let horizontal = if cross == "center" { center } else if cross == "end" { right } else { left }
    let aligned = if cross == none { (:) } else { (align: horizontal) }
    grid(columns: (across,), rows: tracks, row-gutter: gutter, ..aligned, ..cells)
  } else {
    let vertical = if cross == "center" { horizon } else if cross == "end" { bottom } else { top }
    let aligned = if cross == none { (:) } else { (align: vertical) }
    grid(rows: (across,), columns: tracks, column-gutter: gutter, ..aligned, ..cells)
  }
}

// A container's surface: inverse turns it to light content on the canvas's
// inverse with every paint inside inverted, as TRMNL's `.inverse` does; padding
// and rounded corners are TRMNL's spacing and rounding. `body` draws the content
// with the tokens of the surface.
#let surface(t, v, bounds, body) = {
  let inverse = v.at("inverse", default: false)
  let padding = gap(t, v.at("padding", default: "none"))
  let rounded = v.at("rounded", default: "none")
  let radius = if rounded == "none" { 0pt } else if rounded == "base" { px(t, "--rounded", default: 10) } else { px(t, "--rounded-" + rounded, default: 10) }
  if not inverse and padding == 0pt and radius == 0pt { return body(t) }
  let t = if inverse { (..t, "@inverse": true) } else { t }
  let canvas = paint(t, "canvas")
  block(
    width: if bounds.w { 100% } else { auto },
    height: if bounds.h { 100% } else { auto },
    fill: if inverse and "color" in canvas { rgb(canvas.color) } else { none },
    inset: padding,
    radius: radius,
    {
      set text(fill: ink(t))
      body(t)
    },
  )
}

#let item(t, v, bounds, draw) = {
  let emphasis = int(v.at("emphasis", default: 1))
  let meta = fill-of(t, paint(t, if emphasis == 1 { "item-meta" } else { "item-meta-emphasis-" + str(emphasis) }))
  // Its children are a column, so a divider among them is one as a column draws it.
  // TRMNL's item content sets its parts one under another with no gap.
  let content = stack-of(t, (children: v.children, gap: "none"), "column", (w: bounds.w, h: false), draw)
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

// The alignment of a `TextAlign`.
#let across(align) = if align == "center" { center } else if align == "end" { right } else { left }

// TRMNL's table: rows as tall as the size's row height or their tallest cell,
// cells centred down the row, a rule under the heading row and between body
// rows, and columns sharing a fixed width in proportion to their widest text,
// as the browser's table layout shares it. Small tables set their headings as
// small titles and their cells as small labels.
#let table-of(t, v, bounds) = {
  let size = v.at("size", default: "base")
  let small = size in ("xsmall", "small")
  let infix = if size == "base" { "" } else { "-" + size }
  let head-face = typeface(t, "--title", if small { "small" } else { "base" })
  let cell-face = typeface(t, "--label", if small { "small" } else { "base" })
  let head-height = px(t, "--table" + infix + "-thead-height", default: 36)
  let row-height = px(t, "--table" + infix + "-tbody-height", default: 46)
  let columns = v.at("columns", default: ())
  let rows = v.at("rows", default: ())
  let more = v.at("more", default: none)
  let indexed = v.at("indexed", default: false)
  let meta-width = px(t, "--item-meta-width", default: 10)
  // The index bar and the space after it, before the first column.
  let lead = if indexed { meta-width + px(t, "--gap-small", default: 7) } else { 0pt }
  layout(region => {
    let widest(index) = {
      let heading = measure(text(..head-face, shown(columns.at(index).heading))).width
      calc.max(heading, ..rows.map(row => measure(text(..cell-face, shown(row.at(index, default: none)))).width), 1pt)
    }
    let widths = range(columns.len()).map(widest)
    let tracks = if bounds.w { widths.map(width => width / 1pt * 1fr) } else { widths }
    let row-of(height, cells, rule-name, start) = block(width: if bounds.w { 100% } else { auto }, breakable: false, {
      grid(
        columns: (lead,) + tracks,
        align: horizon,
        box(height: height, width: lead, start),
        ..cells,
      )
      if rule-name != none { place(bottom + left, rule(t, rule-name)) }
    })
    let cell(face, value, column) = align(across(column.at("align", default: "start")),
      clamped(face, shown(value), column.at("clamp", default: 1)))
    let headings = columns.map(column => cell(head-face, column.heading, column))
    stack(
      row-of(head-height, headings, "table-head-rule", []),
      ..rows.enumerate().map(((at, row)) => {
        let last = at == rows.len() - 1 and more == none
        let meta = paint(t, "table-meta")
        let index = paint(t, "table-index")
        let index-ink = rgb(index.at("ink", default: "#000000"))
        let halo = rgb(index.at("halo", default: "#FFFFFF"))
        let start = if not indexed { [] } else {
          place(left + top, dy: 4pt, rect(width: meta-width, height: row-height - 8pt, radius: 3pt, fill: fill-of(t, meta), stroke: none))
          place(left + horizon, box(width: meta-width, align(center, haloed(t, color: halo, text(..typeface(t, "--item-index", "base"), fill: index-ink, str(at + 1))))))
        }
        row-of(row-height, columns.enumerate().map(((index, column)) => cell(cell-face, row.at(index, default: none), column)), if last { none } else { "table-body-rule" }, start)
      }),
      ..if more == none { () } else {
        (block(height: if small { px(t, "--table-xsmall-tbody-height", default: 22) } else { px(t, "--table-small-tbody-height", default: 31) },
          align(horizon, pad(left: lead, text(..typeface(t, "--label", "small"), shown(more))))),)
      },
    )
  })
}

// TRMNL's progress bar: the label and figure on one line, then a track with a
// one-pixel border whose fill reaches `value` of `max`. It fills what its parent
// fixes, or is 320 pixels wide.
#let progress-bar(t, v, bounds) = {
  let size = v.at("size", default: "base")
  let small = size in ("xsmall", "small")
  let infix = if size == "base" { "" } else { "-" + size }
  let height = px(t, "--progress-bar-height" + infix, default: 24)
  let radius = px(t, "--progress-bar-radius", default: 10)
  let value = number(v.at("value", default: none))
  let maximum = number(v.at("max", default: 100))
  let share = if value == none or maximum == none or maximum <= 0 { 0 } else { calc.clamp(value / maximum, 0, 1) }
  let emphasis = int(v.at("emphasis", default: 1))
  let track = paint(t, "progress-track")
  let fill = paint(t, if emphasis == 1 { "progress-fill" } else { "progress-fill-emphasis-" + str(emphasis) })
  let label = v.at("label", default: none)
  let figure = v.at("figure", default: none)
  block(width: if bounds.w { 100% } else { 320pt }, breakable: false, {
    // The parts' own boxes and gaps space them, as in the stylesheet.
    set block(spacing: 0pt)
    if label != none or figure != none {
      grid(
        columns: (1fr, auto),
        align: (left + horizon, right + horizon),
        text(..typeface(t, "--label", if small { "small" } else { "base" }), shown(label)),
        text(..typeface(t, "--value", "xxsmall"), shown(figure)),
      )
      std.v(px(t, "--gap-xsmall", default: 5), weak: false)
    }
    block(width: 100%, height: height, radius: radius, inset: 1pt, fill: rgb(track.at("border", default: "#000000")),
      block(width: 100%, height: 100%, radius: calc.max(radius - 1pt, 0pt), clip: true, fill: fill-of(t, track),
        block(width: share * 100%, height: 100%, fill: fill-of(t, fill))))
  })
}

// TRMNL's progress dots: `count` round dots with a one-pixel border, the first
// `value` filled and, where `current`, the next marked as the step under way.
#let progress-dots(t, v) = {
  let size = v.at("size", default: "base")
  let infix = if size == "base" { "" } else { "-" + size }
  let diameter = px(t, "--progress-dot-size" + infix, default: 16)
  let whole(value) = { let n = number(value); if n == none { 0 } else { int(calc.max(n, 0)) } }
  let count = calc.min(whole(v.at("count", default: 0)), 64)
  let filled = calc.min(whole(v.at("value", default: 0)), count)
  let current = v.at("current", default: false)
  stack(dir: ltr, spacing: px(t, "--gap-small", default: 7), ..range(count).map(at => {
    let name = if at < filled { "progress-dot-filled" } else if current and at == filled { "progress-dot-current" } else { "progress-dot" }
    let dot = paint(t, name)
    box(width: diameter, height: diameter, radius: 50%, inset: 1pt, fill: rgb(dot.at("border", default: "#000000")),
      box(width: 100%, height: 100%, radius: 50%, fill: fill-of(t, dot)))
  }))
}

// TRMNL's rich text: the blocks the renderer read from the Markdown, in the
// size's typeface, at most `--richtext-content-max-width` wide and as wide as its
// longest line, placed and aligned by `align`.
#let rich-text(t, v, bounds) = {
  let size = v.at("size", default: "base")
  let face = typeface(t, "--richtext", size)
  let spacing = gap(t, v.at("gap", default: "medium"))
  let horizontal = across(v.at("align", default: "start"))
  let runs(items) = items.map(run => if run.at("break", default: false) { linebreak() } else {
    text(
      weight: if run.at("strong", default: false) { "bold" } else { face.weight },
      style: if run.at("emphasis", default: false) { "italic" } else { "normal" },
      run.text,
    )
  }).join()
  let blocks(items) = items.map(item => if "paragraph" in item { par(runs(item.paragraph)) } else {
    let list-of = item.list
    let entries = list-of.items.map(entry => blocks(entry))
    if "start" in list-of { enum(start: list-of.start, tight: true, ..entries) } else { list(tight: true, ..entries) }
  }).join()
  let body = {
    set text(..face)
    set par(leading: 0pt, spacing: spacing, justify: false)
    set list(spacing: spacing)
    set enum(spacing: spacing)
    set align(horizontal)
    blocks(v.at("blocks", default: ()))
  }
  layout(region => {
    let limit = calc.min(region.width, px(t, "--richtext-content-max-width", default: 640))
    let width = measure(body, width: limit).width
    block(width: if bounds.w { 100% } else { auto }, align(horizontal, block(width: width, body)))
  })
}

// The bytes of the image a resolved `url` names, or `none`.
#let image-of(images, key) = if type(key) == str { images.at(key, default: none) } else { none }

#let node(t, images, grays, n, direction: "column", bounds: (w: true, h: true)) = {
  let kind = n.keys().first()
  let v = n.at(kind)
  if kind == "column" or kind == "row" {
    surface(t, v, bounds, t => stack-of(t, v, kind, bounds, (child, bounds) => node(t, images, grays, child, direction: kind, bounds: bounds)))
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
    surface(t, v, bounds, t => item(t, v, bounds, (child, bounds) => node(t, images, grays, child, bounds: bounds)))
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
    if not fixed { [] } else { rule(t, "divider", down: direction == "row") }
  } else if kind == "lineChart" {
    chart(t, v, "line", bounds)
  } else if kind == "barChart" {
    chart(t, v, "bar", bounds)
  } else if kind == "table" {
    table-of(t, v, bounds)
  } else if kind == "progressBar" {
    progress-bar(t, v, bounds)
  } else if kind == "progressDots" {
    progress-dots(t, v)
  } else if kind == "richText" {
    rich-text(t, v, bounds)
  } else {
    text(..typeface(t, "--label", "small"), "Unknown component " + kind)
  }
}

// TRMNL's title bar is a row in which the title keeps its own width and the
// instance, in the title's face, takes what is left after a gap, at its end.
// Each stays on one line and is cut with an ellipsis: the title only once the
// instance has given way, and an instance with less than an em of room is left
// out.
#let title-bar(t, images, bar) = {
  let bar-paint = paint(t, "title-bar")
  let face = (..typeface(t, "--title-bar", "base"), fill: rgb(bar-paint.at("ink", default: "#000000")))
  let icon = bar.at("icon", default: none)
  let icon = if image-of(images, icon) == none { none } else {
    std.image(image-of(images, icon), height: px(t, "--title-bar-image-height", default: 24))
  }
  let gap = px(t, "--gap", default: 10)
  let title = shown(bar.title)
  let instance = shown(bar.at("instance", default: none))
  // One line of `words` in `room`: whole where it fits, else cut.
  let line(words, room) = pad(top: px(t, "--title-bar-padding-top"), haloed(t,
    if measure(text(..face, words)).width <= room { text(..face, words) } else { box(width: room, clamped(face, words, 1)) }))
  block(
    width: 100%,
    height: px(t, "--title-bar-height", default: 40),
    fill: fill-of(t, bar-paint),
    radius: px(t, "--title-bar-border-radius", default: 10),
    inset: (x: gap),
    layout(region => {
      let lead = if icon == none { 0pt } else { measure(icon).width + px(t, "--gap-small", default: 7) }
      let room = region.width - lead - (if instance == "" { 0pt } else { gap })
      let title-room = calc.min(measure(text(..face, title)).width, room)
      let instance-room = room - title-room
      grid(
        columns: (lead, title-room, 1fr),
        column-gutter: (0pt, if instance == "" { 0pt } else { gap }),
        align: (left + horizon, left + horizon, right + horizon),
        rows: (100%,),
        if icon == none { [] } else { icon },
        line(title, title-room),
        if instance == "" or instance-room < face.size { [] } else { line(instance, instance-room) },
      )
    }),
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
