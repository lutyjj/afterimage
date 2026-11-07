"""Compares Afterimage's rendering with TRMNL's, case by case.

Each case in cases/ holds one screen twice: as markup of TRMNL's framework, laid
inside `.view.view--full`, and as an Afterimage screen document. TRMNL's side is
drawn the way TRMNL captures a plugin: in Firefox, with the pinned release of the
framework's stylesheet, script and fonts, then reduced with ImageMagick as
trmnlp's image quantizer does. Afterimage's side is the image its management
API's `PreviewScreen` answers, from a server the harness starts on an empty data
directory, so the two meet only at the API.

A case names the profiles it is drawn on, as `model/palette` from TRMNL's
catalogue, by stating each one's limit: the share of pixels, in percent, that
may differ. For each it writes OUT_DIR/<case>/<model>_<palette>.png (TRMNL,
Afterimage, and an overlay: black where both have ink, red where only TRMNL
has, blue where only Afterimage has) and prints the share. It exits with 1 when
a share is over its limit.

The harness speaks the management API's published wire format as any script
does, so it builds on nothing of the server's but that contract.

Usage: parity.py SERVER RELEASE_DIR PINS CATALOG_DIR OUT_DIR [CASE...]

SERVER is the afterimage binary; RELEASE_DIR holds the files PINS
(tools/trmnl-release.json) pins, each named by its SHA-256; CATALOG_DIR holds
TRMNL's models.json and palettes.json.
"""

import base64
import http.cookiejar
import io
import json
import os
import socket
import subprocess
import sys
import tempfile
import time
import urllib.request
from pathlib import Path

import numpy as np
import yaml
from PIL import Image
from playwright.sync_api import sync_playwright

CASES = Path(__file__).parent / "cases"

PAGE = """<!DOCTYPE html><html><head>
<link rel="stylesheet" href="https://trmnl.test/css/plugins.css">
<script type="module" src="https://trmnl.test/js/plugins.js"></script>
<style id="layout-pass">.trmnl .screen {{ --pixel-ratio: 1 !important; }}</style>
</head><body class="environment trmnl">
<div class="{classes}" style="{variables}"><div class="view view--full">{markup}</div></div>
</body></html>"""
# TRMNL lays a plugin out at a pixel ratio of 1 while its scripts run, then
# freezes the page and captures it at the device's size.
READY = (
    "document.readyState === 'complete' && window.TRMNL_PLUGINS_READY === true && "
    "(!document.fonts || document.fonts.status === 'loaded')"
)
FREEZE = """() => {
  const noop = () => 0;
  const newest = window.setTimeout(noop, 0);
  window.setTimeout = window.setInterval = noop;
  window.requestAnimationFrame = noop;
  for (let i = newest; i >= 0; i--) { clearTimeout(i); clearInterval(i); }
  if (window.ResizeObserver) window.ResizeObserver.prototype.observe = noop;
  if (window.MutationObserver) window.MutationObserver.prototype.observe = noop;
  document.documentElement.style.overflow = 'hidden';
  document.body.style.overflow = 'hidden';
  document.getElementById('layout-pass')?.remove();
  document.documentElement.offsetHeight;
}"""


class Release:
    """The pinned files of TRMNL's release, by the last part of their URL."""

    def __init__(self, directory: Path, pins: Path):
        self.files = {
            url.rsplit("/", 1)[1]: directory / digest
            for url, digest in json.loads(pins.read_text()).items()
        }

    def route(self, route):
        name = route.request.url.rsplit("/", 1)[1]
        kinds = {".css": "text/css", ".js": "text/javascript", ".ttf": "font/ttf"}
        kind = next((kind for suffix, kind in kinds.items() if name.endswith(suffix)), None)
        if kind is None or name not in self.files:
            return route.fulfill(status=404, body="")
        return route.fulfill(
            body=self.files[name].read_bytes(),
            content_type=kind,
            headers={"Access-Control-Allow-Origin": "*"},
        )


