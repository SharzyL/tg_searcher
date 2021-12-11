# TG Searcher

众所周知，Telegram 的搜索功能较弱，尤其是对于中文等 CJK 语言，由于 Telegram 无法对其进行正确的分词，因此很难搜到想要的内容。[EYHN/telegram-search](https://github.com/EYHN/telegram-search) 是一个基于 Elasticsearch 的工具，能够通过 bot 来提供较为理想的搜索服务，但是由于 Elasticsearch 不够轻量化，在配置较差的服务器上性能不理想，因此这里基于上述工具的代码进行了修改，使用了 [whoosh](https://whoosh.readthedocs.io) 这个纯 python 的全文搜索库来提供搜索功能，从而比较轻便地运行。

## 示例

[@sharzy_search_bot](https://t.me/sharzy_search_bot) 是一个部署的示例，它给 [@sharzy_talk](https://t.me/sharzy_talk) 提供搜索服务。发送任意文字，bot 即会返回搜索到的内容，并按照相关性排序。可以直接点击链接跳转。

![](https://p.sda1.dev/0/c0f19f7cab2aa58879e716e3f1cec538/image.png)

## 部署和使用

### 准备

1. 在[这里](https://my.telegram.org/apps)申请一个 Telegram App，并获得一组 `api_id` （形如 `1430968`）和 `api_hash`（形如 `07689061c27182818012e05c1987a998`）。

2. 在 [@Bot Father](https://t.me/BotFather) 中注册一个 bot，获得一个 `token`（形如 `1023456789:AbCd44534523241-dsSD324ljkjldsafgfdgf4dD`）。

3. 本服务需要一个 Telegram 帐号来读取对话中的消息，同时通过这个帐号来管理 bot 的运行。这个帐号称为管理员，我们需要找到这个管理员的 id。方法见[下文](#如何找到对话的-id)介绍。

4. 找出对于需要提供搜索服务的对话（可以是私聊、群组、频道）的 id。方法同上。


### 运行

#### 手动运行

1. 安装 Redis 并运行（可以按照[这里](https://redis.io/topics/quickstart)的操作指示）。

2. 确保 python 版本在 3.7 或以上。

3. 将仓库代码克隆到服务器上，参考 `searcher.yaml.example` 文件填写配置文件，并将其重命名为 `searcher.yaml`；安装相关的 python 库。

```shell script
git clone https://github.com/SharzyL/tg_searcher.git
cd tg_searcher
pip install -r requirement.txt
```

运行 `python main.py` ，首次运行时需要使用自己的账号信息登录。运行成功后 bot 会在 Telegram 中向管理员发送一条 `I am ready` 消息。

bot 不会自动下载历史消息，使用管理员帐号向上面填写的账号向 bot 发送 `/download_history` 可以让 bot 从头开始下载历史消息。之后发送 / 删除 / 修改消息时，bot 会自行更新数据库，无需干预。这个命令可以带有两个可选的参数，分别代表要下载的消息的最大 id 和最小 id。例如，发送 `/download_history 100 500` 可以下载所有 id 在 100 和 500 之间的消息（包括 100 和 500）。如果第二个参数没有指定，那么会下载所有 id 不小于第一个参数的消息。

如果在配置文件中指定 `private_mode: true`，那么除了在 `private_whitelist` 中指定 id 的用户，其它用户无法看到搜索到的消息的内容。如果指定 `random_mode: true`，那么当用户分送 `/random` 指令时，会随机返回一条已索引消息。

#### Docker Compose

##### 初次配置

```shell
mkdir tg_searcher
cd tg_searcher
wget https://raw.githubusercontent.com/Rongronggg9/tg_searcher/master/docker-compose.yaml.sample -O docker-compose.yaml
mkdir config
wget https://raw.githubusercontent.com/Rongronggg9/tg_searcher/master/searcher.yaml.example -O config/searcher.yaml
vi config/searcher.yaml  # 修改 searcher.yaml（见下）
```

需要保证 `searcher.yaml` 中: `redis host`=`redis`, `redis port`=`6379`, `runtime_dir`=`/app/config/tg_searcher_data` ，其余注意事项参考上一节及配置文件中的注释。  
`tg_searcher` 目录将含有 bot 运行所需及产生的所有资讯，谨防泄露。需要迁移时，整个目录迁移即可。

##### 初次运行

```shell
docker-compose up --no-start
docker start tg_searcher_redis
docker start -ia tg_searcher  # 这时你将需要按指引登入账号，一切完成后 Ctrl-P Ctrl-Q 解离
```

完成登入后，为了安全性着想，可以注释掉 `docker-compose.yaml` 里标明的两行（不是必须）。

```shell
docker-compose down  # 先停止运行
vi docker-compose.yaml  # 注释掉标明的两行
```

##### 再次运行

以后需要再次运行时，进入 `tg_searcher` 目录，执行以下命令即可。

```shell
docker-compose up -d
```

##### 升级

以后需要升级时，进入 `tg_searcher` 目录，执行以下命令即可。

```shell
docker-compose down  # 先停止运行
docker-compose pull  # 更新镜像
docker-compose up -d
```

## 如何找到对话的 id

对话的 id 是一个正整数（目前一般不大于 10 位），有的时候你会遇到长达 13 位、以 100 开头的 id，或者带有负号的 id，这些是 Telegram 内部为了区分不同类型的对话而将原来的 id 进行了简单的转换。在本项目中，我们兼容这些 id，你可以在配置文件中使用任何一种 id。唯一的要求是管理员能够访问对应对话的消息。

### 对于某些特定类型的对话

如果对方的隐私设置里面没有禁止带引用的消息转发，那么直接将对方的消息转发到 @getidsbot 这个 bot 即可获得对方的 id。

如果这个对话是频道，那么将频道的消息转发到 @getidsbot 也可以获得 id。

如果这个对话是私有群组，那么在群组的任意一条的右键菜单中点击复制链接 (Copy Message Link) 按钮，可以得到一个类似于 `https://t.me/c/1234567890/154` 的链接，其中的第一个数字（本例中为 1234567890）即为群组的 id。

**注意**：如果消息的右键菜单中没有复制链接的按钮，说明这个群不是超级群，群内的消息没有对应的链接（新建的私有群组默认处于这个状态），这会导致无法通过 bot 搜索结果中的链接跳转到对应的消息。若要将群组升级为超级群，可以打开群组设置，关掉向新成员隐藏历史消息的选项，或者将群组转为公有，在进行这样的操作之后，群组会不可逆地转换为超级群，每条在此之后发送的消息都有对应的链接。

如果你有权限将 bot 拉入群，那么你可以将 @getidsbot 拉入群组，这个 bot 会告诉你群组的 id。在将 bot 拉入群后，对任何用户回复 `/user`，bot 便会告诉你回复的用户的 id。

### 通用的方法

如果上面这些方法都不奏效，你可以尝试下面的这些方法：

一种方法是使用一些第三方客户端，例如 PlusMessager，这一客户端会在用户的 profile 页面显示用户的 id。

另一种方法是运行下面这个脚本（注意修改其中的变量）：`python3 get_id.py some_keyword`，脚本会打印出所有名字里面包含 `some_keyword` 的对话及其 id，如果去掉这一参数，则打印出所有的对话及其 id。

```python
# get_id.py
import asyncio
from telethon import TelegramClient
import sys

api_id = 1430968  # change to your own id
api_hash = '1023456789:AbCd44534523241-dsSD324ljkjldsafgfdgf4dD'  # change to your own hash
proxy = ('socks5', '127.0.0.1', 1080)  # in case you need a proxy
session_file = 'foo.session'  # the path to store your session file

client = TelegramClient(session_file, api_id, api_hash, proxy=proxy)
client.start()

async def iter_diag():
    async for c in client.iter_dialogs(ignore_migrated=True):
        if len(sys.argv) <= 1 or sys.argv[1] in c.name:
            print(f'{c.name} [{c.entity.id}]')

async def main():
    await iter_diag()

asyncio.get_event_loop().run_until_complete(main())
```

## 说明

### 关于消息链接
由于私聊的消息和非超级群的消息均没有链接，因此这些对话我们无法通过 bot 在搜索结果中提供的链接跳转到对应消息。

### 关于搜索语法
在这个 bot 中我们使用了 [jieba](https://github.com/fxsjy/jieba) 库提供的中文分词，使用了 [whoosh](https://whoosh.readthedocs.io) 的默认算法，也支持 whoosh 自带的[高级搜索功能](https://whoosh.readthedocs.io/en/latest/querylang.html)。

### 关于命令行参数
在运行时如果传入 `-c` 参数，则会在清空之前记录的消息（即清除建立的消息索引）。

如果传入 `-f /path/to/yaml` 参数，bot 会读取 `/path/to/yaml` 位置的配置文件，默认的配置文件目录为 `./searcher.yaml`。
