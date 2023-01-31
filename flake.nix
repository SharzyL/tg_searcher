{
  description = "Tg searcher: a searcher framework for Telegram";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem
      (system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          searcher-pkg = pkgs.callPackage ./nix/searcher-pkg.nix { };
        in
        {
          devShell = pkgs.mkShell {
            buildInputs = [ searcher-pkg ];
          };

          defaultApp = searcher-pkg;
          defaultPackage = searcher-pkg;
        }
      )
    // {
      overlays.default = final: prev: {
        tg-searcher = self.defaultPackage.${prev.system};
      };
      nixosModules.default = {
        nixpkgs.overlays = [ self.overlays.default ];
        imports = [ ./nix/searcher-service.nix ];
      };
    };
}

