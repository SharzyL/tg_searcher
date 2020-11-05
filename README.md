# TG Searcher

众所周知，Telegram 的搜索功能较弱，尤其是对于中文等 CJK 语言，由于 Telegram 无法对其进行正确的分词，因此很难搜到想要的内容。[EYHN/telegram-search](https://github.com/EYHN/telegram-search) 是一个基于 Elasticsearch 的工具，能够通过 bot 来提供较为理想的搜索服务，但是由于 Elasticsearch 不够轻量化，在配置较差的服务器上性能不理想，因此这里基于上述工具的代码进行了修改，使用了 [whoosh](https://whoosh.readthedocs.io) 这个纯 python 的全文搜索库来提供搜索功能，从而比较轻便地运行。

## 示例

[@sharzy_search_bot](https://t.me/sharzy_search_bot) 是一个部署的示例，它给 [@sharzy_talk](https://t.me/sharzy_talk) 提供搜索服务。发送任意文字，bot 即会返回搜索到的内容，并按照相关性排序。可以直接点击链接跳转。

![](https://p.sda1.dev/0/c0f19f7cab2aa58879e716e3f1cec538/image.png)

## 部署和使用

### 准备

在[这里](https://my.telegram.org/apps)申请一个 Telegram App，并获得一组 `api_id` （形如 `1430968`）和 `api_hash`（形如 `07689061c27182818012e05c1987a998`）。

在 [@Bot Father](https://t.me/BotFather) 中注册一个 bot，获得一个 `token`（形如 `1023456789:AbCd44534523241-dsSD324ljkjldsafgfdgf4dD`）。

向 [@getidsbot](https://t.me/getidsbot) 发送一条消息，获得自己的用户 `id`（形如 `629321234`） 。对于需要提供搜索服务的频道 / 聊天，向 [@get_id_info_bot](https://t.me/get_id_info_bot) 转发这个聊天的一条消息，获得这个聊天的 `id`（形如 `-1001439046799`）。

安装 Redis 并运行（可以按照[这里](https://redis.io/topics/quickstart)的操作指示）。

确保 python 版本在 3.7 或以上。

### 运行

将仓库代码克隆到服务器上，参考 `searcher.yaml.example` 文件填写配置文件，并将其重命名为 `searcher.yaml.example`；安装相关的 python 库。

```shell script
git clone https://github.com/SharzyL/tg_searcher.git
cd tg_searcher
pip install telethon pyyaml whoosh jieba redis
```

运行 `python main.py` ，首次运行时需要使用自己的账号信息登录。运行成功后 bot 会在 Telegram 中发送一条 `I am ready` 消息。

bot 不会自动下载历史消息，需要使用管理员帐号向上面填写的账号向 bot 发送 `/download_history` 。之后发送 / 删除 / 修改消息时，bot 会自行进行对应的操作，无需干预。

## 说明

在这个 bot 中我们使用了 [jieba](https://github.com/fxsjy/jieba) 库提供的中文分词，使用了 [whoosh](https://whoosh.readthedocs.io) 的默认算法，也支持 whoosh 自带的[高级搜索功能](https://whoosh.readthedocs.io/en/latest/querylang.html)。

在运行时可以传入 `-c` 参数，从而可以清空之前记录的消息（即清除建立的索引）。如果传入 `-f /path/to/yaml` 参数，bot 会读取 `/path/to/yaml` 位置的配置文件，默认的配置文件目录为 `./searcher.yaml`。

