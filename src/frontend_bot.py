import html
from time import time
from typing import Optional
from traceback import format_exc
from argparse import ArgumentParser
import shlex

from telethon import TelegramClient, events, Button
from telethon.tl.types import Message as TgMessage, \
    BotCommand, BotCommandScopePeer, BotCommandScopeDefault
from telethon.tl.functions.bots import SetBotCommandsRequest
from redis import Redis

from common import CommonBotConfig, get_logger
from backend_bot import BackendBot
from indexer import SearchResult


class BotFrontendConfig:
    @staticmethod
    def _parse_redis_cfg(redis_cfg: str) -> tuple[str, int]:
        colon_idx = redis_cfg.index(':')
        if colon_idx < 0:
            raise ValueError("No colon in redis host config")
        return redis_cfg[:colon_idx], int(redis_cfg[colon_idx + 1:])

    def __init__(self, bot_token: str, redis: str, admin_id: int, page_len: int = 10):
        # TODO: private mode
        self.bot_token = bot_token
        self.admin_id = admin_id
        self.page_len = page_len
        self.redis_host: tuple[str, int] = self._parse_redis_cfg(redis)


class BotFrontend:
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
        self._logger = get_logger('bot-frontend')

        self.download_arg_parser = ArgumentParser()
        self.download_arg_parser.add_argument('--min', type=int)
        self.download_arg_parser.add_argument('--max', type=int)
        self.download_arg_parser.add_argument('chats', type=int, nargs='*')

    async def start(self):
        self._logger.info(f'init frontend bot {self.id}')
        await self.bot.start(bot_token=self._cfg.bot_token)
        await self._register_commands()
        self._register_hooks()
        await self.bot.send_message(self._cfg.admin_id, 'I am ready')

    async def _callback_handler(self, event: events.CallbackQuery.Event):
        if event.data and event.data != b'-1':
            page_num = int(event.data)
            q = self._redis.get('msg-' + str(event.message_id) + '-q')
            self._logger.info(f'Query [{q}] turned to page {page_num}')
            if q:
                start_time = time()
                result = self.backend.search(q, None, self._cfg.page_len, page_num)
                used_time = time() - start_time
                respond = await self._render_respond_text(result, used_time)
                buttons = self._render_respond_buttons(result, page_num)
                await event.edit(respond, parse_mode='html', buttons=buttons)
        await event.answer()

    async def _normal_msg_handler(self, event: events.NewMessage.Event):
        text: str = event.message.message
        self._logger.info(f'User {event.chat_id} Queries "{text}"')

        if not (text and text.strip()) or text.startswith('/start'):
            return

        elif text.startswith('/random'):
            msg = self.backend.rand_msg()
            respond = f'Random message from <b>{msg.chat_id} [{msg.post_time}]</b>\n'
            respond += f'{msg.url}\n'
            await event.respond(respond, parse_mode='html')

        elif text.startswith('/'):
            await event.respond(f'错误：未知命令 {text.split()[0]}')

        else:
            await self._search(event)

    async def _admin_msg_handler(self, event: events.NewMessage.Event):
        text: str = event.raw_text
        self._logger.info(f'Admin {event.chat_id} Queries "{text}"')
        if text.startswith('/stat'):
            await event.respond(self.backend.get_stat(), parse_mode='html')

        elif text.startswith('/download_history'):
            download_args = text.split()

            args = self.download_arg_parser.parse_args(shlex.split(text)[1:])
            min_id = args.min or 1
            max_id = args.max or 1 << 31 - 1
            chat_ids = args.chats or self.backend.indexed_chats
            for chat_id in chat_ids:
                await self._download_history(chat_id, min_id, max_id)

        elif text.startswith('/clear'):
            self.backend.clear()
            await event.reply("索引已清除")

        elif text.startswith('/chats'):
            pass

        else:
            await self._normal_msg_handler(event)

    async def _search(self, event: events.NewMessage.Event):
        if self.backend.is_empty():
            await self.bot.send_message(self._cfg.admin_id, '当前索引为空，请先 /download_history 建立索引')
            return
        start_time = time()
        q = event.raw_text
        result = self.backend.search(q, in_chats=None, page_len=self._cfg.page_len, page_num=1)
        used_time = time() - start_time
        respond = await self._render_respond_text(result, used_time)
        buttons = self._render_respond_buttons(result, 1)
        msg = await event.respond(respond, parse_mode='html', buttons=buttons)

        self._redis.set('msg-' + str(msg.id) + '-q', q)

    async def _download_history(self, chat_id: int, min_id: int, max_id: int):
        admin_id = self._cfg.admin_id
        chat_name = await self.backend.translate_chat_id(chat_id)

        if chat_id not in self.backend.indexed_chats:
            await self.bot.send_message(
                admin_id,
                f'警告：重启后端 bot 之后，{chat_name} ({chat_id}) 的索引可能失效，'
                f'请将 {chat_id} 加入配置文件'
            )
        if min_id == 1 and max_id == 1 << 31 - 1 and not self.backend.is_empty(chat_id):
            await self.bot.send_message(
                admin_id,
                f'错误：{chat_name} ({chat_id}) 的索引非空，下载历史会导致索引重复消息，'
                f'请先 /clear 清除索引，或者指定索引范围'
            )
            return
        last_prog_remaining: Optional[int] = None
        prog_msg: Optional[TgMessage] = None

        async def call_back(msg_id):
            nonlocal prog_msg, last_prog_remaining
            remaining_msg_cnt = msg_id - min_id

            if last_prog_remaining is None or remaining_msg_cnt < last_prog_remaining - 100:
                prog_text = f'"{chat_name}" ({chat_id}): 还需下载 {remaining_msg_cnt} 条消息'
                if prog_msg is not None:
                    await self.bot.edit_message(prog_msg, prog_text)
                else:
                    prog_msg = await self.bot.send_message(admin_id, prog_text)
                last_prog_remaining = remaining_msg_cnt

        await self.backend.download_history(chat_id, min_id, max_id, call_back)
        if prog_msg is None:
            await self.bot.send_message(admin_id, f'{chat_name} ({chat_id}) 下载完成')
        else:
            await self.bot.edit_message(prog_msg, f'{chat_name} ({chat_id}) 下载完成')

    def _register_hooks(self):
        @self.bot.on(events.CallbackQuery())
        async def callback_query_handler(event: events.CallbackQuery.Event):
            await self._callback_handler(event)

        @self.bot.on(events.NewMessage())
        async def bot_message_handler(event: events.NewMessage.Event):
            if event.chat_id != self._cfg.admin_id:
                try:
                    await self._normal_msg_handler(event)
                except Exception as e:
                    event.reply(f'Error occurs: {e}\n\nPlease contact the admin for fix')
                    raise e
            else:
                try:
                    await self._admin_msg_handler(event)
                except Exception as e:
                    await event.reply(f'Error occurs:\n\n<pre>{html.escape(format_exc())}</pre>', parse_mode='html')
                    raise e

    async def _register_commands(self):
        admin_input_peer = None  # make IDE happy!
        try:
            admin_input_peer = await self.bot.get_input_entity(self._cfg.admin_id)
        except ValueError as e:
            self._logger.critical(
                f'Admin ID {self._cfg.admin_id} is invalid, or you have not had any conversation with '
                f'the bot yet. Please send a "/start" to the bot and retry. Exiting...', exc_info=e)
            exit(-1)

        admin_commands = [
            BotCommand(command="download_history", description='[ START[ END]] 下载历史消息'),
            BotCommand(command="random", description='随机返回一条已索引消息'),
            BotCommand(command="stat", description='索引状态'),
            BotCommand(command="clear", description='清除索引'),
        ]
        commands = [
            BotCommand(command="random", description='随机返回一条已索引消息'),
            BotCommand(command="chats", description='选择聊天'),
        ]
        await self.bot(
            SetBotCommandsRequest(
                scope=BotCommandScopePeer(admin_input_peer),
                lang_code='',
                commands=admin_commands
            )
        )
        await self.bot(
            SetBotCommandsRequest(
                scope=BotCommandScopeDefault(),
                lang_code='',
                commands=commands
            )
        )

    async def _render_respond_text(self, result: SearchResult, used_time: float):
        string_builder = []
        hits = result.hits
        string_builder.append(f'共搜索到 {result.total_results} 个结果，用时 {used_time: .3} 秒：\n\n')
        for hit in result.hits:
            chat_title = await self.backend.translate_chat_id(hit.msg.chat_id)
            string_builder.append(f'<b>{chat_title} [{hit.msg.post_time}]</b>\n')
            string_builder.append(f'<a href="{hit.msg.url}">{hit.highlighted}</a>\n')
        return ''.join(string_builder)

    def _render_respond_buttons(self, result, cur_page_num):
        former_page, former_text = ('-1', ' ') \
            if cur_page_num == 1 \
            else (str(cur_page_num - 1), '上一页⬅️')
        next_page, next_text = ('-1', ' ') \
            if result.is_last_page else \
            (str(cur_page_num + 1), '➡️下一页')
        total_pages = - (- result.total_results // self._cfg.page_len)  # use floor to simulate ceil function
        return [
            [
                Button.inline(former_text, former_page),
                Button.inline(f'{cur_page_num} / {total_pages}', '-1'),
                Button.inline(next_text, next_page),
            ]
        ]
