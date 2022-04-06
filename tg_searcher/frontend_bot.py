import html
from time import time
from typing import Optional, List, Tuple, Set, Union
from traceback import format_exc
from argparse import ArgumentParser
import shlex

import whoosh.index
from telethon import TelegramClient, events, Button
from telethon.tl.types import BotCommand, BotCommandScopePeer, BotCommandScopeDefault
from telethon.tl.custom import Message as TgMessage
from telethon.tl.functions.bots import SetBotCommandsRequest
from redis import Redis
from redis.exceptions import ConnectionError as RedisConnectionError

from .common import CommonBotConfig, get_logger, get_share_id, remove_first_word
from .backend_bot import BackendBot, EntityNotFoundError
from .indexer import SearchResult


class BotFrontendConfig:
    @staticmethod
    def _parse_redis_cfg(redis_cfg: str) -> Tuple[str, int]:
        colon_idx = redis_cfg.index(':')
        if colon_idx < 0:
            raise ValueError("No colon in redis host config")
        return redis_cfg[:colon_idx], int(redis_cfg[colon_idx + 1:])

    def __init__(self, **kw):
        self.bot_token: str = kw['bot_token']
        self.admin: Union[int, str] = kw['admin_id']
        self.page_len: int = kw.get('page_len', 10)
        self.redis_host: Tuple[str, int] = self._parse_redis_cfg(kw.get('redis', 'localhost:6379'))

        self.private_mode: bool = kw.get('private_mode', False)
        self.private_whitelist: Set[int] = set(kw.get('private_whitelist', []))
        self.private_whitelist.add(self.admin)


