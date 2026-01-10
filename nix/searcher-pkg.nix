{ buildPythonPackage
, lib
, uv-build

, whoosh
, telethon
, jieba
, python-socks
, pyyaml
, redis
, cryptg
}:

buildPythonPackage {
  version = lib.head
    (builtins.match ".*__version__ = '([0-9.]+)'.*"
      (builtins.readFile ./../tg_searcher/__init__.py));

  pyproject = true;
  nativeBuildInputs = [ uv-build ];

  pname = "tg-searcher";

  src = with lib.fileset; toSource {
    root = ./..;
    fileset = fileFilter (file: file.name != "flake.nix" && file.name != "nix") ./..;
  };

  propagatedBuildInputs = [
    whoosh
    telethon
    jieba
    python-socks
    pyyaml
    redis
    cryptg
  ];

  doCheck = false; # since we have no test
}

