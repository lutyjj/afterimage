{
  mkShell,
  rust,
  smithy-cli,
  sqlx-cli,
  sqlite,
  jq,
  curl,
  nodejs_24,
  pnpm,
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
    nodejs_24
    pnpm
  ];

  shellHook = ''
    export SMITHY_MAVEN_CACHE="''${SMITHY_MAVEN_CACHE:-$HOME/.cache/smithy/maven}"
  '';
}
