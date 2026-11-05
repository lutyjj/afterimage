{
  lib,
  stdenvNoCC,
  smithy-cli,
}:

let
  fs = lib.fileset;
  root = ../..;
  buildConfig = ../../smithy-build.json;
  maven = (lib.importJSON buildConfig).maven;

  # The codegen plugins resolve from Maven Central, so they are fetched once into a
  # fixed-output Maven repository. Its name carries a hash of the Maven section of
  # smithy-build.json: a dependency change renames the repository, and Nix fetches
  # it again and checks it against `outputHash` instead of reusing the old one.
  #
  # Update after changing that section: set `outputHash = lib.fakeHash;`, run
  # `nix build .#contract.plugins`, and copy the hash Nix reports.
  #
  # Workaround for smithy-lang/smithy-rs#4351: codegen-core 0.1.37 depends on
  # kotlin-stdlib-jdk8 at version "unspecified", which no repository serves.
  # smithy-build.json pins 2.1.0, the version codegen-core manages for its other
  # Kotlin artifacts, which wins mediation; online, the resolver tolerates Maven
  # Central's 404 for the unspecified one, and offline the empty descriptor below
  # gives the same answer. Drop the pin and the descriptor once a codegen-server
  # release resolves without them.
  plugins = stdenvNoCC.mkDerivation {
    name = "afterimage-smithy-plugins-${builtins.substring 0 12 (builtins.hashString "sha256" (builtins.toJSON maven))}";
    src = fs.toSource {
      inherit root;
      fileset = buildConfig;
    };
    nativeBuildInputs = [ smithy-cli ];

    buildPhase = ''
      runHook preBuild
      mkdir -p model/smithy
      HOME=$TMPDIR SMITHY_MAVEN_CACHE=$out smithy validate
      runHook postBuild
    '';

    # The resolver records when and from where it fetched each file, which would
    # make the output depend on the time of the fetch.
    installPhase = ''
      runHook preInstall
      rm -rf $out/.meta
      find $out \( -name _remote.repositories -o -name '*.lastUpdated' -o -name resolver-status.properties \) -delete
      unspecified=$out/org/jetbrains/kotlin/kotlin-stdlib-jdk8/unspecified
      mkdir -p $unspecified
      cat > $unspecified/kotlin-stdlib-jdk8-unspecified.pom <<'EOF'
      <project>
        <modelVersion>4.0.0</modelVersion>
        <groupId>org.jetbrains.kotlin</groupId>
        <artifactId>kotlin-stdlib-jdk8</artifactId>
        <version>unspecified</version>
      </project>
      EOF
      runHook postInstall
    '';

    dontFixup = true;
    outputHashMode = "recursive";
    outputHashAlgo = "sha256";
    outputHash = "sha256-agvTMbdp6ShIYC9XvoqrR0d/9xPDU6HbO0EOITOZwSQ=";
  };
in
stdenvNoCC.mkDerivation {
  pname = "afterimage-contract";
  version = "0.1.0";

  src = fs.toSource {
    inherit root;
    fileset = fs.unions [
      buildConfig
      ../../model/smithy
      ../../tools/generate
    ];
  };
  nativeBuildInputs = [ smithy-cli ];

  env.SMITHY_MAVEN_REPOS = "file://${plugins}";

  # tools/generate owns the layout of gen/, in a checkout and here.
  buildPhase = ''
    runHook preBuild
    HOME=$TMPDIR SMITHY_MAVEN_CACHE=$TMPDIR/maven sh tools/generate
    runHook postBuild
  '';

  installPhase = ''
    runHook preInstall
    cp -r gen $out
    runHook postInstall
  '';

  dontFixup = true;
  passthru = { inherit plugins; };
}
