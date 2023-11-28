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
      type = types.path;
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
    users.users.tg-searcher = {
      isNormalUser = true;
      description = "searcher daemon user";
    };

    systemd.services.tg-searcher = {
      description = "Telegram searcher service";
      after = [ "network.target"  ] ++ (lib.optional cfg.redis.enable "redis-searcher.service");
      wantedBy = [ "multi-user.target" ];
      serviceConfig = {
        ExecStart = "${cfg.package}/bin/tg-searcher --config ${cfg.configFile}";
        User = "tg-searcher";
        StateDirectory = "tg-searcher";
        Restart = "on-failure";
        ReadOnlyPaths = "/";
        ReadWritePaths = "%S/tg-searcher";
        PrivateTmp = true;
      };
    };

    services.redis.servers.searcher = mkIf cfg.redis.enable {
      enable = true;
      port = 6379;
    };
  };
}

