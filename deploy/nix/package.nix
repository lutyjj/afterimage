{
  lib,
  runCommand,
  makeWrapper,
  server,
  web,
}:

# The server binary, serving the web UI unless AFTERIMAGE_WEB_DIR says otherwise.
runCommand "afterimage-${server.version}"
  {
    nativeBuildInputs = [ makeWrapper ];
    inherit (server) version;
    passthru = { inherit server web; };
    meta = server.meta // {
      mainProgram = "afterimage";
    };
  }
  ''
    makeWrapper ${lib.getExe server} $out/bin/afterimage \
      --set-default AFTERIMAGE_WEB_DIR ${web}
  ''
