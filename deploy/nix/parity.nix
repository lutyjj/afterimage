{
  lib,
  writeShellApplication,
  python3,
  playwright-driver,
  imagemagick,
  server,
  trmnlRelease,
}:

# tools/parity/parity.py with what it needs: the server it draws with, Firefox
# for TRMNL's side, ImageMagick for TRMNL's reduction, and the pinned release.
# Pictures go to build/parity, or to $PARITY_OUT.
writeShellApplication {
  name = "afterimage-parity";
  runtimeInputs = [
    (python3.withPackages (packages: [
      packages.numpy
      packages.pillow
      packages.playwright
      packages.pyyaml
    ]))
    imagemagick
  ];
  text = ''
    export PLAYWRIGHT_BROWSERS_PATH=${playwright-driver.browsers}
    export PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS=true
    exec python3 -I ${../../tools/parity}/parity.py ${lib.getExe' server "afterimage"} \
      ${trmnlRelease} ${../../tools/trmnl-release.json} ${../../crates/trmnl/catalog} \
      "''${PARITY_OUT:-build/parity}" "$@"
  '';
}
