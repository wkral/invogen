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
            buildInputs = [ pkgs.installShellFiles ];
            nativeBuildInputs = [ pkgs.openssl ];
            postBuild = ''
              installShellCompletion target/release/build/invogen-*/out/invogen.bash
            '';
          };
          default = invogen;
          test = naersk-lib.buildPackage {
            src = ./.;
            mode = "test";
          };
          check = naersk-lib.buildPackage {
            src = ./.;
            mode = "check";
          };
          clippy = naersk-lib.buildPackage {
            src = ./.;
            mode = "clippy";
          };
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
