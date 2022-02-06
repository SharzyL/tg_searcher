---
name: Bug report
about: Create a report to help us improve
title: ''
labels: ''
assignees: ''

---

**Bug 描述**
关于 bug 的简短描述

**复现方式**
复现 bug 的步骤：
1. 使用 tg_searcher 的版本 ...（使用 git log 等工具查看当前的 commit）
2. 使用了配置文件
```yaml
common:
...
frontends:
...
backends:
...
```
3. 使用管理员帐号发送 …… 命令
4. 发生异常：……

**期望行为**
你期望发生的行为

**日志**
```
INFO:telethon.network.mtprotosender:Connecting to 91.108.56.156:443/TcpFull...
INFO:telethon.network.mtprotosender:Connection to 91.108.56.156:443/TcpFull complete!
INFO:session:sharzy:Start iterating dialogs
INFO:session:sharzy:End iterating dialogs, 633 dialogs in total
...
```
