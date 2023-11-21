{
  inputs = {
    naersk.url = "github:nix-community/naersk/master";
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, utils, naersk }:
    utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        naersk-lib = pkgs.callPackage naersk { };
        nativeBuildInputs = with pkgs; [
          gtk4
          cairo
          glib
          pkg-config
          poppler
          wrapGAppsHook
        ];
      in {
        defaultPackage = naersk-lib.buildPackage {
          src = ./.;
          inherit nativeBuildInputs;
        };

        devShell = with pkgs;
          mkShell {
            inherit nativeBuildInputs;
            buildInputs = [
              cargo
              rustc
              rustfmt
              rust-analyzer
              pre-commit
              rustPackages.clippy
              cargo-outdated
              cargo-audit
            ];
            RUST_SRC_PATH = rustPlatform.rustLibSrc;
          };
      });
}
