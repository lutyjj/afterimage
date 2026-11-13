{
  dockerTools,
  afterimage,
}:

# The OCI image a release publishes: the server with the web UI it serves and
# the CA certificates its requests trust, running as an unprivileged user that
# keeps its database, images and firmware cache in the /data volume. /tmp is
# where SQLite spills a large sort. The image has no /etc/localtime, so `TZ`
# names the server's time zone, UTC unless a deployment sets another.
dockerTools.buildLayeredImage {
  name = "afterimage";
  tag = afterimage.version;
  contents = [ dockerTools.caCertificates ];
  fakeRootCommands = ''
    mkdir -p data tmp
    chown 1000:1000 data
    chmod 1777 tmp
  '';
  config = {
    Entrypoint = [ "${afterimage}/bin/afterimage" ];
    User = "1000:1000";
    Env = [
      "AFTERIMAGE_BIND=0.0.0.0:8080"
      "AFTERIMAGE_DATA_DIR=/data"
      "TZ=UTC"
    ];
    ExposedPorts."8080/tcp" = { };
    Volumes."/data" = { };
    Labels = {
      "org.opencontainers.image.title" = "Afterimage";
      "org.opencontainers.image.description" = "A self-hosted server for TRMNL e-paper displays";
      "org.opencontainers.image.source" = "https://github.com/lutyjj/afterimage";
      "org.opencontainers.image.licenses" = "MIT";
      "org.opencontainers.image.version" = afterimage.version;
    };
  };
}
