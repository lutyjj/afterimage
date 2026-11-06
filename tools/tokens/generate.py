"""Resolves the design tokens of TRMNL's design system for every display profile.

For each combination of CSS classes the catalogue's models and palettes give a
screen (device, size, density, bit depth), the script loads the pinned compiled
stylesheet in headless Chromium, applies every token the release documents to a
probe element of the matching kind and records the computed value. It also lays
out the release's own markup for each component and records what each part
paints, on the screen and inside an `.inverse` element: a color, a dither tile,
or a row of pixels for a rule. It copies the release's fonts with their notices
and the licence texts that cover them and the tokens.

Usage: generate.py CATALOG_DIR OUTPUT_DIR

CATALOG_DIR holds TRMNL's models.json and palettes.json; OUTPUT_DIR receives
tokens.json, LICENSE (the design system's), fonts/, fonts/NOTICE.md and
fonts/OFL.txt.
"""

import hashlib
import json
import re
import sys
import urllib.parse
import urllib.request
from pathlib import Path


FRAMEWORK = "3.4.4"
STYLESHEET = f"https://trmnl.com/css/{FRAMEWORK}/plugins.css"
# The stylesheet's SHA-256 when FRAMEWORK was pinned; a different download stops
# the run instead of producing tokens from an unreviewed stylesheet.
STYLESHEET_SHA256 = "af12fdc34f6186fa0a208fe1c1dc37a14e98c8bfa09139e1be02e0600669be18"
# The commit tag v3.4.4 names; a tag can move, a commit cannot.
REPOSITORY = "https://raw.githubusercontent.com/usetrmnl/trmnl-framework/f366a9303bbce97adca73d5747da8e2212379bbc"
TOKEN_LIST = f"{REPOSITORY}/public/framework/docs/3.4/tokens.md"
FONT_REGISTRY = f"{REPOSITORY}/db/data/framework_fonts.yml"
FRAMEWORK_LICENSE = f"{REPOSITORY}/LICENSE"
# The SIL Open Font License's text, as Inter v4.1 ships it.
OFL = "https://raw.githubusercontent.com/rsms/inter/e3a3d4c57d5ecc01453a575621882a384c1995a3/LICENSE.txt"
# The SHA-256 of every file fetched besides the stylesheet. A file without an
# entry, or with another digest, stops the run and prints the digest it got, to
# review and add here after changing a pin.
DIGESTS = {
    TOKEN_LIST: "58426e2536b4f011276e490a18762bde709ebe0b19855ebcff8f038a695c0f74",
    FONT_REGISTRY: "43a4e78aac3806e2026406f607f84b137748feb2aa2a7fde654d38c8e82197bb",
    FRAMEWORK_LICENSE: "625120fbe9ba35bab1d0376cdc5352b0f08c91d0c68711a58cc606e563c3176b",
    OFL: "262481e844521b326f5ecd053e59b98c8b2da78c8ee1bdbb6e8174305e54935a",
    f"{REPOSITORY}/public/fonts/Inter.ttf": "0be2399ea925f1f83ff974764761da9860ec50742ed29a5d4c1ffd0c5c7ac3a8",
    f"{REPOSITORY}/public/fonts/Inter-Italic.ttf": "6136f73372fedc37b80fd1a8ec3e21734073ff17376f3672718f186779672e7a",
    f"{REPOSITORY}/public/fonts/TRMNL12-Regular.ttf": "17711493bd26695a40b9c72557a6c2f4611a30a81845c5aaa117648f3a0b3795",
    f"{REPOSITORY}/public/fonts/TRMNL12-Bold.ttf": "1b7a292a4dec1708f9c78b69a5380c617a6add97636284c2d22803018b6b57cc",
    f"{REPOSITORY}/public/fonts/TRMNL16-Regular.ttf": "d442c4a1fa1973b2210f8745a604f0353ceb422448825445884b2b9719e43095",
    f"{REPOSITORY}/public/fonts/TRMNL16-Bold.ttf": "8907dc5ef0db8a14731ad24917db79fb5f2e019f0fc03b624cb675538d598958",
    f"{REPOSITORY}/public/fonts/TRMNL21-Regular.ttf": "1ece6f9200955d51f8e6c941e13fa3d121555e532a76f0c0b88664915c5c444f",
    f"{REPOSITORY}/public/fonts/TRMNL21-Bold.ttf": "1f82225e7fbcfda2047f01184ac6f387ce8d3796ac9615cc38e4fa2048310b31",
}
# Internal custom properties that are not documented tokens but that drawing
# needs: the size of a dither tile.
FILLS = ["--dither-bg-size"]

