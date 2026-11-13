{
  lib,
  rustPlatform,
  pkg-config,
  aws-lc,
  sqlite,
  contract,
  design,
  mosquitto,
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
      ../../crates
      ../../model/conformance
      ../../model/trmnl
    ];
  };

  cargoLock.lockFile = ../../Cargo.lock;

  # The generated contract and design, laid out as tools/generate writes them in
  # a checkout.
  postPatch = ''
    cp -r --no-preserve=mode ${contract} gen
    cp -r --no-preserve=mode ${design} gen/design
  '';

  nativeBuildInputs = [ pkg-config ];

  buildInputs = [
    aws-lc
    sqlite
  ];
  # aws-lc-sys finds the system AWS-LC on its own; libsqlite3-sys compiles its
  # bundled SQLite unless told to look for the system's.
  env.LIBSQLITE3_SYS_USE_PKG_CONFIG = "1";

  cargoBuildFlags = [
    "--package"
    "afterimage-server"
  ];
  # Every crate's tests, the conformance test against TRMNL's document included.
  cargoTestFlags = [ "--workspace" ];
  # The broker the MQTT tests publish to.
  nativeCheckInputs = [ mosquitto ];

  meta = {
    inherit (manifest.package) description;
    license = lib.licenses.mit;
    mainProgram = "afterimage";
    platforms = lib.platforms.linux;
  };
}
