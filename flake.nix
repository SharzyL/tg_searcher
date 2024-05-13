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
          pkg = pkgs.tg-searcher;
        in
        {
          packages.default = pkg;

          devShell = pkg.overrideAttrs (oldAttrs: {
            nativeBuildInputs = oldAttrs.nativeBuildInputs ++ [ pkgs.pdm ];
          });

          apps.default = flake-utils.lib.mkApp { drv = pkg; };
        }
      )
    // {
      overlays.default = overlay;
      nixosModules.default = {
        imports = [ ./nix/searcher-service.nix ];
      };
    };
}

