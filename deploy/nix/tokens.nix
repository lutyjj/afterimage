{
  writeShellApplication,
  python3,
  playwright-driver,
  tokensSource,
}:

# tools/tokens/generate.py with the Python, Playwright and headless Chromium it
# needs. Chromium runs here only, never in the server.
writeShellApplication {
  name = "afterimage-tokens";
  runtimeInputs = [ (python3.withPackages (packages: [ packages.playwright ])) ];
  text = ''
    export PLAYWRIGHT_BROWSERS_PATH=${playwright-driver.browsers}
    export PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS=true
    exec python3 -I ${tokensSource}/tokens/generate.py "$@"
  '';
}
