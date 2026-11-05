{
  description = "Afterimage, a self-hosted server for TRMNL e-paper displays";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    { nixpkgs, rust-overlay, ... }:
    let
      inherit (nixpkgs) lib;
      forSystems = lib.genAttrs [
        "x86_64-linux"
        "aarch64-linux"
      ];
      scope = forSystems (
        system:
        import ./deploy/nix {
          inherit lib;
          pkgs = nixpkgs.legacyPackages.${system}.extend rust-overlay.overlays.default;
        }
      );
    in
    {
      packages = forSystems (system: {
        default = scope.${system}.afterimage;
        inherit (scope.${system}) contract server web tokens;
      });

      # The server's build runs every Rust test; lint runs clippy and rustfmt; the
      # web UI's build type-checks it with the generated client and runs its tests.
      checks = forSystems (system: {
        inherit (scope.${system}) server lint web;
      });

      # tools/tokens/refresh runs this.
      apps = forSystems (system: {
        tokens = {
          type = "app";
          program = lib.getExe scope.${system}.tokens;
        };
      });

      devShells = forSystems (system: {
        default = scope.${system}.devShell;
      });
    };
}
