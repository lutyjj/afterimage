{
  lib,
  runCommand,
  tokens,
  trmnlRelease,
}:

# The renderer's design: TRMNL's design tokens for every profile of the
# catalogue snapshot and the fonts of its pinned release, with their licences,
# as tools/tokens/generate.py resolves them in headless Chromium from the
# release's pinned files, so the build runs offline and gives the same output
# for the same pins.
runCommand "afterimage-design" { } ''
  export HOME=$TMPDIR
  ${lib.getExe tokens} ${../../crates/trmnl/catalog} $out ${trmnlRelease}
''
