{ python3Packages, lib }:

python3Packages.buildPythonApplication {
  version = lib.removeSuffix "\n" (builtins.readFile ../__version__);
  pname = "tg-searcher";
  src = builtins.path { path = ./..; name = "tg-searcher"; };
  propagatedBuildInputs = with python3Packages; [
    whoosh
    telethon
    jieba
    python-socks
    pyyaml
    redis
    cryptg
  ];
  doCheck = false;  # since we have no test
}

