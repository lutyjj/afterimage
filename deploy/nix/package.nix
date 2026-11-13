{
  lib,
  runCommand,
  makeBinaryWrapper,
  server,
  web,
  design,
}:

# The server binary, serving the web UI unless AFTERIMAGE_WEB_DIR says otherwise,
# with the licences of the design system and fonts it embeds.
runCommand "afterimage-${server.version}"
  {
    nativeBuildInputs = [ makeBinaryWrapper ];
    inherit (server) version;
    meta = server.meta // {
      mainProgram = "afterimage";
    };
  }
  ''
    makeWrapper ${lib.getExe server} $out/bin/afterimage \
      --set-default AFTERIMAGE_WEB_DIR ${web}
    doc=$out/share/doc/afterimage/design
    mkdir -p $doc/fonts
    cp ${design}/LICENSE $doc/
    cp ${design}/fonts/NOTICE.md ${design}/fonts/OFL.txt $doc/fonts/
  ''
