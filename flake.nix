{
  description = "Tg searcher: a searcher framework for Telegram";

  inputs = {
    nixpkgs.url = "nixpkgs";
    flake-utils.url = "flake-utils";
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

          defaultApp = flake-utils.lib.mkApp { drv = searcher-pkg; };
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