# What each component paints, read from the computed style of the release's own
# markup rather than from the slots behind it, so the stylesheet's cascade picks
# the slot for each profile. Each paint is probed on the screen and again inside
# an `.inverse` element, whose paints are recorded under `inverse/<name>`.
PAINT_MARKUP = """
<div class="title_bar"><span class="title">T</span><span class="instance">I</span></div>
<div class="item"><div class="meta"></div><div class="content"><span class="title">T</span></div></div>
<div class="item item--emphasis-2"><div class="meta"></div><div class="content"><span class="title">T</span></div></div>
<div class="item item--emphasis-3"><div class="meta"></div><div class="content"><span class="title">T</span></div></div>
<table class="table table--indexed"><thead><tr><th><span class="title">H</span></th></tr></thead>
<tbody><tr><td><div class="meta"><span class="index">1</span></div><span class="label">A</span></td></tr>
<tr><td><span class="label">B</span></td></tr></tbody></table>
<div class="progress-bar"><div class="track"><div class="fill" style="width: 50%"></div></div></div>
<div class="progress-bar progress-bar--emphasis-2"><div class="track"><div class="fill" style="width: 50%"></div></div></div>
<div class="progress-bar progress-bar--emphasis-3"><div class="track"><div class="fill" style="width: 50%"></div></div></div>
<div class="progress-dots"><div class="track"><div class="dot dot--filled"></div><div class="dot dot--current"></div><div class="dot"></div></div></div>
<span class="label label--gray">G</span>
<div class="divider"></div>
"""
# Paint name: the element's selector within the markup.
PAINTS = {
    "canvas": ":scope",
    "title-bar": ".title_bar",
    "item-meta": ".item:not([class*=emphasis]) .meta",
    "item-meta-emphasis-2": ".item--emphasis-2 .meta",
    "item-meta-emphasis-3": ".item--emphasis-3 .meta",
    "table-head-rule": ".table thead tr",
    "table-body-rule": ".table tbody tr:first-child",
    "table-meta": ".table .meta",
    "table-index": ".table .index",
    "progress-track": ".progress-bar:not([class*=emphasis]) .track",
    "progress-fill": ".progress-bar:not([class*=emphasis]) .fill",
    "progress-fill-emphasis-2": ".progress-bar--emphasis-2 .fill",
    "progress-fill-emphasis-3": ".progress-bar--emphasis-3 .fill",
    "progress-dot": ".dot:not([class*=dot--])",
    "progress-dot-current": ".dot--current",
    "progress-dot-filled": ".dot--filled",
    "label-gray": ".label--gray",
    "divider": ".divider",
}

# Each token goes through the CSS property that resolves its kind, so lengths come
# back in pixels and colors as rgb().
PROBE = """
names => {
  const screen = document.querySelector('.screen');
  const probe = document.createElement('div');
  screen.appendChild(probe);
  const raw = name => getComputedStyle(screen).getPropertyValue(name).trim();
  const through = (property, name) => {
    probe.style.setProperty(property, `var(${name})`);
    const value = getComputedStyle(probe).getPropertyValue(property);
    probe.removeAttribute('style');
    return value;
  };
  const out = {};
  for (const name of names) {
    let value;
    if (/-font-family$/.test(name)) value = ['family', through('font-family', name)];
    else if (/-font-weight$/.test(name)) value = ['number', through('font-weight', name)];
    else if (/-line-height$/.test(name)) {
      // Against the font size of the same component and size, since a line
      // height may be a ratio or a length; a ratio to that size either way.
      const size = name.replace(/-line-height$/, '-font-size');
      probe.style.setProperty('font-size', raw(size) ? `var(${size})` : '100px');
      probe.style.setProperty('line-height', `var(${name})`);
      const style = getComputedStyle(probe);
      value = ['ratio', `${style.getPropertyValue('line-height')} ${style.getPropertyValue('font-size')}`];
      probe.removeAttribute('style');
    }
    else if (/^--(gray|black|white|color)/.test(name))
      value = ['color', through('color', name)];
    else if (/-font-smoothing$/.test(name)) value = ['text', raw(name)];
    else value = ['length', through('width', name)];
    out[name] = value;
  }
  probe.remove();
  return out;
}
"""

