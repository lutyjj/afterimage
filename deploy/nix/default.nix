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

  devShell = self.callPackage ./devshell.nix { };
})
