{
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
      makePkg = { lib, rustPlatform, rustc, cargo, runCommand }:
        rustPlatform.buildRustPackage {
          inherit name;
          src = with lib.fileset; toSource {
            root = ./.;
            fileset = fileFilter
              (file: ! (lib.elem file.name [ "flake.nix" "flake.lock" ]))
              ./.;
          };

          # for rust-rover usage
          passthru.toolchain = runCommand "rust-toolchain" { } ''
            mkdir -p $out/{bin,lib}
            ln -s ${rustc}/bin/rustc $out/bin/
            ln -s ${cargo}/bin/cargo $out/bin/
            ln -s ${rustPlatform.rustLibSrc} $out/src
          '';

          cargoHash = "sha256-bqTOlkW5wZcsqfxa1Hu+DVUs/hBlsI9oaM0umj8NORQ=";
          meta.mainProgram = name;
        };

      shellOverride = pkgs: oldAttrs: {
        name = "${name}-dev-shell";
        version = null;
        src = null;
        nativeBuildInputs = (oldAttrs.nativeBuildInputs or [ ]) ++ (with pkgs; [
          clippy
        ]);
        shellHook = ''
          unset RUST_LOG
        '';
        cargoDeps = pkgs.emptyDirectory;
      };

      overlay = final: _: { ${name} = final.callPackage makePkg { }; };

    in
    # flake-parts boilerplate
    flake-parts.lib.mkFlake { inherit inputs; } {
      imports = [
        inputs.treefmt-nix.flakeModule
      ];

      flake.overlays.default = overlay;

      systems = inputs.nixpkgs.lib.systems.flakeExposed;

      perSystem = { system, config, pkgs, ... }: {
        packages.default = config.legacyPackages.${name};
        packages.${name} = config.packages.default;
        legacyPackages = pkgs;

        _module.args.pkgs = import inputs.nixpkgs {
          inherit system;
          overlays = [ overlay ];
        };

        devShells.default = config.packages.default.overrideAttrs (shellOverride pkgs);

        treefmt = {
          programs.rustfmt.enable = true;
          programs.nixpkgs-fmt.enable = true;
        };
      };
    };
}
