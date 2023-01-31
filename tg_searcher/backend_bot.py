import html
from datetime import datetime
from typing import Optional, List, Set, Dict

import telethon.errors.rpcerrorlist
from telethon import events
from telethon.tl.patched import Message as TgMessage
from telethon.tl.types import User

from .indexer import Indexer, IndexMsg
from .common import CommonBotConfig, escape_content, get_share_id, get_logger, format_entity_name, brief_content, \
    EntityNotFoundError
from .session import ClientSession


class BackendBotConfig:
    def __init__(self, **kw):
        self.monitor_all = kw.get('monitor_all', False)
        self.excluded_chats: Set[int] = set(get_share_id(chat_id)
                                            for chat_id in kw.get('exclude_chats', []))


class BackendBot:
    def __init__(self, common_cfg: CommonBotConfig, cfg: BackendBotConfig,
                 session: ClientSession, clean_db: bool, backend_id: str):
        self.id: str = backend_id
        self.session = session

        self._logger = get_logger(f'bot-backend:{backend_id}')
        self._cfg = cfg
        if clean_db:
            self._logger.info(f'Index will be cleaned')
        self._indexer: Indexer = Indexer(common_cfg.index_dir / backend_id, clean_db)

        # on startup, all indexed chats are added to monitor list
        self.monitored_chats: Set[int] = self._indexer.list_indexed_chats()
        self.excluded_chats = cfg.excluded_chats
        self.newest_msg: Dict[int, IndexMsg] = dict()

    async def start(self):
        self._logger.info(f'Init backend bot')

        for chat_id in self.monitored_chats:
            chat_name = await self.translate_chat_id(chat_id)
            self._logger.info(f'Ready to monitor "{chat_name}" ({chat_id})')

        self._register_hooks()

    def search(self, q: str, in_chats: Optional[List[int]], page_len: int, page_num: int):
        return self._indexer.search(q, in_chats, page_len, page_num)

    def rand_msg(self) -> IndexMsg:
        return self._indexer.retrieve_random_document()

    def is_empty(self, chat_id=None):
        if chat_id is not None:
            with self._indexer.ix.searcher() as searcher:
                return not any(True for _ in searcher.document_numbers(chat_id=str(chat_id)))
        else:
            return self._indexer.ix.is_empty()

    async def download_history(self, chat_id: int, min_id: int, max_id: int, call_back=None):
        writer = self._indexer.ix.writer()
        share_id = get_share_id(chat_id)
        self._logger.info(f'Downloading history from {share_id} ({min_id=}, {max_id=})')
        self.monitored_chats.add(share_id)
        async for tg_message in self.session.iter_messages(chat_id, min_id=min_id, max_id=max_id):
            if msg_text := self._extract_text(tg_message):
                url = f'https://t.me/c/{share_id}/{tg_message.id}'
                sender = await self._get_sender_name(tg_message)
                msg = IndexMsg(
                    content=msg_text,
                    url=url,
                    chat_id=chat_id,
                    post_time=datetime.fromtimestamp(tg_message.date.timestamp()),
                    sender=sender,
                )
                self._indexer.add_document(msg, writer)
                self.newest_msg[share_id] = msg
                if call_back:
                    await call_back(tg_message.id)
        writer.commit()

    def clear(self, chat_ids: Optional[List[int]] = None):
        if chat_ids is not None:
            for chat_id in chat_ids:
                with self._indexer.ix.writer() as w:
                    w.delete_by_term('chat_id', str(chat_id))
            for chat_id in chat_ids:
                self.monitored_chats.remove(chat_id)
        else:
            self._indexer.clear()
            self.monitored_chats.clear()

    async def find_chat_id(self, q: str) -> List[int]:
        return await self.session.find_chat_id(q)

    async def get_index_status(self, length_limit: int = 4000):
        # TODO: add session and frontend name
        cur_len = 0
        sb = [  # string builder
            f'后端 "{self.id}"（session: "{self.session.name}"）总消息数: <b>{self._indexer.ix.doc_count()}</b>\n\n'
        ]
        overflow_msg = f'\n\n由于 Telegram 消息长度限制，部分对话的统计信息没有展示'

        def append_msg(msg_list: List[str]):  # return whether overflow
            nonlocal cur_len, sb
            total_len = sum(len(msg) for msg in msg_list)
            if cur_len + total_len > length_limit - len(overflow_msg):
                return True
            else:
                cur_len += total_len
                for msg in msg_list:
                    sb.append(msg)
                    return False

        if self._cfg.monitor_all:
            append_msg([f'{len(self.excluded_chats)} 个对话被禁止索引\n'])
            for chat_id in self.excluded_chats:
                append_msg([f'- {await self.format_dialog_html(chat_id)}\n'])
            sb.append('\n')

        append_msg([f'总计 {len(self.monitored_chats)} 个对话被加入了索引：\n'])
        for chat_id in self.monitored_chats:
            msg_for_chat = []
            num = self._indexer.count_by_query(chat_id=str(chat_id))
            msg_for_chat.append(f'- {await self.format_dialog_html(chat_id)} 共 {num} 条消息\n')
            if newest_msg := self.newest_msg.get(chat_id, None):
                msg_for_chat.append(f'  最新消息：<a href="{newest_msg.url}">{brief_content(newest_msg.content)}</a>\n')
            if append_msg(msg_for_chat):
                # if overflow
                sb.append(overflow_msg)
                break

        return ''.join(sb)

    async def translate_chat_id(self, chat_id: int) -> str:
        try:
            return await self.session.translate_chat_id(chat_id)
        except telethon.errors.rpcerrorlist.ChannelPrivateError:
            return '[无法获取名称]'

    async def str_to_chat_id(self, chat: str) -> int:
        return await self.session.str_to_chat_id(chat)

    async def format_dialog_html(self, chat_id: int):
        # TODO: handle PM URL
        name = await self.translate_chat_id(chat_id)
        return f'<a href = "https://t.me/c/{chat_id}/99999999">{html.escape(name)}</a> ({chat_id})'

    def _should_monitor(self, chat_id: int):
        # tell if a chat should be monitored
        share_id = get_share_id(chat_id)
        if self._cfg.monitor_all:
            return share_id not in self.excluded_chats
        else:
            return share_id in self.monitored_chats

    @staticmethod
    def _extract_text(event):
        if hasattr(event, 'raw_text') and event.raw_text and len(event.raw_text.strip()) >= 0:
            return escape_content(event.raw_text.strip())
        else:
            return ''

    @staticmethod
    async def _get_sender_name(message: TgMessage) -> str:
        # empty string will be returned if no sender
        sender = await message.get_sender()
        if isinstance(sender, User):
            return format_entity_name(sender)
        else:
            return ''

    def _register_hooks(self):
        @self.session.on(events.NewMessage())
        async def client_message_handler(event: events.NewMessage.Event):
            if self._should_monitor(event.chat_id) and (msg_text := self._extract_text(event)):
                share_id = get_share_id(event.chat_id)
                sender = await self._get_sender_name(event.message)
                url = f'https://t.me/c/{share_id}/{event.id}'
                self._logger.info(f'New msg {url} from "{sender}": "{brief_content(msg_text)}"')
                msg = IndexMsg(
                    content=msg_text,
                    url=url,
                    chat_id=share_id,
                    post_time=datetime.fromtimestamp(event.date.timestamp()),
                    sender=sender
                )
                self.newest_msg[share_id] = msg
                self._indexer.add_document(msg)

        @self.session.on(events.MessageEdited())
        async def client_message_update_handler(event: events.MessageEdited.Event):
            if self._should_monitor(event.chat_id) and (msg_text := self._extract_text(event)):
                share_id = get_share_id(event.chat_id)
                url = f'https://t.me/c/{share_id}/{event.id}'
                self._logger.info(f'Update message {url} to: "{brief_content(msg_text)}"')
                self._indexer.update(url=url, content=msg_text)

        @self.session.on(events.MessageDeleted())
        async def client_message_delete_handler(event: events.MessageDeleted.Event):
            if not hasattr(event, 'chat_id') or event.chat_id is None:
                return
            if self._should_monitor(event.chat_id):
                share_id = get_share_id(event.chat_id)
                for msg_id in event.deleted_ids:
                    url = f'https://t.me/c/{share_id}/{msg_id}'
                    self._logger.info(f'Delete message {url}')
                    self._indexer.delete(url=url)
