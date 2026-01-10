{ config, pkgs, lib, ... }:

with lib;
let
  cfg = config.services.tg-searcher;
in
{
  options.services.tg-searcher = {
    enable = mkEnableOption "Telegram searcher service";

    package = lib.mkPackageOption pkgs "tg-searcher" { };

    configFile = mkOption {
      type = types.str;
    };

    redis = mkOption {
      type = types.submodule {
        options = {
          enable = mkEnableOption "Redis server for tg-searcher";
          port = mkOption { type = types.port; default = 6379; };
        };
      };
    };
  };

  config = mkIf cfg.enable {
    systemd.services.tg-searcher = {
      description = "Telegram searcher service";
      after = [ "network.target" ] ++ (lib.optional cfg.redis.enable "redis-searcher.service");
      wantedBy = [ "multi-user.target" ];
      serviceConfig = {
        ExecStart = "${cfg.package}/bin/tg-searcher --config ${cfg.configFile}";
        User = "tg-searcher";
        StateDirectory = "tg-searcher";
        Restart = "on-failure";
        ReadOnlyPaths = "/";
        ReadWritePaths = "%S/tg-searcher";

        # hardening
        RemoveIPC = true;
        ProtectSystem = "strict";
        PrivateTmp = true;
        NoNewPrivileges = true;
        RestrictSUIDSGID = true;
        ProtectHome = true;
        UMask = "0077";

        ProtectHostname = true;
        ProtectProc = "invisible";
        ProcSubset = "pid";
        PrivateUsers = true;
        PrivateDevices = true;

        ProtectControlGroups = true;
        LockPersonality = true;
        RestrictRealtime = true;
        ProtectClock = true;
        ProtectKernelLogs = true;
        ProtectKernelTunables = true;
        ProtectKernelModules = true;
        RestrictNamespaces = true;

        SystemCallArchitectures = "native";

        DynamicUser = true; # implies RemoveIPC, ProtectSystem, PrivateTmp, NoNewPrivileges, RestrictSUIDSGID
        MemoryDenyWriteExecute = true;

        CapabilityBoundingSet = [ ];
        AmbientCapabilities = [ ];

        SystemCallFilter = [ "@system-service" ];

        RestrictAddressFamilies = [ "AF_INET" "AF_INET6" ];
      };
    };

    services.redis.servers.searcher = mkIf cfg.redis.enable {
      enable = true;
      port = 6379;
    };
  };
}

