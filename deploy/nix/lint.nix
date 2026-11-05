{ rust, server }:

# The server's build setup, run as lints: clippy with every workspace lint an error,
# and rustfmt over the workspace members (`--all` would reach into gen/). A toolchain
# with both comes first on PATH.
server.overrideAttrs (previous: {
  pname = "afterimage-lint";

  nativeBuildInputs = [
    (rust.override {
      extensions = [
        "clippy"
        "rustfmt"
      ];
    })
  ]
  ++ previous.nativeBuildInputs;

  buildPhase = ''
    runHook preBuild
    cargo clippy --offline --workspace --all-targets -- --deny warnings
    cargo fmt --check
    runHook postBuild
  '';

  doCheck = false;

  installPhase = ''
    runHook preInstall
    touch $out
    runHook postInstall
  '';
})
