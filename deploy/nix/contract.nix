{
  lib,
  stdenvNoCC,
  smithy-cli,
}:

let
  fs = lib.fileset;
  root = ../..;
  buildConfig = ../../smithy-build.json;

  # The codegen plugins resolve from Maven Central, so they are fetched once into a
  # fixed-output Maven repository keyed by smithy-build.json alone.
  #
  # Workaround for smithy-lang/smithy-rs#4351: codegen-core 0.1.37 depends on
  # kotlin-stdlib-jdk8 at version "unspecified", which no repository serves.
  # smithy-build.json pins a real version, which wins mediation; online, the
  # resolver tolerates Maven Central's 404 for the unspecified one, and offline the
  # empty descriptor below gives the same answer. Drop the pin and the descriptor
  # once a codegen-server release resolves without them.
  plugins = stdenvNoCC.mkDerivation {
    name = "afterimage-smithy-plugins";
    src = fs.toSource {
      inherit root;
      fileset = buildConfig;
    };
    nativeBuildInputs = [ smithy-cli ];

    buildPhase = ''
      runHook preBuild
      mkdir model
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
    outputHash = "sha256-RW7yrGAS2h03+nuZNx0HNSjCkI6aJfOQ+UJ8/gjGngQ=";
  };
in
stdenvNoCC.mkDerivation {
  pname = "afterimage-contract";
  version = "0.1.0";

  src = fs.toSource {
    inherit root;
    fileset = fs.unions [
      buildConfig
      (fs.fileFilter (file: file.hasExt "smithy") ../../model)
    ];
  };
  nativeBuildInputs = [ smithy-cli ];

  env.SMITHY_MAVEN_REPOS = "file://${plugins}";

  buildPhase = ''
    runHook preBuild
    HOME=$TMPDIR SMITHY_MAVEN_CACHE=$TMPDIR/maven smithy build
    runHook postBuild
  '';

  # The same layout tools/generate writes into gen/.
  installPhase = ''
    runHook preInstall
    mkdir $out
    cp -r build/smithy/source/rust-server-codegen $out/device-api
    cp build/smithy/source/openapi/DeviceApi.openapi.json $out/device-api.openapi.json
    runHook postInstall
  '';

  dontFixup = true;
  passthru = { inherit plugins; };
}
