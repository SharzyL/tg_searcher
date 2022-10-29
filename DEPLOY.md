# 部署

我们提供了两种部署的方法：手动部署，使用 docker-compose 部署和 nix flake 部署。

## 手动运行

1. 安装 Redis 并运行（可以按照[这里](https://redis.io/topics/quickstart)的操作指示）。

2. 确保 python 版本在 3.7 或以上。

```shell script
# install from pip
python3 -m pip install -U tg-searcher

# or install from github
# python3 -m pip install -U git+https://github.com/SharzyL/tg_searcher

# or install locally
# git clone https://github.com/SharzyL/tg_seacher && cd tg_searcher
# python3 -m pip install -e .
```

参考 README 填写配置文件，运行 `python3 -m tg_searcher -f /path/to/config.yaml` 即可。如果 pip 安装可执行文件的目录在 `PATH` 里面，也可以直接 `tg-searcher -f /path/to/config.yaml`。

首次运行时需要填写验证码（如果设置了两步验证，还需填写密码）。运行成功后 bot 会在 Telegram 中向管理员发送一条包含服务器状态的消息。

## Docker Compose

### 初次配置

```shell
mkdir tg_searcher
cd tg_searcher
wget https://raw.githubusercontent.com/SharzyL/tg_searcher/master/docker-compose.sample.yaml -O docker-compose.yaml
mkdir config
vi config/searcher.yaml  # 修改 searcher.yaml（见下）
```

需要保证 `searcher.yaml` 中: `redis: redis:6379`, `runtime_dir: /app/config/tg_searcher_data` ，其余注意事项参考上一节及配置文件中的注释。  
`tg_searcher` 目录将含有 bot 运行所需及产生的所有资讯，谨防泄露。需要迁移时，整个目录迁移即可。

### 代理设置

如果需要使用宿主机上的代理，需要正确配置 `proxy host`:

**Linux**: 使用默认的网络配置的情况下会是 `docker0` 虚拟网卡的 IP，一般是 `172.17.0.1`

```shell
$ ip address

*: docker0: <NO-CARRIER,BROADCAST,MULTICAST,UP> mtu 1500 qdisc noqueue state DOWN group default
    link/ether **:**:**:**:**:** brd ff:ff:ff:ff:ff:ff
    inet 172.17.0.1/16 brd 172.17.255.255 scope global docker0
       valid_lft forever preferred_lft forever
```

**Mac / Windows**: `host.docker.internal`

除了在 tg_searcher 的配置文件中进行配置以外，注意宿主机的代理也需要设置监听这一 IP 地址，具体设置方法视代理客户端而定。

### 初次运行

```shell
docker-compose up --no-start
docker start tg_searcher_redis
docker start -ia tg_searcher  # 这时你将需要按指引登入账号，一切完成后 Ctrl-P Ctrl-Q 解离
```

完成登入后，考虑到安全性，可以注释掉 `docker-compose.yaml` 里标明的两行（不是必须）。

```shell
docker-compose down  # 先停止运行
vi docker-compose.yaml  # 注释掉标明的两行
```

### 再次运行

以后需要再次运行时，进入 `tg_searcher` 目录，执行以下命令即可。

```shell
docker-compose up -d
```

### 升级

以后需要升级时，进入 `tg_searcher` 目录，执行以下命令即可。

```shell
docker-compose down  # 先停止运行
docker-compose pull  # 更新镜像
docker-compose up -d
```

## Nix Flake

假设你正在使用 NixOS，并且启用了 flake，以下是一个部署 `tg_searcher` 的示例:

```nix
# flake.nix
{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixpkgs-unstable";
    searcher = {
      url = "github:SharzyL/tg_searcher";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, searcher }@inputs: {
    nixosConfigurations.holland = let
      system = "x86_64-linux";
    in
    nixpkgs.lib.nixosSystem {
      inherit system;
      specialArgs = inputs;
      modules = [
        {
          nixpkgs.overlays = [
            (final: prev: {
              searcher = searcher.defaultPackage.${system};
            })
          ];
        }
        ./searcher.nix
      ];
    };
  };
}
```

```nix
# searcher.nix
{ config, lib, pkgs, ... }:

{
  systemd.services.searcher = {
    description = "tg searcher";
    after = [ "network.target" "redis-searcher.service" ];
    wantedBy = [ "multi-user.target" ];
    serviceConfig = {
      ExecStart = "${pkgs.searcher}/bin/tg-searcher --config /path/to/config.yaml";
      User = "sharzy";
    };
  };

  services.redis.servers.searcher = {
    enable = true;
    port = 6379;
  };
}
```

使用 `nixos-rebuild switch` 即可部署。 

