self:
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.services.botinski;
in
{
  options.services.botinski = {
    enable = lib.mkEnableOption "the botinski Discord bot";

    package = lib.mkOption {
      type = lib.types.package;
      default = self.packages.${pkgs.stdenv.hostPlatform.system}.default;
      defaultText = lib.literalExpression "botinski.packages.\${system}.default";
      description = "botinski package to run.";
    };

    user = lib.mkOption {
      type = lib.types.str;
      default = "botinski";
      description = "User the service runs as. Created automatically when set to the default.";
    };

    group = lib.mkOption {
      type = lib.types.str;
      default = "botinski";
      description = "Group the service runs as. Created automatically when set to the default.";
    };

    dataDir = lib.mkOption {
      type = lib.types.path;
      default = "/var/lib/botinski";
      description = ''
        Persistent state directory. Holds `config.toml` (auto-created on first run
        with defaults) and the SQLite database. Migrations are applied automatically
        on every start.
      '';
    };

    httpAddr = lib.mkOption {
      type = lib.types.str;
      default = "127.0.0.1:3000";
      example = "0.0.0.0:3000";
      description = "Address:port the HTTP server binds to.";
    };

    httpRemoteBaseUrl = lib.mkOption {
      type = lib.types.str;
      example = "https://bot.example.com";
      description = ''
        Public base URL the HTTP server is reachable at. Must match the OAuth
        redirect URI configured in the Discord application
        (`{baseUrl}/api/oauth/callback`).
      '';
    };

    discordClientId = lib.mkOption {
      type = lib.types.str;
      description = ''
        Discord OAuth client id. Not secret — exposed in browser-visible
        OAuth flows — so it's fine in the Nix store. The matching client
        secret and bot token belong in `environmentFile` instead.
      '';
    };

    environmentFile = lib.mkOption {
      type = lib.types.path;
      example = "/run/secrets/botinski.env";
      description = ''
        Path to a file containing the bot's secret environment variables.
        Read by systemd at start time; not copied into the Nix store. Must
        define at least:

        ```
        DISCORD_TOKEN=...
        DISCORD_CLIENT_SECRET=...
        HTTP_SECRET=...           # base64 of 32 bytes, e.g. `openssl rand -base64 32`
        ```

        Suitable as the target of `sops.secrets` / `age.secrets`.
      '';
    };

    databaseMaxConnections = lib.mkOption {
      type = lib.types.ints.positive;
      default = 5;
      description = "sqlx connection pool size.";
    };

    logLevel = lib.mkOption {
      type = lib.types.str;
      default = "botinski=info";
      example = "botinski=debug,songbird=info";
      description = "`RUST_LOG` filter passed to tracing-subscriber.";
    };

    extraEnvironment = lib.mkOption {
      type = lib.types.attrsOf lib.types.str;
      default = { };
      description = ''
        Additional environment variables to pass to the bot. Useful for
        overriding clap-recognised options not exposed elsewhere on this
        module (e.g. `DISCORD_SKIP_REGISTER_COMMANDS=true`).
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    users.users = lib.mkIf (cfg.user == "botinski") {
      botinski = {
        isSystemUser = true;
        group = cfg.group;
        home = cfg.dataDir;
        description = "botinski Discord bot";
      };
    };

    users.groups = lib.mkIf (cfg.group == "botinski") {
      botinski = { };
    };

    systemd.services.botinski = {
      description = "botinski Discord bot";
      after = [ "network-online.target" ];
      wants = [ "network-online.target" ];
      wantedBy = [ "multi-user.target" ];

      environment = {
        CONFIG_PATH = "${cfg.dataDir}/config.toml";
        DATABASE_URL = "sqlite://${cfg.dataDir}/main.db?mode=rwc";
        DATABASE_MAX_CONNECTIONS = toString cfg.databaseMaxConnections;
        HTTP_ADDR = cfg.httpAddr;
        HTTP_REMOTE_BASE_URL = cfg.httpRemoteBaseUrl;
        HTTP_SITE_ROOT = "${cfg.package}/share/site";
        DISCORD_CLIENT_ID = cfg.discordClientId;
        RUST_LOG = cfg.logLevel;
      } // cfg.extraEnvironment;

      # yt-dlp + ffmpeg are runtime requirements; the docker image bundles them
      # but a native install needs them on PATH.
      path = with pkgs; [
        ffmpeg
        yt-dlp
      ];

      serviceConfig = {
        Type = "exec";
        ExecStart = "${cfg.package}/bin/botinski";
        EnvironmentFile = cfg.environmentFile;
        User = cfg.user;
        Group = cfg.group;
        WorkingDirectory = cfg.dataDir;
        ReadWritePaths = [ cfg.dataDir ];
        Restart = "on-failure";
        RestartSec = 5;

        # Systemd sandboxing. None of these prevent legitimate operation
        # (network out, voice UDP, local sqlite, /tmp scratch) but they
        # close off a lot of unrelated kernel surface.
        NoNewPrivileges = true;
        PrivateTmp = true;
        ProtectSystem = "strict";
        ProtectHome = true;
        ProtectKernelTunables = true;
        ProtectKernelModules = true;
        ProtectKernelLogs = true;
        ProtectControlGroups = true;
        ProtectClock = true;
        ProtectHostname = true;
        RestrictNamespaces = true;
        RestrictRealtime = true;
        RestrictSUIDSGID = true;
        LockPersonality = true;
        MemoryDenyWriteExecute = false; # rustc emits JIT-like code paths; safer left off
        SystemCallArchitectures = "native";
        CapabilityBoundingSet = "";
        AmbientCapabilities = "";
      };
    };

    systemd.tmpfiles.rules = [
      "d ${cfg.dataDir} 0750 ${cfg.user} ${cfg.group} -"
    ];
  };
}
