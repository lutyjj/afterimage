{
  mkShell,
  rust,
  smithy-cli,
  sqlx-cli,
  sqlite,
  jq,
  curl,
}:

mkShell {
  name = "afterimage";

  packages = [
    (rust.override {
      extensions = [
        "clippy"
        "rustfmt"
        "rust-src"
        "rust-analyzer"
      ];
    })
    smithy-cli
    sqlx-cli
    sqlite
    jq
    curl
  ];

  shellHook = ''
    export SMITHY_MAVEN_CACHE="''${SMITHY_MAVEN_CACHE:-$HOME/.cache/smithy/maven}"
  '';
}
