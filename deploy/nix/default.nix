{
  lib,
  pkgs,
}:

lib.makeScope pkgs.newScope (self: {
  # The latest stable release; nixpkgs trails it.
  rust = pkgs.rust-bin.stable."1.99.1".minimal;
  rustPlatform = pkgs.makeRustPlatform {
    cargo = self.rust;
    rustc = self.rust;
  };
  smithy-cli = self.callPackage ./smithy-cli.nix { };

  contract = self.callPackage ./contract.nix { };
  server = self.callPackage ./server.nix { };
  web = self.callPackage ./web.nix { };
  # The server with the web UI it serves.
  afterimage = self.callPackage ./package.nix { };
  # Boots the NixOS module (module.nix) with that package.
  moduleTest = self.callPackage ./module-test.nix { };
  lint = self.callPackage ./lint.nix { };
  # The token generator with the release pins it reads, laid out as in tools/.
  tokensSource = lib.fileset.toSource {
    root = ../../tools;
    fileset = lib.fileset.unions [
      ../../tools/tokens
      ../../tools/trmnl-release.json
    ];
  };
  tokens = self.callPackage ./tokens.nix { };
  # The pinned files of TRMNL's design system release.
  trmnlRelease = self.callPackage ./trmnl-release.nix { };
  # Afterimage's rendering against TRMNL's, case by case (tools/parity).
  parity = self.callPackage ./parity.nix { };
  # The design tokens and fonts the renderer embeds, generated with tokens.
  design = self.callPackage ./design.nix { };
  tokensTest = self.callPackage ./tokens-test.nix { };

  devShell = self.callPackage ./devshell.nix { };
})
