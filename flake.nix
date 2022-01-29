{
  description = "Tg searcher: a searcher framework for Telegram";

  inputs = {
    nixpkgs.url = github:NixOS/nixpkgs/nixos-unstable;
    flake-utils.url = github:numtide/flake-utils;
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem
      (system:
      let 
        pkgs = nixpkgs.legacyPackages.${system}; 
        searcher-pkg = pkgs.callPackage ./searcher.nix {};
      in
        {
          devShell = pkgs.mkShell {
            buildInputs = [ 
              searcher-pkg
              pkgs.python3.pkgs.venvShellHook
            ];
            venvDir = "./venv";
          };

          defaultPackage = searcher-pkg;
        }
      );
}

