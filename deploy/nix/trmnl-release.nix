{
  lib,
  fetchurl,
  linkFarm,
}:

# The files of TRMNL's design system release that tools/trmnl-release.json pins,
# each named by its SHA-256: what the token generator and the parity harness
# read, fetched once and offline after that.
linkFarm "afterimage-trmnl-release" (
  lib.mapAttrsToList (url: sha256: {
    name = sha256;
    path = fetchurl { inherit url sha256; };
  }) (lib.importJSON ../../tools/trmnl-release.json)
)