class BotFrontend:
    """
    Redis data protocol:
    - {frontend_id}:query_text:{bot_chat_id}:{msg_id} => query text corresponding to a search result
    - {frontend_id}:query_chats:{bot_chat_id}:{msg_id} => chat filter corresponding to a search result
    - {frontend_id}:select_chat:{bot_chat_id}:{msg_id} => the chat_id selected

    Button data protocol:
    - select_chat={chat_id}
    - search_page={page_number}
    """

    def __init__(self, common_cfg: CommonBotConfig, cfg: BotFrontendConfig, frontend_id: str, backend: BackendBot):
        self.backend = backend
        self.id = frontend_id
        self.bot = TelegramClient(
            str(common_cfg.session_dir / f'frontend_{self.id}.session'),
            api_id=common_cfg.api_id,
            api_hash=common_cfg.api_hash,
            proxy=common_cfg.proxy
        )
        self._cfg = cfg
        self._redis = Redis(host=cfg.redis_host[0], port=cfg.redis_host[1], decode_responses=True)
        self._logger = get_logger(f'bot-frontend:{frontend_id}')
        self._admin = None  # to be initialized in start()
        self.username = None

        self.download_arg_parser = ArgumentParser()
        self.download_arg_parser.add_argument('--min', type=int)
        self.download_arg_parser.add_argument('--max', type=int)
        self.download_arg_parser.add_argument('chats', type=str, nargs='*')

        self.chat_ids_parser = ArgumentParser()
        self.chat_ids_parser.add_argument('chats', type=str, nargs='*')

    async def start(self):
        self._admin = await self.backend.str_to_chat_id(self._cfg.admin)
        try:
            self._redis.ping()
        except RedisConnectionError as e:
            self._logger.critical(f'Cannot connect to redis server {self._cfg.redis_host}: {e}')
            exit(1)

        self._logger.info(f'Start init frontend bot')
        self._logger.info(f'Start login to bot')
        await self.bot.start(bot_token=self._cfg.bot_token)
        self.username = (await self.bot.get_me()).username
        self._logger.info(f'Bot (@{self.username}) account login ok')
        await self._register_commands()
        self._logger.info(f'Register bot commands ok')
        self._register_hooks()

        # prevent chat with bot being indexed
        self.backend.excluded_chats.add((await self.bot.get_me()).id)

        sb = ['bot 初始化完成\n\n', await self.backend.get_index_status()]
        # TODO: pass structured status message from backend
        await self.bot.send_message(self._admin, ''.join(sb), parse_mode='html')

    async def _callback_handler(self, event: events.CallbackQuery.Event):
        self._logger.info(f'Callback query ({event.message_id}) from {event.chat_id}, data={event.data}')
        if event.data:
            data = event.data.decode('utf-8').split('=')
            if data[0] == 'search_page':
                page_num = int(data[1])
                q = self._redis.get(f'{self.id}:query_text:{event.chat_id}:{event.message_id}')
                chats = self._redis.get(f'{self.id}:query_chats:{event.chat_id}:{event.message_id}')
                chats = chats and [int(chat_id) for chat_id in chats.split(',')]
                self._logger.info(f'Query [{q}] (chats={chats}) turned to page {page_num}')
                if q:
                    start_time = time()
                    result = self.backend.search(q, chats, self._cfg.page_len, page_num)
                    used_time = time() - start_time
                    response = await self._render_response_text(result, used_time)
                    buttons = self._render_respond_buttons(result, page_num)
                    await event.edit(response, parse_mode='html', buttons=buttons)
            elif data[0] == 'select_chat':
                chat_id = int(data[1])
                chat_name = await self.backend.translate_chat_id(chat_id)
                await event.edit(f'回复本条消息以对 {chat_name} ({chat_id}) 进行操作')
                self._redis.set(f'{self.id}:select_chat:{event.chat_id}:{event.message_id}', chat_id)
            else:
                raise RuntimeError(f'unknown callback data: {event.data}')
        await event.answer()

    async def _normal_msg_handler(self, event: events.NewMessage.Event):
        text: str = event.raw_text.strip()
        self._logger.info(f'User {(await event.message.get_sender()).id} (in {event.chat_id}) sends "{text}"')

        if not text or text.startswith('/start'):
            return

        elif text.startswith('/random'):
            # TODO: support random msg in a given chat
            # TODO: show message brief
            try:
                msg = self.backend.rand_msg()
                chat_name = await self.backend.translate_chat_id(msg.chat_id)
                respond = f'随机消息: <b>{chat_name} [{msg.post_time}]</b>\n'
                respond += f'{msg.url}\n'
            except IndexError:
                respond = '错误：索引为空'
            await event.respond(respond, parse_mode='html')

        elif text.startswith('/chats'):
            # TODO: support paging
            buttons = []
            kw = remove_first_word(text)
            for chat_id in self.backend.monitored_chats:
                chat_name = await self.backend.translate_chat_id(chat_id)
                if kw in chat_name:
                    buttons.append([Button.inline(f'{chat_name} ({chat_id})', f'select_chat={chat_id}')])
            await event.respond('选择一个聊天', buttons=buttons)

        elif text.startswith('/search'):
            await self._search(event)

        elif text.startswith('/'):
            await event.respond(f'错误：未知命令 {text.split()[0]}')

        else:
            await self._search(event)

    async def _chat_ids_from_args(self, chats: List[str]) -> List[int]:
        return [await self.backend.str_to_chat_id(chat) for chat in chats]

    async def _admin_msg_handler(self, event: events.NewMessage.Event):
        text: str = event.raw_text.strip()
        self._logger.info(f'Admin {event.chat_id} sends "{text}"')
        if text.startswith('/stat'):
            await event.respond(await self.backend.get_index_status(), parse_mode='html')

        elif text.startswith('/download_chat'):
            args = self.download_arg_parser.parse_args(shlex.split(text)[1:])
            min_id = args.min or 1
            max_id = args.max or 1 << 31 - 1
            chat_ids = await self._chat_ids_from_args(args.chats) or self._query_selected_chat(event)
            if not chat_ids:
                await event.reply(f'错误：请至少指定一个会话')
                return
            for chat_id in chat_ids:
                self._logger.info(f'start downloading history of {chat_id} (min={min_id}, max={max_id})')
                await self._download_history(event, chat_id, min_id, max_id)
                self._logger.info(f'succeed downloading history of {chat_id} (min={min_id}, max={max_id})')

        elif text.startswith('/monitor_chat'):
            args = self.chat_ids_parser.parse_args(shlex.split(text)[1:])
            chat_ids = await self._chat_ids_from_args(args.chats) or self._query_selected_chat(event)
            if not chat_ids:
                await event.reply(f'错误：请至少指定一个会话')
                return
            for chat_id in chat_ids:
                self._logger.info(f'add {chat_id} to monitored_chat')
                self.backend.monitored_chats.add(chat_id)
                chat_html = self.backend.format_dialog_html(chat_id)
                await event.reply(f'{chat_html} 已被加入监听列表', parse_mode='html')

        elif text.startswith('/clear'):
            args = self.chat_ids_parser.parse_args(shlex.split(text)[1:])
            chat_ids = await self._chat_ids_from_args(args.chats) or self._query_selected_chat(event)
            self._logger.info(f'clear downloading history of chats {chat_ids}')
            self.backend.clear(chat_ids)
            if chat_ids:
                for chat_id in chat_ids:
                    await event.reply(f'{await self.backend.format_dialog_html(chat_id)} 的索引已清除', parse_mode='html')
            else:
                await event.reply('全部索引已清除')

        elif text.startswith('/refresh_chat_names'):
            msg = await event.reply(f'正在刷新后端的对话名称缓存')
            await self.backend.session.refresh_translate_table()
            await msg.edit(msg, f'对话名称缓存刷新完成')

        elif text.startswith('/find_chat_id'):
            q = text[14:].strip()
            if len(q) == 0:
                await event.reply('错误：关键词不能为空')
                return
            chat_ids = await self.backend.find_chat_id(q)
            sb = []
            for chat_id in chat_ids[0:50]:  # avoid too many chats included
                chat_name = await self.backend.translate_chat_id(chat_id)
                sb.append(f'{html.escape(chat_name)}: <pre>{chat_id}</pre>\n')
            result_text = ''.join(sb) if len(sb) > 0 else f'未找到标题中包含 "{q}" 的对话'
            await event.reply(result_text, parse_mode='html')

        else:
            await self._normal_msg_handler(event)

    async def _search(self, event: events.NewMessage.Event):
        print('start search')
        if self.backend.is_empty():
            await event.reply('当前索引为空，请先 /download_history 建立索引')
            return
        start_time = time()
        q: str = event.raw_text
        if q.startswith('/'):
            first_space = q.index(' ')
            q = q[first_space+1:]
        chats = self._query_selected_chat(event)

        self._logger.info(f'Search "{q}" in chats {chats}')
        result = self.backend.search(q, in_chats=chats, page_len=self._cfg.page_len, page_num=1)

        used_time = time() - start_time
        respond = await self._render_response_text(result, used_time)
        buttons = self._render_respond_buttons(result, 1)
        msg: TgMessage = await event.respond(respond, parse_mode='html', buttons=buttons)

        self._redis.set(f'{self.id}:query_text:{event.chat_id}:{msg.id}', q)
        if chats:
            self._redis.set(f'{self.id}:query_chats:{event.chat_id}:{msg.id}', ','.join(map(str, chats)))

    async def _download_history(self, event: events.NewMessage.Event, chat_id: int, min_id: int, max_id: int):
        chat_html = await self.backend.format_dialog_html(chat_id)
        if min_id == 1 and max_id == 1 << 31 - 1 and not self.backend.is_empty(chat_id):
            # TODO: automatically handle message duplication
            await event.reply(
                f'错误: {chat_html} 的索引非空，下载历史会导致索引重复消息，'
                f'请先 /clear 清除索引，或者通过 --min, --max 参数指定索引范围',
                parse_mode='html')
            return
        cnt: int = 0
        prog_msg: Optional[TgMessage] = None

        async def call_back(msg_id):
            nonlocal prog_msg, cnt
            remaining_msg_cnt = msg_id - min_id

            if cnt % 100 == 0:
                prog_text = f'{chat_html}: 还需下载大约 {remaining_msg_cnt} 条消息'
                if prog_msg is not None:
                    await prog_msg.edit(prog_text, parse_mode='html')
                else:
                    prog_msg = await event.reply(prog_text, parse_mode='html')
            cnt += 1

        await self.backend.download_history(chat_id, min_id, max_id, call_back)
        await event.reply(f'{chat_html} 下载完成，共计 {cnt} 条消息', parse_mode='html')
        await prog_msg.delete()

    def _register_hooks(self):
        @self.bot.on(events.CallbackQuery())
        async def callback_query_handler(event: events.CallbackQuery.Event):
            await self._callback_handler(event)

        @self.bot.on(events.NewMessage())
        async def bot_message_handler(event: events.NewMessage.Event):
            sender = await event.message.get_sender()
            # when in group, ignore messages that are neither mentioning nor replying
            if event.is_channel and not event.message.mentioned and f'@{self.username}' not in event.raw_text:
                return
            if sender.is_self:
                return
            if self._cfg.private_mode \
                    and sender.id not in self._cfg.private_whitelist \
                    and get_share_id(event.chat_id) not in self._cfg.private_whitelist:
                await event.reply(f'由于隐私设置，您无法使用本 bot')
                return
            if event.chat_id != self._admin:
                try:
                    await self._normal_msg_handler(event)
                except whoosh.index.LockError:
                    await event.reply(f'当前索引正在被写入，请等待现有写入操作完成')
                except EntityNotFoundError as e:
                    await event.reply(f'未找到 id 为 "{e.entity}" 的对话或用户')
                except Exception as e:
                    await event.reply(f'错误: {e}\n\n请联系管理员修复')
                    raise e
            else:
                try:
                    await self._admin_msg_handler(event)
                except EntityNotFoundError as e:
                    await event.reply(f'未找到 id 为 "{e.entity}" 的对话或用户')
                except Exception as e:
                    await event.reply(f'错误:\n\n<pre>{html.escape(format_exc())}</pre>', parse_mode='html')
                    raise e

    def _query_selected_chat(self, event: events.NewMessage.Event) -> Optional[List[int]]:
        msg: TgMessage = event.message
        if msg.reply_to:
            redis_query_result = self._redis.get(
                f'{self.id}:select_chat:{event.chat_id}:{msg.reply_to.reply_to_msg_id}'
            )
            if redis_query_result:
                return [int(redis_query_result)]
        return None

    async def _register_commands(self):
        admin_input_peer = None  # make IDE happy!
        try:
            admin_input_peer = await self.bot.get_input_entity(self._cfg.admin)
        except ValueError as e:
            self._logger.critical(
                f'Admin ID {self._cfg.admin} is invalid, or you have not had any conversation with '
                f'the bot yet. Please send a "/start" to the bot and retry. Exiting...', exc_info=e)
            exit(-1)

        admin_commands = [
            BotCommand(command="download_chat", description='[--min=MIN] [--max=MAX] [CHAT...] '
                                                            '下载并索引会话的历史消息，并将其加入监听列表'),
            BotCommand(command="monitor_chat", description='CHAT... 将会话加入监听列表'),
            BotCommand(command="stat", description='查询后端索引状态'),
            BotCommand(command="clear", description='[CHAT...] 清除索引'),
            BotCommand(command="find_chat_id", description='KEYWORD 根据关键词获取聊天 id'),
            BotCommand(command="refresh_chat_names", description='刷新对话名称缓存'),
        ]
        commands = [
            BotCommand(command="random", description='随机返回一条已索引消息'),
            BotCommand(command="chats", description='选择对话'),
            BotCommand(command="search", description='搜索消息'),
        ]
        await self.bot(
            SetBotCommandsRequest(
                scope=BotCommandScopePeer(admin_input_peer),
                lang_code='',
                commands=admin_commands + commands
            )
        )
        await self.bot(
            SetBotCommandsRequest(
                scope=BotCommandScopeDefault(),
                lang_code='',
                commands=commands
            )
        )

    async def _render_response_text(self, result: SearchResult, used_time: float):
        string_builder = [f'共搜索到 {result.total_results} 个结果，用时 {used_time: .3} 秒：\n\n']
        for hit in result.hits:
            chat_title = await self.backend.translate_chat_id(hit.msg.chat_id)
            if len(hit.msg.sender) > 0:
                string_builder.append(f'<b>{chat_title} (<u>{hit.msg.sender}</u>) [{hit.msg.post_time}]</b>\n')
            else:
                string_builder.append(f'<b>{chat_title} [{hit.msg.post_time}]</b>\n')
            string_builder.append(f'<a href="{hit.msg.url}">{hit.highlighted}</a>\n')
        return ''.join(string_builder)

    def _render_respond_buttons(self, result, cur_page_num):
        former_page, former_text = (None, ' ') \
            if cur_page_num == 1 \
            else (f'search_page={cur_page_num - 1}', '上一页⬅️')
        next_page, next_text = (None, ' ') \
            if result.is_last_page \
            else (f'search_page={cur_page_num + 1}', '➡️下一页')
        total_pages = - (- result.total_results // self._cfg.page_len)  # use floor to simulate ceil function
        return [
            [
                Button.inline(former_text, former_page),
                Button.inline(f'{cur_page_num} / {total_pages}', None),
                Button.inline(next_text, next_page),
            ]
        ]