# Builds the paint markup on the screen, inside an `.inverse` element when asked,
# and reads what each paint's element draws.
PAINT_PROBE = """
([markup, paints, inverse]) => {
  const screen = document.querySelector('.screen');
  const root = document.createElement('div');
  if (inverse) root.className = 'inverse';
  root.innerHTML = markup;
  screen.appendChild(root);
  const ratio = parseFloat(getComputedStyle(screen).getPropertyValue('--dither-ratio')) || 1;
  const out = {};
  for (const [name, selector] of Object.entries(paints)) {
    const element = selector === ':scope' ? root : root.querySelector(selector);
    if (!element) { out[name] = null; continue; }
    const style = getComputedStyle(element);
    out[name] = {
      color: style.backgroundColor,
      image: style.backgroundImage,
      size: style.backgroundSize,
      ratio,
      ink: style.color,
      border: style.borderTopStyle === 'none' ? '' : style.borderTopColor,
      halo: style.textShadow,
    };
  }
  root.remove();
  return out;
}
"""


def fetch(url: str) -> bytes:
    request = urllib.request.Request(url, headers={"User-Agent": "afterimage-tokens"})
    with urllib.request.urlopen(request, timeout=60) as response:
        return response.read()


def pinned(url: str) -> bytes:
    """A file of the pinned sources, checked against its recorded digest."""
    data = fetch(url)
    digest = hashlib.sha256(data).hexdigest()
    if DIGESTS.get(url) != digest:
        sys.exit(f"{url} has SHA-256 {digest}, not the pinned {DIGESTS.get(url)}")
    return data


def typed(kind: str, value: str):
    """A probe's computed value as JSON: pixels as numbers, colors as #RRGGBB, an
    image as its SVG text, a family as its first name."""
    value = value.strip()
    if kind == "length":
        match = re.fullmatch(r"(-?[0-9.]+)px", value)
        return float(match.group(1)) if match else None
    if kind == "number":
        return float(value) if re.fullmatch(r"-?[0-9.]+", value) else None
    if kind == "ratio":
        match = re.fullmatch(r"(-?[0-9.]+)px (-?[0-9.]+)px", value)
        return round(float(match.group(1)) / float(match.group(2)), 4) if match else None
    if kind == "color":
        match = re.fullmatch(r"rgba?\(([0-9]+), ([0-9]+), ([0-9]+)(?:, ([0-9.]+))?\)", value)
        if not match or (match.group(4) is not None and float(match.group(4)) == 0):
            return None
        return "#{:02X}{:02X}{:02X}".format(*(int(match.group(i)) for i in (1, 2, 3)))
    if kind == "family":
        first = value.split(",")[0].strip().strip('"')
        return first or None
    if kind == "image":
        match = re.fullmatch(r'url\("data:image/svg\+xml,(.*)"\)', value)
        return urllib.parse.unquote(match.group(1)) if match else None
    return value or None


def split(text: str) -> list[str]:
    """Comma-separated parts outside parentheses."""
    parts, depth, start = [], 0, 0
    for at, character in enumerate(text):
        depth += {"(": 1, ")": -1}.get(character, 0)
        if character == "," and depth == 0:
            parts.append(text[start:at].strip())
            start = at + 1
    parts.append(text[start:].strip())
    return parts


