from time import time

from telethon import TelegramClient, events, Button
from telethon.tl.types import BotCommand, BotCommandScopePeer
from telethon.tl.functions.bots import SetBotCommandsRequest
from redis import Redis

from common import CommonBotConfig, get_logger
from backend_bot import BackendBot

class SingleUserFrontendConfig:
    yaml_tag = 'single_user_frontend'

    @staticmethod
    def _parse_redis_cfg(redis_cfg: str) -> tuple[str, int]:
        colon_idx = redis_cfg.index(':')
        if colon_idx < 0:
            raise ValueError("No colon in redis host config")
        return redis_cfg[:colon_idx], int(redis_cfg[colon_idx + 1:])

    def __init__(self, bot_token: str, admin_id: int, page_len: int, redis: str):
        self.bot_token = bot_token
        self.admin_id = admin_id
        self.page_len = page_len
        self.redis_host: tuple[str, int] = self._parse_redis_cfg(redis)


class SingleUserFrontend:
    def __init__(self, common_cfg: CommonBotConfig, cfg: SingleUserFrontendConfig, backend: BackendBot):
        self._backend = backend
        self._bot = TelegramClient(
            str(common_cfg.session_dir / 'indexer.session'),
            api_id=common_cfg.api_id,
            api_hash=common_cfg.api_hash,
            proxy=common_cfg.proxy
        )
        self._bot.start(bot_token=cfg.bot_token)
        self._cfg = cfg
        self._redis = Redis(host=cfg.redis_host[0], port=cfg.redis_host[1], decode_responses=True)
        self._logger = get_logger('su-frontend')

    async def start(self):
        await self._register_commands()
        self._register_hooks()

    async def _callback_handler(self, event):
        if event.data and event.data != b'-1':
            page_num = int(event.data)
            q = self._redis.get('msg-' + str(event.message_id) + '-q')
            self._logger.info(f'Query [{q}] turned to page {page_num}')
            if q:
                start_time = time()
                result = self._backend.query(q, self._cfg.page_len, page_num)
                used_time = time() - start_time
                respond = self._render_respond_text(result, used_time)
                buttons = self._render_respond_buttons(result, page_num)
                await event.edit(respond, parse_mode='html', buttons=buttons)
        await event.answer()

    async def _msg_handler(self, event):
        text = event.raw_text
        self._logger.info(f'User {event.chat_id} Queries [{text}]')
        start_time = time()

        if not (event.raw_text and event.raw_text.strip()):
            return

        elif event.raw_text.startswith('/start'):
            return

        elif event.raw_text.startswith('/random'):
            doc = self._backend.rand_msg()
            respond = f'Random message from <b>{doc["chat_id"]} [{doc["post_time"]}]</b>\n'
            respond += f'{doc["url"]}\n'
            await event.respond(respond, parse_mode='html')

        elif event.raw_text.startswith('/download_history') and event.chat_id == self._cfg.admin_id:
            download_args = event.raw_text.split()
            min_id = max(int(download_args[1]), 1) if len(download_args) > 1 else 1
            max_id = int(download_args[2]) if len(download_args) > 2 else 1 << 31 - 1
            await event.respond('开始下载历史记录')
            if len(download_args) == 0:
                self._backend.clear()
            await self._backend.download_history(chat_id=114514, min_id=min_id, max_id=max_id, call_back=None)

        else:
            q = event.raw_text
            result = self._backend.query(q, page_len=self._cfg.page_len, page_num=1)
            used_time = time() - start_time
            respond = self._render_respond_text(result, used_time)
            buttons = self._render_respond_buttons(result, 1)
            msg = await event.respond(respond, parse_mode='html', buttons=buttons)

            self._redis.set('msg-' + str(msg.id) + '-q', q)

    def _register_hooks(self):
        @self._bot.on(events.CallbackQuery())
        async def callback_query_handler(event):
            await self._callback_handler(event)

        @self._bot.on(events.NewMessage())
        async def bot_message_handler(event):
            try:
                await self._msg_handler(event)
            except Exception as e:
                self._logger.error(f'Error occurs on processing bot request: {e}')
                await event.reply(f'Error occurs on processing bot request: {e}')
                raise e

    async def _register_commands(self):
        admin_input_peer = None  # make IDE happy!
        try:
            admin_input_peer = await self._bot.get_input_entity(self._cfg.admin_id)
        except ValueError as e:
            self._logger.critical(f'Admin ID {self._cfg.admin_id} is invalid, or you have not had any conversation with '
                                  f'the bot yet. Please send a "/start" to the bot and retry. Exiting...', exc_info=e)
            exit(-1)

        commands = [
            BotCommand(command="download_history", description='[ START[ END]] 下载历史消息'),
            BotCommand(command="random", description='随机返回一条已索引消息'),
        ]
        await self._bot(
            SetBotCommandsRequest(
                scope=BotCommandScopePeer(admin_input_peer),
                lang_code='zh_CN',
                commands=commands
            )
        )

    def _render_respond_text(self, result, used_time, is_private=False):
        respond = f'共搜索到 {result["total"]} 个结果，用时 {used_time: .3} 秒：\n\n'
        for hit in result['hits']:
            respond += f'<b>{hit["chat_id"]} [{hit["post_time"]}]</b>\n'
            if is_private:
                respond += f'{hit["url"]}\n'
            else:
                respond += f'<a href="{hit["url"]}">{hit["highlighted"]}</a>\n'
        return respond

    def _render_respond_buttons(self, result, cur_page_num):
        former_page, former_text = ('-1', ' ') \
            if cur_page_num == 1 \
            else (str(cur_page_num - 1), '上一页⬅️')
        next_page, next_text = ('-1', ' ') \
            if result['is_last_page'] else \
            (str(cur_page_num + 1), '➡️下一页')
        total_pages = - (- result['total'] // self._cfg.page_len)  # use floor to simulate ceil function
        return [
            [
                Button.inline(former_text, former_page),
                Button.inline(f'{cur_page_num} / {total_pages}', '-1'),
                Button.inline(next_text, next_page),
            ]
        ]
