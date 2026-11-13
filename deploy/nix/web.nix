{
  lib,
  stdenvNoCC,
  fetchPnpmDeps,
  pnpmConfigHook,
  pnpm,
  nodejs_24,
  contract,
}:

# The web UI's static files. Its build type-checks the app together with the
# generated client it compiles from source, runs its unit tests, and bundles it.
let
  fs = lib.fileset;
  version = (lib.importJSON ../../web/package.json).version or "0.1.0";
  src = fs.toSource {
    root = ../..;
    fileset = fs.unions [
      ../../package.json
      ../../pnpm-lock.yaml
      ../../pnpm-workspace.yaml
      (fs.difference ../../web (
        fs.unions [
          (fs.maybeMissing ../../web/node_modules)
          (fs.maybeMissing ../../web/dist)
        ]
      ))
    ];
  };
  # The workspace includes the generated client, laid out as tools/generate
  # writes it in a checkout.
  withContract = "cp -r --no-preserve=mode ${contract} gen";
in
stdenvNoCC.mkDerivation (finalAttrs: {
  pname = "afterimage-web";
  inherit version src;

  postPatch = withContract;

  nativeBuildInputs = [
    nodejs_24
    pnpm
    pnpmConfigHook
  ];

  # Fetched once into a fixed output. After changing pnpm-lock.yaml, set
  # `hash = lib.fakeHash;`, run `nix build .#web`, and copy the hash Nix reports.
  pnpmDeps = fetchPnpmDeps {
    inherit (finalAttrs) pname version src;
    postPatch = withContract;
    inherit pnpm;
    fetcherVersion = 5;
    hash = "sha256-QM0xAlpEXhtvPjAfdud3tfG2+mjO9105H3/MBdVzAxM=";
  };

  buildPhase = ''
    runHook preBuild
    pnpm --filter afterimage-web run build
    runHook postBuild
  '';

  doCheck = true;
  checkPhase = ''
    runHook preCheck
    pnpm --filter afterimage-web run format:check
    pnpm --filter afterimage-web run test
    runHook postCheck
  '';

  installPhase = ''
    runHook preInstall
    cp -r web/dist $out
    runHook postInstall
  '';
})
