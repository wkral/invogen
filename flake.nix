{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    crane.url = "github:ipetkov/crane";
    utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, utils, crane }:
    utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        craneLib = crane.mkLib pkgs;
        texFilter = path: _type: builtins.match ".*tex$" path != null;
        texOrCargo = path: type: (texFilter path type) || (craneLib.filterCargoSources path type);

        src = nixpkgs.lib.cleanSourceWith {
          src = craneLib.path ./.;
          filter = texOrCargo;
        };
        commonArgs = {
          inherit src;
          strictDeps = true;
          buildInputs = [ pkgs.installShellFiles ];
          nativeBuildInputs = [ pkgs.openssl ];
          postBuild = ''
            installShellCompletion target/release/build/invogen-*/out/invogen.bash
          '';
        };
        cargoArtifacts = craneLib.buildDepsOnly commonArgs;
        invogen = craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;
        });
      in
      {
        checks = {
          inherit invogen;
          invogen-clippy = craneLib.cargoClippy (commonArgs // {
            inherit cargoArtifacts;
            cargoClippyExtraArgs = "--all-targets -- --deny warnings";
          });
          invogen-format = craneLib.cargoFmt {
            inherit src;
          };
        };
        packages.default = invogen;
        packages.invogen = invogen;
        devShells.default = craneLib.devShell {
          checks = self.checks.${system};
          packages = [
            pkgs.cargo-outdated
          ];
        };
      });
}
