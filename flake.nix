{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    crane.url = "github:ipetkov/crane";
    advisory-db = {
      url = "github:rustsec/advisory-db";
      flake = false;
    };
  };

  outputs =
    inputs:
    let
      supportedSystems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      forEachSupportedSystem =
        f:
        inputs.nixpkgs.lib.genAttrs supportedSystems (
          system:
          f {
            pkgs = import inputs.nixpkgs {
              inherit system;
              overlays = [
                (import inputs.rust-overlay)
                (final: prev: {
                  rust-toolchain = prev.rust-bin.stable.latest.default.override {
                    targets = [
                      "x86_64-unknown-linux-gnu"
                    ];
                    extensions = [
                      "rust-src"
                      "rustfmt"
                      "clippy"
                    ];
                  };
                })
              ];
            };
          }
        );
      cargoFile = fromTOML (builtins.readFile ./Cargo.toml);

      # Per-system project setup. Calls to this are cheap to repeat across
      # outputs: derivations are content-addressed so Nix dedupes the actual
      # builds (cargoArtifacts is built once and reused by every check + package).
      mkProject =
        pkgs:
        let
          craneLib = (inputs.crane.mkLib pkgs).overrideToolchain pkgs.rust-toolchain;
          httpFilter = path: _type: builtins.match ".*(public|input.css)(/.*)?$" path != null;
          sqlxAndMigrationsFilter =
            path: _type: builtins.match ".*(\\.sqlx|migrations)(/.*)?$" path != null;
          srcFilter =
            path: type:
            (sqlxAndMigrationsFilter path type)
            || (httpFilter path type)
            || (craneLib.filterCargoSources path type);
          src = pkgs.lib.cleanSourceWith {
            src = ./.;
            filter = srcFilter;
            name = "source";
          };

          commonArgs = {
            inherit src;
            pname = cargoFile.package.name;
            version = cargoFile.package.version;
            nativeBuildInputs = with pkgs; [
              pkg-config
              tailwindcss_4
              cmake
            ];
            buildInputs = with pkgs; [
              openssl
              libopus
            ];
            SQLX_OFFLINE = "true";
          };
          # Debug-profile deps, shared by the check derivations (fmt is the
          # exception — it doesn't need cargoArtifacts).
          cargoArtifacts = craneLib.buildDepsOnly commonArgs;
          # Release-profile deps, shared by the binary + docker image. Without
          # this, buildPackage (which is --release) recompiles every dep from
          # scratch on every docker build because the debug cargoArtifacts is
          # not reusable across profiles.
          cargoArtifactsRelease = craneLib.buildDepsOnly (
            commonArgs
            // {
              CARGO_PROFILE = "release";
            }
          );
        in
        {
          inherit
            craneLib
            src
            commonArgs
            cargoArtifacts
            cargoArtifactsRelease
            ;
        };
    in
    {
      nixosModules.default = import ./nix/module.nix inputs.self;

      packages = forEachSupportedSystem (
        { pkgs }:
        let
          p = mkProject pkgs;
          rustApp = p.craneLib.buildPackage (
            p.commonArgs
            // {
              cargoArtifacts = p.cargoArtifactsRelease;
              postInstall = ''
                mkdir -p $out/share/site
                cp -R target/dist/. $out/share/site/
              '';
            }
          );
          dockerImage = pkgs.dockerTools.buildLayeredImage {
            name = cargoFile.package.name;
            tag = "latest";

            contents = with pkgs; [
              rustApp
              cacert
              ffmpeg
              yt-dlp
            ];

            config = {
              Cmd = [ "${rustApp}/bin/${cargoFile.package.name}" ];
              Env = [
                "RUST_LOG=${cargoFile.package.name}=info"
                "HTTP_SITE_ROOT=${rustApp}/share/site"
              ];
            };
          };
        in
        {
          default = rustApp;
          docker = dockerImage;
        }
      );

      checks = forEachSupportedSystem (
        { pkgs }:
        let
          p = mkProject pkgs;
        in
        {
          fmt = p.craneLib.cargoFmt {
            inherit (p) src;
          };
          clippy = p.craneLib.cargoClippy (
            p.commonArgs
            // {
              inherit (p) cargoArtifacts;
              cargoClippyExtraArgs = "--all-targets -- -D warnings";
            }
          );
          test = p.craneLib.cargoTest (
            p.commonArgs
            // {
              inherit (p) cargoArtifacts;
            }
          );
          # RustSec CVE scan against the dep tree.
          #
          # cargo-audit 0.22+ no longer reads `audit.toml`, so each ignored
          # advisory has to be passed as a CLI flag here. Every entry needs a
          # justification — where the dep enters our tree, why we can't fix it
          # directly, what would unblock removing the ignore.
          audit = p.craneLib.cargoAudit {
            inherit (p) src;
            advisory-db = inputs.advisory-db;
            cargoAuditExtraArgs = builtins.concatStringsSep " " [
              "--ignore yanked"

              # libcrux-chacha20poly1305 0.0.7 — high-severity panic on overlong
              # ciphertext buffer. Pulled via songbird → davey → openmls →
              # hpke-rs-libcrux. Songbird 0.6.0 has not yet shipped a release
              # bumping davey to a version using libcrux ≥ 0.0.8. We don't use
              # openmls directly. Resolves when songbird publishes > 0.6.0.
              "--ignore RUSTSEC-2026-0124"

              # rsa 0.9.10 — Marvin Attack timing sidechannel. The advisory says
              # "No fixed upgrade is available". Pulled transitively via
              # sqlx-macros-core, which depends on sqlx-mysql for compile-time
              # query validation even though we only use the SQLite backend.
              # Resolves when the rsa crate ships a fix or sqlx makes backend
              # deps in macros-core optional.
              "--ignore RUSTSEC-2023-0071"

              # rustls-webpki 0.102.8 — four advisories all on the same pinned
              # version. Reached via serenity 0.12 → tokio-tungstenite 0.21 →
              # rustls 0.22. Our other rustls path (oauth2/reqwest) is already
              # on rustls-webpki 0.103.13 (fixed). Resolves when serenity or
              # poise bumps tokio-tungstenite past 0.21.
              "--ignore RUSTSEC-2026-0104" # reachable panic in CRL parsing
              "--ignore RUSTSEC-2026-0049" # CRLs not considered authoritative
              "--ignore RUSTSEC-2026-0098" # URI name constraints accepted
              "--ignore RUSTSEC-2026-0099" # wildcard name constraints accepted
            ];
          };
          # Verifies the committed .sqlx/ cache matches the queries in the source.
          # Builds a throwaway sqlite in the sandbox, runs migrations, then has
          # sqlx-cli re-derive the metadata and diff it against .sqlx/.
          sqlx-prepare = p.craneLib.mkCargoDerivation (
            p.commonArgs
            // {
              inherit (p) cargoArtifacts;
              pnameSuffix = "-sqlx-prepare-check";
              nativeBuildInputs = p.commonArgs.nativeBuildInputs ++ [ pkgs.sqlx-cli ];
              SQLX_OFFLINE = "false";
              buildPhaseCargoCommand = ''
                export DATABASE_URL="sqlite://$PWD/sqlx-check.db?mode=rwc"
                sqlx database create
                sqlx migrate run
                cargo sqlx prepare --check -- --all-targets --all-features
              '';
            }
          );
        }
      );

      devShells = forEachSupportedSystem (
        { pkgs }:
        {
          default = pkgs.mkShell {
            packages = with pkgs; [
              rust-toolchain
              pkg-config
              cargo-watch
              just
              sqlx-cli
              tailwindcss_4
              cmake
              ffmpeg
              yt-dlp
            ];
            buildInputs = with pkgs; [
              openssl
              libopus
            ];
            env = {
              RUST_LOG = "${cargoFile.package.name}=debug";
            };
          };
        }
      );
    };
}