def pattern(image: str, size: str, ratio: float):
    """The first layer of a linear gradient as a rule: one row of pixel colours,
    `None` where it is transparent, sampled at the display's own pixels (`ratio`
    of them to a CSS pixel, as the stylesheet sizes its rules), with the CSS width
    the row repeats over and its thickness. Hard stops on few grays draw dotted
    rules; one colour on many draws a plain one."""
    layer = split(image)[0]
    if not layer.startswith("linear-gradient("):
        return None
    arguments = split(layer[len("linear-gradient(") : -1])
    # CSS runs a gradient without a direction to the bottom, as no rule across is.
    across = False
    if arguments and re.match(r"to |-?[0-9.]+deg$", arguments[0]):
        across = arguments[0] in ("to right", "90deg")
        arguments = arguments[1:]
    stops = []
    for stop in arguments:
        match = re.fullmatch(r"(rgba?\([^)]*\))(?: ([0-9.]+)%)?", stop)
        if not match:
            return None
        stops.append((typed("color", match.group(1)), match.group(2) and float(match.group(2))))
    lengths = [re.fullmatch(r"([0-9.]+)px", part) for part in split(size)[0].split(" ")]
    thickness = float(lengths[1].group(1)) if len(lengths) > 1 and lengths[1] else 1.0
    if len({color for color, _ in stops}) == 1:
        return {"pixels": [stops[0][0]], "period": round(1 / ratio, 4), "thickness": thickness}
    if not across or not lengths[0] or any(position is None for _, position in stops):
        return None
    period = float(lengths[0].group(1))
    pixels = max(1, round(period * ratio))
    row = []
    for pixel in range(pixels):
        centre = (pixel + 0.5) * 100 / pixels
        row.append(next((color for (color, start), (_, end) in zip(stops, stops[1:]) if start <= centre < end), None))
    return {"pixels": row, "period": round(period, 4), "thickness": round(thickness, 4)}


def paint(raw):
    """What an element paints: its fill (a colour, a dither tile or a rule), its
    ink, its border colour and the colour of its text's halo, each present only
    when it has one."""
    if raw is None:
        return None
    tile = typed("image", raw["image"])
    halo = re.match(r"(rgba?\([^)]*\))", raw["halo"] or "")
    found = {
        "color": typed("color", raw["color"]),
        "tile": tile,
        "pattern": None if tile else pattern(raw["image"], raw["size"], raw["ratio"]),
        "ink": typed("color", raw["ink"]),
        "border": typed("color", raw["border"]) if raw["border"] else None,
        "halo": typed("color", halo.group(1)) if halo else None,
    }
    return {key: value for key, value in found.items() if value is not None}


def profiles(catalog: Path):
    """Each model and palette pair, with the classes and inline variables its
    screen element carries."""
    models = json.loads((catalog / "models.json").read_text())["data"]
    palettes = {p["id"]: p for p in json.loads((catalog / "palettes.json").read_text())["data"]}
    for model in models:
        css = model.get("css")
        if not css:
            continue
        classes = css["classes"]
        variables = "; ".join(f"{name}: {value}" for name, value in css.get("variables", []))
        for palette in model["palette_ids"]:
            screen = " ".join(
                [classes["device"], classes["size"], classes["density"], palettes[palette]["framework_class"]]
            )
            yield f"{model['name']}/{palette}", screen, variables


