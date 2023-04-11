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
      in
      {
        packages = rec {
          invogen = naersk-lib.buildPackage {
            src = ./.;
            nativeBuildInputs = [ pkgs.openssl ];
          };
          default = invogen;
        };
        devShells.default = with pkgs; mkShell {
          nativeBuildInputs = [
            cargo
            cargo-outdated
            rustc
            rustfmt
            rust-analyzer
            pre-commit
            openssl
            rustPackages.clippy
          ];
          RUST_SRC_PATH = rustPlatform.rustLibSrc;
        };
      });
}
