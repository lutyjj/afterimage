# Contributing

Requires [Nix](https://nixos.org/download/) with flakes. `nix develop` provides every tool. [AGENTS.md](AGENTS.md) holds the rules; [docs/design.md](docs/design.md) the architecture.

```sh
nix develop
tools/generate                                                  # gen/ from the Smithy model and TRMNL's design system
pnpm install
AFTERIMAGE_DATA_DIR=build/data cargo run -p afterimage-server   # APIs on :8080
pnpm --filter afterimage-web dev                                 # web UI on :5173, proxied to :8080 or $AFTERIMAGE_SERVER
```

| Command | Does |
|---|---|
| `cargo test --workspace` | Rust tests, including conformance with TRMNL's API |
| `pnpm --filter afterimage-web check` | Web UI formatting, types and tests |
| `nix flake check` | Everything CI runs: builds, tests, lints, release image, NixOS VM test |
| `nix run .#parity [CASE...]` | Compares renders with TRMNL's into `build/parity/` |
| `tools/trmnl-excerpt`, `tools/trmnl-catalog` | Refresh TRMNL's device API excerpt and model catalog |

API changes start in `model/smithy/`, then `tools/generate`. Never edit `gen/`.

## Dependencies

| Change | Then |
|---|---|
| Web UI package or [linen](https://github.com/lutyjj/linen/releases) release (`pnpm --filter afterimage-web add <tarball URL>`) | Set `hash = lib.fakeHash;` in `deploy/nix/web.nix`, run `nix build .#web`, paste the reported hash |
| TRMNL design system release | Edit `tools/tokens/generate.py`, run `nix run .#tokens -- crates/trmnl/catalog build/tokens`, pin each printed SHA-256 in `tools/trmnl-release.json` |

To test an unreleased linen: `pnpm link <linen checkout>` in `web/`, and don't commit the resulting `pnpm-workspace.yaml` and `pnpm-lock.yaml` changes.

## Commits and releases

Commits: `type(scope): imperative subject`. Pull requests target `main`.

Tag `vX.Y.Z` (matching `Cargo.toml`) to release: CI publishes `ghcr.io/lutyjj/afterimage:X.Y.Z` and `:latest` for amd64 and arm64, and a GitHub release with the OpenAPI document.
