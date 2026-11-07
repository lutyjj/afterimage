{
  lib,
  stdenvNoCC,
  fetchPnpmDeps,
  pnpmConfigHook,
  pnpm,
  nodejs_24,
  contract,
  linen,
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
  # The workspace includes the generated client and linen, laid out as
  # tools/generate writes them in a checkout.
  withContract = ''
    cp -r --no-preserve=mode ${contract} gen
    cp -r --no-preserve=mode ${linen} gen/linen
  '';
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
  # `hash = "sha256-0CZh51R82aHhQJ09nPnRdpMD/jR9TNNkvLi4OdlmbSs=";`, run `nix build .#web`, and copy the hash Nix reports.
  pnpmDeps = fetchPnpmDeps {
    inherit (finalAttrs) pname version src;
    postPatch = withContract;
    inherit pnpm;
    fetcherVersion = 5;
    hash = "sha256-0CZh51R82aHhQJ09nPnRdpMD/jR9TNNkvLi4OdlmbSs=";
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
