{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixpkgs-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    systems.url = "github:nix-systems/default";
  };

  outputs =
    inputs:
    inputs.flake-parts.lib.mkFlake { inherit inputs; } {
      systems = import inputs.systems;

      perSystem =
        { pkgs, ... }:
        let
          nativeBuildInputs = with pkgs; [
            pkg-config
            openssl
          ];
        in
        {
          devShells.default = pkgs.mkShell {
            inherit nativeBuildInputs;
          };
          packages.default = pkgs.rustPlatform.buildRustPackage {
            pname = "mhws-tex-decompressor";
            version = "v0.3.0";
            cargoLock = {
              lockFile = ./Cargo.lock;
              outputHashes = {
                "re-tex-0.2.0" = "sha256-n5uOx5ZIaaAHLq/Yqcoj+EFoz1pkQHK+k4xUkEX3CsE=";
                "ree-pak-core-0.5.0" = "sha256-xhx7EClDmvCVzyWamkVVR3o5988oCYtbcBCYK5BnE5I=";
              };
            };
            src = ./.;
            inherit nativeBuildInputs;
            PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig";
          };
        };
    };
}
