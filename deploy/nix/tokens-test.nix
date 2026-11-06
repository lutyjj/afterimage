{
  runCommand,
  python3,
  tokensSource,
}:

# The token generator's reading of the stylesheet, tested without a browser.
runCommand "afterimage-tokens-test" { nativeBuildInputs = [ python3 ]; } ''
  cd ${tokensSource}/tokens
  PYTHONDONTWRITEBYTECODE=1 python3 -m unittest -v test_generate
  touch $out
''
