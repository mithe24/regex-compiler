{
  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
    }:
    flake-utils.lib.eachSystem nixpkgs.lib.systems.flakeExposed (
      system:
      let
        pkgs = import nixpkgs { inherit system; };
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = [
            pkgs.rustc
            pkgs.rustfmt
            pkgs.clippy
            pkgs.cargo
            pkgs.graphviz
          ];
        };
      }
    );
}
