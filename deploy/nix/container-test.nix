{
  testers,
  container,
}:

# Runs the release image under podman as the README's `docker run` does, and
# checks what that relies on: the server starts with no shell and no
# /etc/localtime, takes its time zone from TZ, serves the web UI and the device
# API, and keeps its database in the /data volume as uid 1000.
testers.runNixOSTest {
  name = "afterimage-container";

  nodes.machine = {
    virtualisation.oci-containers = {
      backend = "podman";
      containers.afterimage = {
        image = "afterimage:${container.imageTag}";
        imageFile = container;
        ports = [ "8080:8080" ];
        volumes = [ "afterimage:/data" ];
        environment.TZ = "Europe/Berlin";
      };
    };
  };

  testScript = ''
    import json

    machine.wait_for_unit("podman-afterimage.service")
    machine.wait_for_open_port(8080)

    machine.succeed("curl -sf http://127.0.0.1:8080/ | grep -q '<div id=\"root\">'")
    setup = json.loads(machine.succeed(
        "curl -sf http://127.0.0.1:8080/api/setup -H 'ID: 41:B4:10:39:A1:24' -H 'Model: og_png'"
    ))
    assert setup["status"] == 200, setup

    machine.succeed(
        "curl -sf -c /tmp/cookies -X POST http://127.0.0.1:8080/manage/auth/first-account"
        " -H 'Content-Type: application/json'"
        " -d '{\"name\": \"Test\", \"username\": \"test\", \"password\": \"a long example password\"}'"
    )
    server = json.loads(machine.succeed("curl -sf -b /tmp/cookies http://127.0.0.1:8080/manage/server"))
    assert server["timeZone"] == "Europe/Berlin", server

    volume = machine.succeed("podman volume inspect afterimage --format '{{.Mountpoint}}'").strip()
    machine.succeed(f"test $(stat -c %u {volume}/afterimage.db) = 1000")
  '';
}
