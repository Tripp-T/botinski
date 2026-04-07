{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    crane.url = "github:ipetkov/crane";
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
                    ];
                  };
                })
              ];
            };
          }
        );
      cargoFile = builtins.fromTOML (builtins.readFile ./Cargo.toml);
    in
    {
      packages = forEachSupportedSystem (
        { pkgs }:
        let
          craneLib = (inputs.crane.mkLib pkgs).overrideToolchain pkgs.rust-toolchain;

          sqlxAndMigrationsFilter = path: _type: builtins.match ".*(\\.sqlx|migrations)(/.*)?$" path != null;
          sqlxOrCargo =
            path: type: (sqlxAndMigrationsFilter path type) || (craneLib.filterCargoSources path type);
          src = pkgs.lib.cleanSourceWith {
            src = ./.;
            filter = sqlxOrCargo;
            name = "source";
          };

          commonArgs = {
            inherit src;
            pname = cargoFile.package.name;
            version = cargoFile.package.version;
            nativeBuildInputs = [ pkgs.pkg-config ];
            buildInputs = [ pkgs.openssl ];
            SQLX_OFFLINE = "true";
          };
          cargoArtifacts = craneLib.buildDepsOnly commonArgs;
          rustApp = craneLib.buildPackage (
            commonArgs
            // {
              inherit cargoArtifacts;
            }
          );

          dockerImage = pkgs.dockerTools.buildLayeredImage {
            name = cargoFile.package.name;
            tag = "latest";

            contents = [
              rustApp
              pkgs.cacert
            ];

            config = {
              Cmd = [ "${rustApp}/bin/${cargoFile.package.name}" ];
              Env = [
                "RUST_LOG=${cargoFile.package.name}=info"
              ];
            };
          };
        in
        {
          default = rustApp;
          docker = dockerImage;
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
            ];
            buildInputs = with pkgs; [
              openssl
            ];
            env = {
              RUST_LOG = "${cargoFile.package.name}=debug";
            };
          };
        }
      );
    };
}
