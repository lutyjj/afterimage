{
  lib,
  runCommand,
  fetchurl,
  linkFarm,
  tokens,
}:

# The renderer's design: TRMNL's design tokens for every profile of the
# catalogue snapshot and the fonts of its pinned release, with their licences,
# as tools/tokens/generate.py resolves them in headless Chromium. Every file it
# reads is pinned in tools/tokens/sources.json and fetched here, so the build
# runs offline and gives the same output for the same pins.
let
  pins = lib.importJSON ../../tools/tokens/sources.json;
  sources = linkFarm "afterimage-design-sources" (
    lib.mapAttrsToList (url: sha256: {
      name = sha256;
      path = fetchurl { inherit url sha256; };
    }) pins
  );
in
runCommand "afterimage-design" { } ''
  export HOME=$TMPDIR
  ${lib.getExe tokens} ${../../crates/trmnl/catalog} $out ${sources}
''
