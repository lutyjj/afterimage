// Draws a resolved Afterimage screen: the model's ScreenDocument after the server
// has replaced every template with its value and expanded every `each`. Sizes,
// fonts and fills come from the design tokens of the display's profile, so one
// pt is one layout pixel of TRMNL's design system.

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

// Gray ink for text: the gray itself where the palette has grays, or black,
// since neither a mid gray nor a dither survives as small text in black and white.
#let gray-ink(t, levels) = if levels > 2 { rgb(tok(t, "--gray-50", default: "#999999")) } else { black }

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

// Cuts text to `lines` lines of its own size.
#let clamp(body, lines, size, leading) = if lines == none { body } else {
  block(height: lines * size * leading, clip: true, body)
}

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
  set par(leading: (leading - 1) * face.size)
  clamp(text(..face, str(value.text)), value.at("clamp", default: none), face.size, leading)
}

#let label(t, value, levels) = {
  let size = value.at("size", default: "base")
  let face = typeface(t, "--label", size)
  let body = text(..face, str(value.text))
  let inset = (x: px(t, "--gap-xsmall", default: 5), y: 1pt)
  let radius = px(t, "--rounded-xsmall", default: 5)
  let style = value.at("style", default: "plain")
  if style == "outline" {
    box(stroke: 1pt + black, inset: inset, radius: radius, body)
  } else if style == "underline" {
    underline(offset: 2pt, body)
  } else if style == "gray" {
    text(fill: gray-ink(t, levels), body)
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
  let body(size) = text(..face(size), str(value.text))
  if not value.at("fit", default: false) { return body(base) }
  layout(region => {
    let width = measure(body(base)).width
    if width > region.width and width > 0pt { body(base * region.width / width) } else { body(base) }
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

#let chart(t, value, kind) = layout(region => {
  let series = value.series.filter(s => type(s.values) == array and s.values.len() > 0)
  if series.len() == 0 { return text(..typeface(t, "--label", "small"), "No data") }
  let all = series.map(s => s.values).flatten().map(float)
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
  let width = region.width - axis-width
  let height = (if region.height == float.inf * 1pt { 200pt } else { region.height }) - label-height - 4pt
  let count = calc.max(..series.map(s => s.values.len()))
  let x(i) = if kind == "bar" {
    axis-width + width * (i + 0.5) / count
  } else {
    axis-width + width * i / calc.max(count - 1, 1)
  }
  let y(v) = 2pt + height * (1 - (float(v) - low) / (high - low))
  box(width: region.width, height: height + label-height + 4pt, {
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
          // Whole pixels, so that pixel fonts land on the device's grid.
          place(dx: calc.round((x(i) - 30pt) / 1pt) * 1pt, dy: calc.round((height + 8pt) / 1pt) * 1pt,
            box(width: 60pt, align(center, text(..small, str(label)))))
        }
      }
    }
    let dashes = (none, "dashed", "dotted", "dash-dotted")
    for (n, s) in series.enumerate() {
      let values = s.values.map(float)
      if kind == "bar" {
        let slot = width / count / series.len()
        for (i, v) in values.enumerate() {
          let top = y(calc.max(v, 0))
          let bottom = y(calc.min(v, 0))
          let shade = if n == 0 { black } else { fill(t, "--framework-slot-item-meta-emphasis-2-bg-color", image: "--framework-slot-item-meta-emphasis-2-bg-image") }
          place(dx: axis-width + width * i / count + slot * n + slot * 0.15, dy: top,
            rect(width: slot * 0.7, height: calc.max(bottom - top, 1pt), fill: shade, stroke: none))
        }
      } else {
        let points = values.enumerate().map(((i, v)) => (x(i), y(v)))
        let segments = (curve.move(points.at(0)),)
        for i in range(points.len() - 1) {
          let (p0, p1, p2, p3) = (points.at(calc.max(i - 1, 0)), points.at(i), points.at(i + 1), points.at(calc.min(i + 2, points.len() - 1)))
          let c1 = (p1.at(0) + (p2.at(0) - p0.at(0)) / 6, p1.at(1) + (p2.at(1) - p0.at(1)) / 6)
          let c2 = (p2.at(0) - (p3.at(0) - p1.at(0)) / 6, p2.at(1) - (p3.at(1) - p1.at(1)) / 6)
          segments.push(curve.cubic(c1, c2, p2))
        }
        place(curve(stroke: (thickness: if n == 0 { 4pt } else { 3pt }, cap: "round", join: "round", dash: dashes.at(calc.rem(n, 4))), ..segments))
      }
    }
  })
})

