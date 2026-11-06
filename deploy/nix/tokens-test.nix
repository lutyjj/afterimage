{
  lib,
  runCommand,
  python3,
}:

# The token generator's reading of the stylesheet, tested without a browser.
let
  tools = lib.fileset.toSource {
    root = ../../tools;
    fileset = lib.fileset.unions [
      ../../tools/tokens
      ../../tools/trmnl-release.json
    ];
  };
in
runCommand "afterimage-tokens-test" { nativeBuildInputs = [ python3 ]; } ''
  cd ${tools}/tokens
  PYTHONDONTWRITEBYTECODE=1 python3 -m unittest -v test_generate
  touch $out
''
