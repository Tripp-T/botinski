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
        "x86_64-darwin"
        "aarch64-darwin"
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
    in
    {
      packages = forEachSupportedSystem (
        { pkgs }:
        let
          craneLib = (inputs.crane.mkLib pkgs).overrideToolchain pkgs.rust-toolchain;
          src = craneLib.cleanCargoSource ./.;
          commonArgs = {
            inherit src;
            pname = "botinski";
            version = "0.1.0";
            nativeBuildInputs = [ pkgs.pkg-config ];
            buildInputs = [ pkgs.openssl ];
          };
          cargoArtifacts = craneLib.buildDepsOnly commonArgs;
          rustApp = craneLib.buildPackage (
            commonArgs
            // {
              inherit cargoArtifacts;
            }
          );

          dockerImage = pkgs.dockerTools.buildLayeredImage {
            name = "botinski";
            tag = "latest";

            contents = [
              rustApp
              pkgs.cacert
            ];

            config = {
              Cmd = [ "${rustApp}/bin/botinski" ];
              Env = [
                "RUST_LOG=botinski=info"
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
            buildInputs = with pkgs; [
              rust-toolchain
              openssl
              pkg-config
            ];
            packages = with pkgs; [
              just
              cargo-watch
            ];
            env = {
              RUST_LOG = "botinski=debug";
            };
          };
        }
      );
    };
}