class Catalog:
    def __init__(self, directory: Path):
        self.models = {m["name"]: m for m in json.loads((directory / "models.json").read_text())["data"]}
        self.palettes = {p["id"]: p for p in json.loads((directory / "palettes.json").read_text())["data"]}

    def screen(self, profile: str):
        """The model, the palette, and the classes and variables TRMNL gives the
        screen element of a device with them and the default font."""
        model_name, _, palette_id = profile.partition("/")
        model = self.models.get(model_name)
        if model is None or palette_id not in model["palette_ids"]:
            raise ValueError(f"{profile} is not a model and one of its palettes in TRMNL's catalogue")
        model, palette = self.models[model_name], self.palettes[palette_id]
        css = model["css"]
        classes = " ".join(
            ["screen", css["classes"]["device"], css["classes"]["size"], css["classes"]["density"],
             palette["framework_class"], "screen--fonts-trmnl"]
        )
        variables = "; ".join(f"{name}: {value}" for name, value in css.get("variables", []))
        return model, palette, classes, variables


class Server:
    """An Afterimage server on an empty data directory, signed in as its first
    account."""

    def __init__(self, binary: str):
        self.data = tempfile.TemporaryDirectory(prefix="afterimage-parity-")
        with socket.socket() as probe:
            probe.bind(("127.0.0.1", 0))
            port = probe.getsockname()[1]
        self.url = f"http://127.0.0.1:{port}"
        environment = {
            "PATH": os.environ.get("PATH", ""),
            "AFTERIMAGE_BIND": f"127.0.0.1:{port}",
            "AFTERIMAGE_DATA_DIR": self.data.name,
            "AFTERIMAGE_TIME_ZONE": "UTC",
            "RUST_LOG": "warn",
        }
        self.process = subprocess.Popen([binary], env=environment)
        self.http = urllib.request.build_opener(
            urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar())
        )
        try:
            self.start()
        except BaseException:
            self.stop()
            raise

    def start(self):
        deadline = time.monotonic() + 30
        while True:
            try:
                self.http.open(f"{self.url}/manage/auth", timeout=2)
                break
            except OSError:
                if time.monotonic() > deadline or self.process.poll() is not None:
                    raise RuntimeError("the server did not start")
                time.sleep(0.1)
        self.post("/manage/auth/first-account",
                  {"name": "Parity", "username": "parity", "password": "parity-harness"})

    def post(self, path: str, body) -> dict:
        request = urllib.request.Request(
            f"{self.url}{path}",
            data=json.dumps(body).encode(),
            headers={"Content-Type": "application/json"},
        )
        with self.http.open(request, timeout=30) as response:
            return json.loads(response.read())

    def draw(self, document: dict, profile: str) -> Image.Image:
        model, palette = profile.split("/")
        answer = self.post("/manage/preview",
                           {"document": document, "display": {"model": model, "palette": palette}})
        if answer["problems"]:
            raise ValueError(f"Afterimage cannot draw it: {answer['problems']}")
        return Image.open(io.BytesIO(base64.b64decode(answer["image"])))

    def stop(self):
        self.process.terminate()
        self.process.wait()
        self.data.cleanup()


def capture(browser, release: Release, catalog: Catalog, markup: str, profile: str) -> Image.Image:
    model, palette, classes, variables = catalog.screen(profile)
    width, height, scale = model["width"], model["height"], model["scale_factor"]
    # A fresh page for each capture: frozen timers and the framework's ready
    # flag would carry over.
    page = browser.new_page(viewport={"width": round(width / scale), "height": round(height / scale)})
    page.route("**/*", release.route)
    page.set_content(PAGE.format(classes=classes, variables=variables, markup=markup), wait_until="load")
    page.wait_for_function(READY, timeout=10_000)
    page.evaluate(FREEZE)
    page.set_viewport_size({"width": width, "height": height})
    page.wait_for_timeout(100)
    raw = page.screenshot(clip={"x": 0, "y": 0, "width": width, "height": height})
    page.close()
    return quantize(raw, palette["grays"])


