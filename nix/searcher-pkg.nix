{ python3 }:

let
  py = python3.pkgs;
in
py.buildPythonApplication {
  version = "0.1.2";
  pname = "tg-searcher";
  src = builtins.path { path = ./..; name = "tg-searcher"; };
  propagatedBuildInputs = with py; [
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

