{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.05";
    #flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
      in
      {
        packages.default = pkgs.rustPlatform.buildRustPackage rec {
          pname = "hashcards";
          version = "0.1.0";
          src = ./.;
          cargoLock = {
            lockFile = ./Cargo.lock;
            # If cargo.lock has git deps, check their hash
            # automatically :
            allowBuiltinFetchGit = true;
          };
          #RUSTFLAGS = "-C target-feature=+crt-static";
          buildInputs = with pkgs; [
            #glibc.static
          ];
        };
        #devShells.default = pkgs.mkShell {};
      });
}
