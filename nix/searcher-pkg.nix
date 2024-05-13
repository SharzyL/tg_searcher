{ buildPythonPackage
, fetchFromGitHub
, lib
, pdm-backend

, whoosh
, telethon
, jieba
, python-socks
, pyyaml
, redis
, cryptg
}:

let
  telethon_1_35 = telethon.overridePythonAttrs (oldAttrs: rec {
    version = "1.35.1";
    src = fetchFromGitHub {
      owner = "LonamiWebs";
      repo = "Telethon";
      rev = "refs/tags/v${version}";
      hash = "sha256-expJdVvR8yxVC1e+v/hH81TKZ1HJceWBv6BqD15aOFU=";
    };
    doCheck = false;
  });
in
buildPythonPackage {
  version = lib.head
    (builtins.match ".*__version__ = \"([0-9.]+)\".*"
      (builtins.readFile ./../tg_searcher/__init__.py));

  pyproject = true;
  nativeBuildInputs = [ pdm-backend ];

  pname = "tg-searcher";

  src = with lib.fileset; toSource {
    root = ./..;
    fileset = fileFilter (file: file.name != "flake.nix" && file.name != "nix") ./..;
  };

  propagatedBuildInputs = [
    whoosh
    telethon_1_35
    jieba
    python-socks
    pyyaml
    redis
    cryptg
  ];

  doCheck = false; # since we have no test
}

