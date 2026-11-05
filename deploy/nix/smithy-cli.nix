{
  lib,
  stdenv,
  fetchzip,
  autoPatchelfHook,
  zlib,
}:

# Smithy's own release archive, which bundles its Java runtime. nixpkgs builds an
# older release from source; moving that build needs a regenerated Gradle lock.
# Update: bump `version`, then replace both hashes with what
# `nix store prefetch-file --unpack <url>` prints for each archive.
let
  release =
    {
      x86_64-linux = {
        cpu = "x86_64";
        hash = "sha256-0jH+DA9iCUIUqz6sJCLasi4kewVHTw0jq5wzopwRDP8=";
      };
      aarch64-linux = {
        cpu = "aarch64";
        hash = "sha256-risjAVcBUyz7wepxx2hm9NeBOrorXduVZJYUyR3O018=";
      };
    }
    .${stdenv.hostPlatform.system};
in
stdenv.mkDerivation (finalAttrs: {
  pname = "smithy-cli";
  version = "1.74.1";

  src = fetchzip {
    url = "https://github.com/smithy-lang/smithy/releases/download/${finalAttrs.version}/smithy-cli-linux-${release.cpu}.zip";
    inherit (release) hash;
  };

  nativeBuildInputs = [ autoPatchelfHook ];
  buildInputs = [
    zlib
    stdenv.cc.cc.lib
  ];

  installPhase = ''
    runHook preInstall
    mkdir -p $out/libexec $out/bin
    cp -r . $out/libexec/smithy-cli
    ln -s $out/libexec/smithy-cli/bin/smithy $out/bin/smithy
    runHook postInstall
  '';

  meta = {
    description = "Smithy interface definition language CLI";
    homepage = "https://smithy.io";
    license = lib.licenses.asl20;
    mainProgram = "smithy";
    platforms = [
      "x86_64-linux"
      "aarch64-linux"
    ];
    sourceProvenance = [ lib.sourceTypes.binaryNativeCode ];
  };
})
