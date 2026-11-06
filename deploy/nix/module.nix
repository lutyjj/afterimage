# The NixOS module: Afterimage as a systemd service whose database and images
# live in its state directory, and the firmware images it serves in its cache
# directory. The flake's `nixosModules.default` imports it
# with the flake's package as the default.
{
  config,
  lib,
  ...
}:

let
  inherit (lib) types mkOption;
  cfg = config.services.afterimage;

  # clap reads booleans as `true` and `false`.
  toEnvironment = value: if lib.isBool value then lib.boolToString value else toString value;
  host = if lib.hasInfix ":" cfg.listenAddress then "[${cfg.listenAddress}]" else cfg.listenAddress;
in
{
  options.services.afterimage = {
    enable = lib.mkEnableOption "Afterimage, a server for TRMNL e-paper displays";

    package = mkOption {
      type = types.package;
      description = "The Afterimage package: the server with the web UI it serves.";
    };

    listenAddress = mkOption {
      type = types.str;
      default = "0.0.0.0";
      example = "127.0.0.1";
      description = "Address to listen on. Devices on the network must reach it, directly or through a reverse proxy.";
    };

    port = mkOption {
      type = types.port;
      default = 8080;
      description = "Port to listen on.";
    };

    openFirewall = mkOption {
      type = types.bool;
      default = false;
      description = "Opens `port` in the firewall, for devices and browsers that reach the server without a reverse proxy.";
    };

    settings = mkOption {
      type = types.attrsOf (
        types.oneOf [
          types.str
          types.int
          types.bool
        ]
      );
      default = { };
      example = {
        AFTERIMAGE_URL = "https://afterimage.example";
        AFTERIMAGE_PASSWORD_SIGN_IN = false;
        AFTERIMAGE_OIDC_ISSUER = "https://id.example.test";
        AFTERIMAGE_OIDC_NAME = "Example ID";
      };
      description = ''
        Environment variables of the server, each one `afterimage --help` lists.
        The module sets `AFTERIMAGE_BIND`, `AFTERIMAGE_DATA_DIR` and
        `AFTERIMAGE_CACHE_DIR`, and
        `AFTERIMAGE_TIME_ZONE` from `time.timeZone` when that is set. Secrets
        belong in `environmentFile`, since these values land in the Nix store.
      '';
    };

    environmentFile = mkOption {
      type = types.nullOr types.path;
      default = null;
      example = "/run/secrets/afterimage.env";
      description = ''
        A file of environment variables the service manager reads at start,
        for secrets: `AFTERIMAGE_OIDC_CLIENT_ID`, `AFTERIMAGE_OIDC_CLIENT_SECRET`
        and the `AFTERIMAGE_SECRET_<NAME>` values data sources read as
        `secrets.<NAME>`.
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    systemd.services.afterimage = {
      description = "Afterimage, a server for TRMNL e-paper displays";
      wantedBy = [ "multi-user.target" ];
      wants = [ "network-online.target" ];
      after = [ "network-online.target" ];

      environment = {
        AFTERIMAGE_BIND = "${host}:${toString cfg.port}";
        AFTERIMAGE_DATA_DIR = "%S/afterimage";
        AFTERIMAGE_CACHE_DIR = "%C/afterimage";
      }
      // lib.optionalAttrs (config.time.timeZone != null) {
        AFTERIMAGE_TIME_ZONE = config.time.timeZone;
      }
      // lib.mapAttrs (_: toEnvironment) cfg.settings;

      serviceConfig = {
        ExecStart = lib.getExe cfg.package;
        EnvironmentFile = lib.optional (cfg.environmentFile != null) cfg.environmentFile;
        Restart = "on-failure";

        DynamicUser = true;
        StateDirectory = "afterimage";
        StateDirectoryMode = "0700";
        CacheDirectory = "afterimage";
        CacheDirectoryMode = "0700";
        UMask = "0077";

        # A port below 1024 needs the one capability; nothing else does.
        AmbientCapabilities = lib.optional (cfg.port < 1024) "CAP_NET_BIND_SERVICE";
        CapabilityBoundingSet = lib.optional (cfg.port < 1024) "CAP_NET_BIND_SERVICE";
        NoNewPrivileges = true;
        ProtectSystem = "strict";
        ProtectHome = true;
        PrivateTmp = true;
        PrivateDevices = true;
        ProtectClock = true;
        ProtectHostname = true;
        ProtectKernelLogs = true;
        ProtectKernelModules = true;
        ProtectKernelTunables = true;
        ProtectControlGroups = true;
        ProtectProc = "invisible";
        ProcSubset = "pid";
        RestrictAddressFamilies = [
          "AF_INET"
          "AF_INET6"
          "AF_UNIX"
        ];
        RestrictNamespaces = true;
        RestrictRealtime = true;
        RestrictSUIDSGID = true;
        LockPersonality = true;
        MemoryDenyWriteExecute = true;
        SystemCallArchitectures = "native";
        SystemCallFilter = [
          "@system-service"
          "~@privileged"
        ];
      };
    };

    networking.firewall.allowedTCPPorts = lib.mkIf cfg.openFirewall [ cfg.port ];
  };
}
