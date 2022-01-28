# experimental nix packaging

{ pkgs ? import <nixpkgs> {} }:

let
cryptg = pkgs.python3.pkgs.buildPythonPackage rec {
  version = "0.2.post4";
  pname = "cryptg";
  src = pkgs.python3.pkgs.fetchPypi {
    inherit pname version;
    sha256 = "sha256-pN4XMMpWqoqUXxdsJVhpAe1enxX/twxkWe7fRm62KZs=";
  };
  propagatedBuildInputs = with pkgs.python3.pkgs; [
    pycparser cffi
  ];
};

searcher = pkgs.python3.pkgs.buildPythonApplication rec {
  version = "0.1.2";
  pname = "tg-searcher";
  src = pkgs.python3.pkgs.fetchPypi {
    inherit pname version;
    sha256 = "sha256-s4u5c2l9nWMw6ypBGVamRaqp/7k8a0b8NOsUihtWP70=";
  };
  propagatedBuildInputs = with pkgs.python3.pkgs; [
    whoosh telethon jieba python-socks pyyaml redis cryptg
  ];
  doCheck = false;
};

in
pkgs.dockerTools.buildLayeredImage {
  name = "searcher";
  tag = "latest";
  contents = [searcher];
}
