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
        inherit (scope.${system}) contract;
      });

      devShells = forSystems (system: {
        default = scope.${system}.devShell;
      });
    };
}