def quantize(png: bytes, grays: int) -> Image.Image:
    """TRMNL's reduction of a capture to its palette's grays, without dither."""
    if grays <= 2:
        steps, depth = ["-monochrome", "-colors", "2"], "1"
    else:
        steps, depth = ["-colorspace", "Gray", "-dither", "None", "-posterize", str(grays)], {4: "2", 16: "4"}[grays]
    reduced = subprocess.run(
        ["magick", "png:-", *steps, "-alpha", "off", "-depth", depth, "-type", "Palette", "png:-"],
        input=png, capture_output=True, check=True,
    ).stdout
    return Image.open(io.BytesIO(reduced))


def compare(trmnl: Image.Image, afterimage: Image.Image, path: Path) -> float:
    """Writes the side-by-side picture and answers the share of pixels that
    differ."""
    a = np.asarray(trmnl.convert("L"), dtype=np.int16)
    b = np.asarray(afterimage.convert("L"), dtype=np.int16)
    if a.shape != b.shape:
        raise ValueError(f"TRMNL drew {a.shape[::-1]}, Afterimage {b.shape[::-1]}")
    ink_a, ink_b = a < 255, b < 255
    overlay = np.full(a.shape + (3,), 255, np.uint8)
    overlay[ink_a & ink_b] = (0, 0, 0)
    overlay[ink_a & ~ink_b] = (230, 0, 0)
    overlay[~ink_a & ink_b] = (0, 90, 255)
    gap = 16
    side = Image.new("RGB", (a.shape[1] * 3 + gap * 2, a.shape[0]), (200, 200, 200))
    for index, image in enumerate([trmnl.convert("RGB"), afterimage.convert("RGB"), Image.fromarray(overlay)]):
        side.paste(image, (index * (a.shape[1] + gap), 0))
    path.parent.mkdir(parents=True, exist_ok=True)
    side.save(path)
    return float((a != b).mean())


def load(name: str, catalog: Catalog) -> dict:
    """A case, refused before anything is drawn when it names no profile, a
    profile outside the catalogue, or a limit that is not a share."""
    case = yaml.safe_load((CASES / f"{name}.yaml").read_text())
    limits = case.get("limits") or {}
    if not limits:
        raise ValueError(f"{name} names no profile in its limits")
    for profile, limit in limits.items():
        catalog.screen(profile)
        if not isinstance(limit, (int, float)) or isinstance(limit, bool) or not 0 <= limit <= 100:
            raise ValueError(f"{name} {profile}: a limit is a percentage from 0 to 100")
    return case


def main(server_binary, release_dir, pins, catalog_dir, out, names):
    release = Release(Path(release_dir), Path(pins))
    catalog = Catalog(Path(catalog_dir))
    out = Path(out)
    names = names or sorted(path.stem for path in CASES.glob("*.yaml"))
    try:
        cases = {name: load(name, catalog) for name in names}
    except ValueError as error:
        sys.exit(str(error))
    server = Server(server_binary)
    over = []
    try:
        with sync_playwright() as playwright:
            browser = playwright.firefox.launch()
            for name, case in cases.items():
                for profile, limit in case["limits"].items():
                    trmnl = capture(browser, release, catalog, case["trmnl"], profile)
                    afterimage = server.draw(case["afterimage"], profile)
                    share = compare(trmnl, afterimage, out / name / f"{profile.replace('/', '_')}.png")
                    percent = share * 100
                    verdict = "over" if percent > limit else "ok"
                    if verdict != "ok":
                        over.append(f"{name} {profile}")
                    print(f"{name:<12} {profile:<16} {percent:7.4f}%  limit {limit:g}%  {verdict}")
            browser.close()
    finally:
        server.stop()
    print(f"pictures in {out}")
    if over:
        sys.exit(f"over its limit: {', '.join(over)}")


if __name__ == "__main__":
    if len(sys.argv) < 6:
        sys.exit(__doc__)
    main(*sys.argv[1:6], sys.argv[6:])