def fonts(output: Path, families: set[str]) -> None:
    """Copies the TTF files of the families the tokens name, with a notice for
    each from the release's font registry."""
    registry = pinned(FONT_REGISTRY).decode()
    target = output / "fonts"
    target.mkdir(parents=True, exist_ok=True)
    notices = ["# Fonts\n", f"From TRMNL's design system {FRAMEWORK} (`public/fonts/`).\n"]
    for block in re.split(r"\n\s*- family: ", registry)[1:]:
        family = block.split("\n", 1)[0].strip()
        if family not in families and family.replace(" Variable", "") not in families:
            continue
        files = re.findall(r"^\s*- ([A-Za-z0-9-]+\.ttf)$", block, re.M)
        field = lambda key: (re.search(rf"^\s*{key}: (.+)$", block, re.M) or [None, ""])[1].strip()
        for name in files:
            (target / name).write_bytes(pinned(f"{REPOSITORY}/public/fonts/{name}"))
        notices.append(
            f"\n## {family}\n\n{field('copyright').rstrip('.')}. Designed by {field('designer')}. "
            f"Licensed under the {field('license')} ({field('license_url')}). "
            f"Files: {', '.join(f'`{name}`' for name in files)}.\n"
        )
    notices.append(
        "\nThe SIL Open Font License's text is in `OFL.txt`; the design system's own licence, "
        "which covers `tokens.json`, is in `../LICENSE`.\n"
    )
    (target / "NOTICE.md").write_text("".join(notices))
    # Inter's copy of the licence, from its first line of the licence itself.
    text = pinned(OFL).decode()
    (target / "OFL.txt").write_text(text[text.index("This Font Software is licensed") :])


def main(catalog: Path, output: Path) -> None:
    # Imported here, so the parsing above loads without a browser for its tests.
    from playwright.sync_api import sync_playwright

    stylesheet = fetch(STYLESHEET)
    digest = hashlib.sha256(stylesheet).hexdigest()
    if digest != STYLESHEET_SHA256:
        sys.exit(f"{STYLESHEET} has SHA-256 {digest}, not the pinned {STYLESHEET_SHA256}")
    names = re.findall(r"^\| `(--[a-z0-9-]+)`", pinned(TOKEN_LIST).decode(), re.M)
    sets: list[dict] = []
    index: dict[str, int] = {}
    chosen: dict[str, int] = {}
    with sync_playwright() as playwright:
        browser = playwright.chromium.launch()
        page = browser.new_page()
        page.route("**/plugins.css", lambda route: route.fulfill(body=stylesheet, content_type="text/css"))
        page.set_content(
            '<html><head><link rel="stylesheet" href="https://afterimage.invalid/plugins.css"></head>'
            '<body class="environment trmnl"><div class="screen"></div></body></html>',
            wait_until="load",
        )
        for profile, classes, variables in profiles(catalog):
            page.evaluate(
                "([classes, variables]) => { const s = document.querySelector('.screen');"
                " s.className = 'screen ' + classes; s.setAttribute('style', variables); }",
                [classes, variables],
            )
            raw = page.evaluate(PROBE, names + FILLS)
            tokens = {name: typed(*raw[name]) for name in names + FILLS}
            for inverse in (False, True):
                painted = page.evaluate(PAINT_PROBE, [PAINT_MARKUP, PAINTS, inverse])
                prefix = "paint/inverse/" if inverse else "paint/"
                tokens.update({prefix + name: paint(painted[name]) for name in PAINTS})
            key = json.dumps(tokens, sort_keys=True)
            if key not in index:
                index[key] = len(sets)
                sets.append(tokens)
            chosen[profile] = index[key]
        browser.close()
    families = {value for tokens in sets for name, value in tokens.items() if name.endswith("-font-family") and value}
    # The few distinct dither tiles stand once in `tiles`; a paint names its tile
    # by index.
    tiles: list[str] = []
    for tokens in sets:
        for value in tokens.values():
            if isinstance(value, dict) and "tile" in value:
                if value["tile"] not in tiles:
                    tiles.append(value["tile"])
                value["tile"] = tiles.index(value["tile"])
    output.mkdir(parents=True, exist_ok=True)
    document = {
        "framework": FRAMEWORK,
        "stylesheet_sha256": digest,
        "profiles": dict(sorted(chosen.items())),
        "sets": sets,
        "tiles": tiles,
    }
    (output / "tokens.json").write_text(json.dumps(document, indent=1, sort_keys=True) + "\n")
    (output / "LICENSE").write_bytes(pinned(FRAMEWORK_LICENSE))
    fonts(output, families)
    print(
        f"{len(chosen)} profiles, {len(sets)} token sets, {len(tiles)} tiles, "
        f"fonts: {', '.join(sorted(families))}"
    )


if __name__ == "__main__":
    main(Path(sys.argv[1]), Path(sys.argv[2]))
