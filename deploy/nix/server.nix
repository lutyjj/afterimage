{
  lib,
  rustPlatform,
  contract,
}:

let
  fs = lib.fileset;
  workspace = lib.importTOML ../../Cargo.toml;
  manifest = lib.importTOML ../../crates/server/Cargo.toml;
in
rustPlatform.buildRustPackage {
  pname = "afterimage";
  inherit (workspace.workspace.package) version;

  src = fs.toSource {
    root = ../..;
    fileset = fs.unions [
      ../../Cargo.toml
      ../../Cargo.lock
      ../../.sqlx
      ../../crates
      ../../model/conformance
      ../../model/trmnl
    ];
  };

  cargoLock.lockFile = ../../Cargo.lock;

  # The layout tools/generate writes in a checkout.
  postPatch = ''
    mkdir gen
    cp -r ${contract}/device-api gen/device-api
    cp ${contract}/device-api.openapi.json gen/device-api.openapi.json
  '';

  # Queries are checked against .sqlx/, which tools/prepare-queries writes.
  env.SQLX_OFFLINE = "true";

  cargoBuildFlags = [
    "--package"
    "afterimage-server"
  ];
  # Every crate's tests, the conformance test against TRMNL's document included.
  cargoTestFlags = [ "--workspace" ];

  meta = {
    inherit (manifest.package) description;
    mainProgram = "afterimage";
    platforms = lib.platforms.linux;
  };
}
