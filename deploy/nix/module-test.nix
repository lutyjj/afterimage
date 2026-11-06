{
  testers,
  afterimage,
}:

# Boots the NixOS module and checks what a deployment relies on: the service
# starts under its hardening, a device sets up and checks in, the management
# API answers, the web UI and its routes are served, and the state directory
# outlasts a restart.
testers.runNixOSTest {
  name = "afterimage-module";

  nodes.machine = {
    imports = [ ./module.nix ];
    services.afterimage = {
      enable = true;
      package = afterimage;
      settings.AFTERIMAGE_URL = "http://machine";
    };
    time.timeZone = "Europe/Berlin";
  };

  testScript = ''
    import json

    machine.wait_for_unit("afterimage.service")
    machine.wait_for_open_port(8080)

    def setup(mac):
        return json.loads(machine.succeed(
            f"curl -sf http://127.0.0.1:8080/api/setup -H 'ID: {mac}' -H 'Model: og_png'"
        ))

    first = setup("41:B4:10:39:A1:01")
    assert first["status"] == 200, first
    assert len(first["friendly_id"]) == 6, first

    # Unclaimed, the device keeps showing its code.
    display = json.loads(machine.succeed(
        "curl -sf http://127.0.0.1:8080/api/display"
        f" -H 'ID: 41:B4:10:39:A1:01' -H 'Access-Token: {first['api_key']}'"
    ))
    assert display["status"] == 202, display

    options = json.loads(machine.succeed("curl -sf http://127.0.0.1:8080/manage/auth"))
    assert options["firstAccount"] is True, options

    machine.succeed("curl -sf http://127.0.0.1:8080/ | grep -q '<div id=\"root\">'")
    machine.succeed("curl -sf http://127.0.0.1:8080/devices/1 | grep -q '<div id=\"root\">'")

    # The database lives in the state directory, so the first account
    # outlasts a restart.
    machine.succeed(
        "curl -sf http://127.0.0.1:8080/manage/auth/first-account"
        " -H 'Content-Type: application/json'"
        " -d '{\"name\": \"Alex Example\", \"username\": \"alex\", \"password\": \"correct-horse-battery\"}'"
    )
    machine.succeed("test -f /var/lib/private/afterimage/afterimage.db")
    machine.systemctl("restart afterimage.service")
    machine.wait_for_open_port(8080)
    options = json.loads(machine.succeed("curl -sf http://127.0.0.1:8080/manage/auth"))
    assert options["firstAccount"] is False, options
  '';
}
