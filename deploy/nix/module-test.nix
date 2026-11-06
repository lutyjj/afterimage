{
  testers,
  afterimage,
}:

# Boots the NixOS module and checks what a deployment relies on: the service
# starts under its hardening, a device sets up and checks in, the management
# API answers, the web UI and its routes are served, a claimed device gets a
# screen drawn under that hardening, and the state directory outlasts a
# restart.
testers.runNixOSTest {
  name = "afterimage-module";

  nodes.machine = {
    imports = [ ./module.nix ];
    services.afterimage = {
      enable = true;
      package = afterimage;
      settings = {
        AFTERIMAGE_URL = "http://machine";
        # The VM has no internet; TRMNL's firmware list fails at once here.
        AFTERIMAGE_TRMNL_URL = "http://127.0.0.1:9";
      };
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

    def manage(method, path, body):
        return json.loads(machine.succeed(
            f"curl -sf -X {method} http://127.0.0.1:8080/manage{path}"
            " -b /tmp/cookies -c /tmp/cookies -H 'Content-Type: application/json'"
            f" -d '{json.dumps(body)}'"
        ))

    manage("POST", "/auth/first-account", {
        "name": "Alex Example", "username": "alex", "password": "correct-horse-battery",
    })

    # Claimed and given a screen, the device gets a drawn image at its next
    # check-in: Typst, the PNG encoder and the image store run under the
    # unit's hardening.
    device = manage("POST", "/devices", {"code": first["friendly_id"]})["device"]
    screen = manage("POST", "/screens", {
        "name": "Hello", "document": {"content": {"title": {"text": "Hello"}}},
    })["screen"]
    manage("PUT", f"/devices/{device['id']}/playlist", {"playlist": [{"screenId": screen["id"]}]})
    drawn = json.loads(machine.succeed(
        "curl -sf http://127.0.0.1:8080/api/display"
        f" -H 'ID: 41:B4:10:39:A1:01' -H 'Access-Token: {first['api_key']}'"
    ))
    assert drawn["status"] == 0, drawn
    machine.succeed(
        f"curl -sf '{drawn['image_url']}' -H 'Access-Token: {first['api_key']}' -o /tmp/screen.png"
        " && head -c 8 /tmp/screen.png | od -An -tx1 | grep -q '89 50 4e 47 0d 0a 1a 0a'"
    )

    # The database lives in the state directory, so the first account
    # outlasts a restart.
    machine.succeed("test -f /var/lib/private/afterimage/afterimage.db")
    machine.systemctl("restart afterimage.service")
    machine.wait_for_open_port(8080)
    options = json.loads(machine.succeed("curl -sf http://127.0.0.1:8080/manage/auth"))
    assert options["firstAccount"] is False, options
  '';
}
