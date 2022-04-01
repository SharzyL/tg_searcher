# TG Searcher

众所周知，Telegram 的搜索功能较弱，尤其是对于中文等 CJK 语言，由于 Telegram 无法对其进行正确的分词，因此很难搜到想要的内容。本项目实现了一个通用的框架，用户可以对 Telegram 的会话建立索引，通过 bot 来便捷地搜索消息。

它的使用效果见下图：

<img src="./assets/example.jpg" alt="使用示例" height="500">

## 配置指南

Searcher 分为前端和后端两个部分。后端部分是一个 userbot，使用一个普通用户的账号，负责获取 Telegram 中会话的消息，并且将其存入硬盘中的索引。Userbot 和 Telegram 的登录记录被称为 session。

前端负责处理和用户的交互，它可以有多种实现，目前实现了 Telegram bot 的前端。用户通过和这个 bot 账号对话来和后端进行交互，一般的用户可以通过 bot 来搜索消息；管理员除了可以用来搜索消息之外，还可以用它来管理后端的数据。

Searcher 使用 YAML 作为配置文件的格式，默认的配置文件位于 `./searcher.yaml`，用户可以通过命令行参数指定其它的配置文件位置。

在填写配置文件之前，有下面几项准备工作：

1. 在 [my.telegram.org](https://my.telegram.org) 申请一对 `api_id` 和 `api_hash`
2. 如果使用 bot 前端，需要向 [BotFather](https://t.me/BotFather) 申请一个 bot 账号，获取它的 `bot_token`，为了确保管理员能收到 bot 发来的消息，申请之后给 bot 发送一条任意的消息。
3. 找到管理员的用户 ID，可以通过向 [GetIDs Bot](https://t.me/getidsbot) 发送任意消息来获取自己的用户 ID。

以下是一个最简单的配置文件，注意其中的各个 id 需要修改成用户自己的对应 id：

```yaml
common:
  name: sharzy_test
  runtime_dir: /var/lib/tg_searcher
  api_id: 1234567
  api_hash: 17a89121c4347182b112e15c1517a998

sessions:
  - name: alice
    phone: '+18352436375'

backends:
  - id: pub_index
    use_session: alice

frontends:
  - type: bot
    id: public
    use_backend: pub_idx
    config:
      admin_id: 619376577
      bot_token: 1200617810:CAF930aE75Vbac02K34tR-A8abzZP4uAq98
```

以下是一个完整的配置文件，包含了所有的可配置项和对应的注释。

```yaml
common:
  # 当前 Searcher 实例的名称，防止部署多个实例的时候文件冲突
  name: sharzy_test

  # 运行时存储索引文件、session 文件等的位置，多个实例可以使用相同的位置
  runtime_dir: /var/lib/tg_searcher

  # 用于访问 Telegram 的代理，支持 socks5 和 http 协议，如不需要可以去掉该行
  proxy: socks5://localhost:1080

  api_id: 1234567
  api_hash: 17a89121c4347182b112e15c1517a998

sessions:
  - name: alice             # 用来标识 session 的名称，在配置文件中唯一即可
    phone: '+18352436375'   # 用户的电话号码

backends:
  - id: pub_index           # 用来标识后端的名称，在配置文件中唯一即可
    use_session: alice      # 后端所使用的 session 的名称

  - id: priv_idx
    use_session: alice
    config:
      monitor_all: true     # 当启用这一选项的时候，所有的会话均会被监听，新消息全部会被加入索引
      excluded_chats:       # 当 monitor_all 选项启用的时候，这个列表里的会话不会被监听
        - 342843148
        - 857204339

frontends:
  - type: bot               # 目前只支持 bot 类型的前端
    id: public
    use_backend: pub_idx    # 使用的后端的名称
    config:
      admin_id: 619376577   # 管理员的用户 ID
      bot_token: 1200617810:CAF930aE75Vbac02K34tR-A8abzZP4uAq98
      page_len: 10          # 搜索时每页显示的结果数量，默认为 10
      redis: localhost:6379 # Redis 服务器的地址，默认为 localhost:6379

  - type: bot
    id: private
    use_backend: priv_idx
    config:
      admin_id: 619376577
      # 不同前端应该使用不同的 bot_token
      bot_token: 2203317382:BkF390ab92kcb1b2ii2b4-1sbc39i20bb12
      # 如果开启了 private_mode，那么只有 private_whitelist 里的用户才能使用 bot
      # 管理员默认位于 private_whitelist 中，无需额外添加
      private_mode: true
      private_whitelist:
        - 719376577
```

关于如何运行和部署 Searcher，参见 [DEPLOY.md](./DEPLOY.md)

关于不同版本的修改记录，参见 [CHANGELOG.md](./CHANGELOG.md)

## Telegram Bot 前端

Telegram Bot 的前端提供了如下功能：

当用户给 bot 发送消息的时候，bot 默认会将消息的内容视为搜索的关键词，因此会返回根据这个关键词进行搜索的结果。除了简单的关键词查询以外，还支持如下的高级搜索语法

1. `"foo bar"` 搜索出现 `foo bar` 这个字符串的消息
2. `AND`, `OR`, `NOT` 可以用来组合搜索逻辑，例如 `NOT foo AND (bar OR woo)` 搜索所有没有出现 `foo`，并且出现了 `bar` 或者 `woo` 的消息
3. `*` 和 `?` 通配符，前者匹配任意多个字符，后者匹配一个字符。注意：包含通配符的搜索会较慢。
4. 更详细的语法介绍，参见 whoosh 的[文档](https://whoosh.readthedocs.io/en/latest/querylang.html)

下面几条命令任何用户都可以使用：

1. `/search [keyword]`: 搜索消息，同去掉 `/search` 的情形等价。
2. `/chats [keyword]`: 列出所有被索引的，标题中包含 `keyword` 的会话列表。如果没有指定 `keyword`，则返回所有的会话。bot 会返回一列按钮，点击一个按钮之后这条消息就对应了一个会话。有一些指令包含 `chat_id` 作为参数，对于这些指令，如果没有指定 chat_id 参数，那么可以通过回复这条消息来把会话的 ID 当作参数。如果回复搜索的关键词，则只搜索这个会话中的消息。
3. `/random`：返回一条随机的消息（暂不支持指定会话）

下面几条命令只有管理员可以使用：

1. `/download_history [--max=MAX] [--min=MIN] [CHAT...]`: 下载某个会话的历史消息并将其进行索引。其中 `CHAT` 是对话的 id 或者用户名。如果没有指定 `CHAT` 也没有通过回复 bot 的消息来指定会话。`MIN` 和 `MAX` 参数用于指定索引的消息的最小和最大 ID。在下载之后，会话会自动加入监听列表，所有新消息和旧消息的修改、删除均会被同步到索引
2. `/track_chat CHAT...`: 仅将会话加入监听列表，不索引历史消息。注意：当前版本当后端重启之后，如果一个聊天没有任何消息被索引，那么它不会被加入监听列表，未来的版本可能会修复这个问题。
3. `/stat`: 报告后端的状态
4. `/clear [CHAT...]`: 清除后端的所有索引，或者清除某个特定会话的索引
5. `/find_chat_id KEYWORD`: 列出所有后端的账号的所有会话中包含 `keyword` 的会话以及对应的 id
6. `/refresh_chat_names`: 刷新会话 id 和会话名称之间映射的缓存
