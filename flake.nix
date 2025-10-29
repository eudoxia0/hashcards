{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    crane.url = "github:ipetkov/crane";
  };

  outputs = { self, nixpkgs, crane, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        craneLib = crane.mkLib pkgs;
        # Crane filters source files, so that mods to eg README.md don't
        # trigger a rebuild. srcFilter adds css, js and sql files to
        # the filtered source files.
        srcFilter = path: _type: builtins.match ".*css$|.*js$|.*sql$" path != null;
        allSrcFilter = path: type:
          (srcFilter path type) || (craneLib.filterCargoSources path type);

      in
      {
        packages.default = craneLib.buildPackage {
          src = pkgs.lib.cleanSourceWith {
            src = ./.;
            filter = allSrcFilter;
            # Be reproducible, regardless of the directory name :
            name = "source";
          };
          # Do not run cargo tests on build, because cargo takes so
          # long anyway :
          doCheck = false;
          buildInputs = with pkgs; [
            sqlite
            openssl
          ];
          nativeBuildInputs = with pkgs; [
            pkg-config
          ];
        };
      });
}