// A column or row: children size to their content, and those that grow share
// what is left. Without growing children, `justify` places the free space.
#let stack-of(t, v, direction, draw) = {
  let children = v.children
  let tracks = children.map(child => if grows(child) { 1fr } else { auto })
  let spacing = gap(t, v.at("gap", default: "medium"))
  let justify = v.at("justify", default: "start")
  let cross = v.at("align", default: "start")
  let free = not tracks.contains(1fr)
  let gutter = if free and justify == "between" and children.len() > 1 { 1fr } else { spacing }
  let cells = children.map(draw)
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
    grid(columns: (100%,), rows: tracks, row-gutter: gutter, align: horizontal, ..cells)
  } else {
    let vertical = if cross == "center" { horizon } else if cross == "end" { bottom } else { top }
    grid(rows: (100%,), columns: tracks, column-gutter: gutter, align: vertical, ..cells)
  }
}

#let item(t, v, draw) = {
  let emphasis = int(v.at("emphasis", default: 1))
  let slot = if emphasis == 1 { "--framework-slot-item-meta" } else { "--framework-slot-item-meta-emphasis-" + str(emphasis) }
  let meta = fill(t, slot + "-bg-color", image: slot + "-bg-image")
  let content = stack(spacing: px(t, "--gap-small", default: 7), ..v.children.map(draw))
  if v.at("plain", default: false) { return content }
  let index = v.at("index", default: none)
  let face = typeface(t, "--item-index", "base")
  grid(
    columns: (if index == none { px(t, "--item-meta-width", default: 10) } else { auto }, 1fr),
    column-gutter: px(t, "--gap-small", default: 7),
    grid.cell(fill: meta, inset: if index == none { 0pt } else { (x: 4pt, y: 2pt) },
      if index == none { [] } else { align(center + horizon, text(..face, str(index))) }),
    content,
  )
}

#let node(t, images, levels, n, direction: "column") = {
  let kind = n.keys().first()
  let v = n.at(kind)
  if kind == "column" or kind == "row" {
    stack-of(t, v, kind, child => node(t, images, levels, child, direction: kind))
  } else if kind == "grid" {
    let columns = int(v.columns)
    grid(
      columns: (1fr,) * columns,
      gutter: gap(t, v.at("gap", default: "medium")),
      ..v.cells.map(cell => grid.cell(colspan: calc.min(int(cell.at("span", default: 1)), columns), node(t, images, levels, cell.content)))
    )
  } else if kind == "item" {
    item(t, v, child => node(t, images, levels, child))
  } else if kind == "title" {
    text-component(t, "--title", v)
  } else if kind == "description" {
    text-component(t, "--description", v)
  } else if kind == "label" {
    label(t, v, levels)
  } else if kind == "value" {
    value(t, v)
  } else if kind == "image" {
    let source = images.at(v.url, default: none)
    if source == none { text(..typeface(t, "--label", "small"), "Image unavailable") } else {
      std.image(source, width: 100%, height: 100%, fit: v.at("fit", default: "contain"))
    }
  } else if kind == "divider" {
    if direction == "row" { line(angle: 90deg, length: 100%) } else { line(length: 100%) }
  } else if kind == "lineChart" {
    chart(t, v, "line")
  } else if kind == "barChart" {
    chart(t, v, "bar")
  } else {
    text(..typeface(t, "--label", "small"), "Unknown component " + kind)
  }
}

#let title-bar(t, images, bar) = {
  let face = typeface(t, "--title-bar", "base")
  let ink = rgb(tok(t, "--framework-slot-title-bar-text-color", default: "#000000"))
  let icon = bar.at("icon", default: none)
  let icon = if icon == none or images.at(icon, default: none) == none { none } else {
    std.image(images.at(icon), height: px(t, "--title-bar-image-height", default: 24))
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
      haloed(t, text(..face, fill: ink, str(bar.title))),
      [],
      haloed(t, text(..face, weight: "regular", fill: ink, str(bar.at("instance", default: "")))),
    ),
  )
}

// The whole screen, `width` by `height` layout pixels, for a palette of
// `levels` colors.
#let screen(document, t, images, levels, width, height) = {
  set page(width: width * 1pt, height: height * 1pt, margin: px(t, "--gap", default: 10), fill: white)
  set text(font: ("Inter",), size: 16pt, fill: black)
  let bar = document.at("titleBar", default: none)
  let content = node(t, images, levels, document.content)
  if bar == none {
    block(width: 100%, height: 100%, content)
  } else {
    grid(rows: (1fr, auto), row-gutter: px(t, "--gap", default: 10), block(width: 100%, height: 100%, content), title-bar(t, images, bar))
  }
}
