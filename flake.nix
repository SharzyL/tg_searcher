{
  description = "Tg searcher: a searcher framework for Telegram";

  inputs = {
    nixpkgs.url = "nixpkgs";
    flake-utils.url = "flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    let
      overlay = final: prev: {
        tg-searcher = final.python3.pkgs.callPackage ./nix/searcher-pkg.nix { };
      };
    in
    flake-utils.lib.eachDefaultSystem
      (system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ overlay ];
          };
          tg-searcher = pkgs.tg-searcher;
        in
        {
          packages.default = tg-searcher;
          legacyPackages = pkgs;
          devShells.default = tg-searcher;
          apps.default = flake-utils.lib.mkApp { drv = tg-searcher; };
        }
      )
    // {
      overlays.default = overlay;
      nixosModules.default = {
        imports = [ ./nix/searcher-service.nix ];
      };
    };
}

