{
  description = "Tg searcher: a searcher framework for Telegram";

  inputs = {
    nixpkgs.url = "nixpkgs";
    flake-parts.url = "flake-parts";
    treefmt-nix = {
      url = "github:numtide/treefmt-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { flake-parts, ... }@inputs:
    let
      name = "tg_searcher";
      makePkg = import ./nix/searcher-pkg.nix;

      shellOverride = pkgs: oldAttrs: {
        name = "${name}-dev-shell";
        version = null;
        src = null;
        nativeBuildInputs = (oldAttrs.nativeBuildInputs or [ ]) ++ (with pkgs; [
          uv
          ty
          ruff
        ]);
      };
      overlay = final: _: {
        ${name} = final.python3Packages.callPackage makePkg { };
      };

    in
    # flake-parts boilerplate
    flake-parts.lib.mkFlake { inherit inputs; } {
      imports = [
        inputs.treefmt-nix.flakeModule
      ];

      flake.overlays.default = overlay;
      flake.nixosModules.default = import ./nix/searcher-service.nix;

      systems = inputs.nixpkgs.lib.systems.flakeExposed;

      perSystem = { system, config, pkgs, ... }: {
        packages.default = config.legacyPackages.${name};
        packages.${name} = config.packages.default;
        devShells.default = config.packages.default.overrideAttrs (shellOverride pkgs);
        legacyPackages = pkgs;

        _module.args.pkgs = import inputs.nixpkgs {
          inherit system;
          overlays = [ overlay ];
        };

        treefmt = {
          programs.ruff-format.enable = true;
          programs.mypy = {
            enable = true;
            directories.".".extraPythonPackages = config.packages.default.propagatedBuildInputs;
          };
          programs.nixpkgs-fmt.enable = true;
        };
      };
    };
}

