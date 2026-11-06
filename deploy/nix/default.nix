{ lib, pkgs }:

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
  tokens = self.callPackage ./tokens.nix { };

  devShell = self.callPackage ./devshell.nix { };
})
