{ pkgs ? import <nixpkgs> {} }:

let
py = pkgs.python3.pkgs;
cryptg = py.buildPythonPackage rec {
  version = "0.2.post4";
  pname = "cryptg";
  src = py.fetchPypi {
    inherit pname version;
    sha256 = "sha256-pN4XMMpWqoqUXxdsJVhpAe1enxX/twxkWe7fRm62KZs=";
  };
  propagatedBuildInputs = with py; [
    pycparser cffi
  ];
};

in
py.buildPythonApplication {
  version = "0.1.2";
  pname = "tg-searcher";
  src = ./.;
  propagatedBuildInputs = with py; [
    whoosh telethon jieba python-socks pyyaml redis cryptg
  ];
  doCheck = false;
}

