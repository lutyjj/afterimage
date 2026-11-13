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
    {
      self,
      nixpkgs,
      rust-overlay,
    }:
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
        inherit (scope.${system}) contract design server web tokens;
      });

      # The server's build runs every Rust test; lint runs clippy and rustfmt; the
      # web UI's build type-checks it with the generated client and runs its tests;
      # tokens tests the token generator's reading of the stylesheet; parity builds
      # the parity harness, which runs on demand; module boots the NixOS module in
      # a VM.
      checks = forSystems (system: {
        inherit (scope.${system}) server lint web parity;
        tokens = scope.${system}.tokensTest;
        module = scope.${system}.moduleTest;
      });

      # The NixOS module, running this flake's package unless
      # services.afterimage.package says otherwise.
      nixosModules.default =
        { pkgs, ... }:
        {
          imports = [ ./deploy/nix/module.nix ];
          services.afterimage.package = lib.mkDefault self.packages.${pkgs.stdenv.hostPlatform.system}.default;
        };

      # The token generator, for a run outside the build, and the parity harness.
      apps = forSystems (system: {
        tokens = {
          type = "app";
          program = lib.getExe scope.${system}.tokens;
        };
        parity = {
          type = "app";
          program = lib.getExe scope.${system}.parity;
        };
      });

      devShells = forSystems (system: {
        default = scope.${system}.devShell;
      });
    };
}
