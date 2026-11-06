{
  mkShell,
  rust,
  smithy-cli,
  sqlite,
  jq,
  curl,
  nodejs_24,
  pnpm,
  mosquitto,
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
    sqlite
    jq
    curl
    nodejs_24
    pnpm
    # The broker the MQTT tests publish to.
    mosquitto
  ];

  shellHook = ''
    export SMITHY_MAVEN_CACHE="''${SMITHY_MAVEN_CACHE:-$HOME/.cache/smithy/maven}"
  '';
}
