{ buildPythonPackage
, lib
, whoosh
, telethon
, jieba
, python-socks
, pyyaml
, redis
, cryptg
}:

buildPythonPackage {
  version = lib.removeSuffix "\n" (builtins.readFile ../__version__);
  pname = "tg-searcher";
  src = builtins.path { path = ./..; name = "tg-searcher"; };
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

