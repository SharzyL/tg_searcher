{
  description = "Tg searcher: a searcher framework for Telegram";

  inputs = {
    nixpkgs.url = "nixpkgs";
    flake-utils.url = "flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    let
      overlay = final: prev: {
        python3 = prev.python3.override {
          packageOverrides = pfinal: pprev: {
            telethon = pprev.telethon.overridePythonAttrs (oldAttrs: rec {
              version = "1.32.1";
              src = final.fetchFromGitHub {
                owner = "LonamiWebs";
                repo = "Telethon";
                rev = "refs/tags/v${version}";
                hash = "sha256-0477SxYRVqRnCDPsu+q9zxejCnKVj+qa5DmH0VHuJyI=";
              };
              doCheck = false;
            });

            tg-searcher = pfinal.callPackage ./nix/searcher-pkg.nix { };
          };
        };
        tg-searcher = final.python3Packages.tg-searcher;
        python3Packages = final.python3.pkgs;
      };
    in
    flake-utils.lib.eachDefaultSystem
      (system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ overlay ];
          };
        in
        rec {
          defaultPackage = pkgs.tg-searcher;
          legacyPackages = pkgs;
          devShell = pkgs.mkShell {
            buildInputs = [ defaultPackage ];
          };

          defaultApp = flake-utils.lib.mkApp { drv = defaultPackage; };
        }
      )
    // {
      overlays.default = overlay;
      nixosModules.default = {
        nixpkgs.overlays = [ overlay ];
        imports = [ ./nix/searcher-service.nix ];
      };
    };
}

