{}:
let
  rust-overlay = (import (builtins.fetchTarball "https://github.com/oxalica/rust-overlay/archive/master.tar.gz"));
  pkgs = (import <nixpkgs> {
    overlays = [ rust-overlay ];
  });
in
pkgs.mkShell {
  nativeBuildInputs = [
    pkgs.openssl
    (pkgs.rust-bin.stable.latest.default.override {
      extensions = [
        "rust-src"
      ];
    })
    pkgs.cargo-outdated
    pkgs.rust-analyzer

    # keep this line if you use bash
    pkgs.bashInteractive
  ];
}
