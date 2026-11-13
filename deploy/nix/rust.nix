{
  lib,
  craneLib,
  pkg-config,
  aws-lc,
  sqlite,
  contract,
  design,
  mosquitto,
}:

# The Rust workspace, built with crane: its dependencies compile once into
# cargoArtifacts, which the server, the tests and clippy all reuse, so a change
# to the workspace's own code rebuilds only its crates.
let
  fs = lib.fileset;
  workspace = lib.importTOML ../../Cargo.toml;
  manifest = lib.importTOML ../../crates/server/Cargo.toml;

  commonArgs = {
    pname = "afterimage";
    inherit (workspace.workspace.package) version;
    strictDeps = true;

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

    # The generated contract and design, laid out as tools/generate writes them
    # in a checkout. They keep their store timestamps, so cargo finds the
    # generated crates unchanged from one derivation to the next.
    postPatch = ''
      cp -r --no-preserve=mode --preserve=timestamps ${contract} gen
      cp -r --no-preserve=mode --preserve=timestamps ${design} gen/design
    '';

    nativeBuildInputs = [ pkg-config ];
    buildInputs = [
      aws-lc
      sqlite
    ];
    # aws-lc-sys finds the system AWS-LC on its own; libsqlite3-sys compiles its
    # bundled SQLite unless told to look for the system's.
    env.LIBSQLITE3_SYS_USE_PKG_CONFIG = "1";
  };

  cargoArtifacts = craneLib.buildDepsOnly commonArgs;
in
{
  inherit cargoArtifacts;

  server = craneLib.buildPackage (
    commonArgs
    // {
      inherit cargoArtifacts;
      cargoExtraArgs = "--locked --package afterimage-server";
      # The workspace's tests run once, in `tests`.
      doCheck = false;
      meta = {
        inherit (manifest.package) description;
        license = lib.licenses.mit;
        mainProgram = "afterimage";
        platforms = lib.platforms.linux;
      };
    }
  );

  # Every crate's tests, the conformance test against TRMNL's document included,
  # with the broker the MQTT tests publish to.
  tests = craneLib.cargoTest (
    commonArgs
    // {
      inherit cargoArtifacts;
      nativeCheckInputs = [ mosquitto ];
    }
  );

  clippy = craneLib.cargoClippy (
    commonArgs
    // {
      inherit cargoArtifacts;
      cargoClippyExtraArgs = "--all-targets -- --deny warnings";
    }
  );

  # rustfmt over the workspace members; `--all` would reach into gen/.
  fmt = craneLib.cargoFmt { inherit (commonArgs) pname version src postPatch; };
}
