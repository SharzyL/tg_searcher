import html
from datetime import datetime

from common import CommonBotConfig
from typing import Optional

from telethon import TelegramClient, events

from indexer import Indexer, IndexMsg
from common import strip_content, get_share_id, get_logger, format_entity_name

class BackendBotConfig:
    def __init__(self, phone: Optional[str], indexed_chats: list):
        self.phone: Optional[str] = phone
        self.indexed_chats: list = indexed_chats


class BackendBot:
    def __init__(self, common_cfg: CommonBotConfig, cfg: BackendBotConfig, clean_db: bool, backend_id: str):
        self.id = backend_id
        self.client = TelegramClient(
            str(common_cfg.session_dir / f'backend_{self.id}.session'),
            api_id=common_cfg.api_id,
            api_hash=common_cfg.api_hash,
            proxy=common_cfg.proxy,
        )
        self.indexed_chats = cfg.indexed_chats

        self._indexer: Indexer = Indexer(common_cfg.index_dir, backend_id, clean_db)
        self._logger = get_logger(f'bot-backend:{backend_id}')
        self._id_to_title_table: dict[int, str] = dict()
        self._cfg = cfg

    async def start(self):
        self._logger.info(f'init backend bot {self.id}')
        if self._cfg.phone:
            await self.client.start(phone=lambda: self._cfg.phone)
        else:
            await self.client.start()

        await self.client.get_dialogs()  # fill in entity cache, to make sure that dialogs can be found by id
        for chat_id in self._cfg.indexed_chats:
            chat_name = await self.translate_chat_id(chat_id)
            self._logger.info(f'ready to monitor "{chat_name}" ({chat_id})')

        self._register_hooks()

    async def translate_chat_id(self, chat_id: int):
        if chat_id not in self._id_to_title_table:
            entity = await self.client.get_entity(await self.client.get_input_entity(chat_id))
            self._id_to_title_table[chat_id] = format_entity_name(entity)
        return self._id_to_title_table[chat_id]

    def search(self, q: str, in_chats: Optional[list[int]], page_len: int, page_num: int):
        return self._indexer.search(q, in_chats, page_len, page_num)

    async def search_chat_id(self, q: str) -> list[int]:
        chat_ids = []
        async for dialog in self.client.iter_dialogs():
            if q in dialog.name:
                chat_ids.append(dialog.entity.id)
        return chat_ids

    def rand_msg(self) -> IndexMsg:
        return self._indexer.retrieve_random_document()

    async def download_history(self, chat_id: int, min_id: int, max_id: int, call_back=None):
        writer = self._indexer.ix.writer()
        self._logger.info(f'Downloading history from {chat_id} ({min_id=}, {max_id=})')
        async for tg_message in self.client.iter_messages(chat_id, min_id=min_id, max_id=max_id):
            if tg_message.raw_text and len(tg_message.raw_text.strip()) >= 0:
                share_id = get_share_id(chat_id)
                url = f'https://t.me/c/{share_id}/{tg_message.id}'
                msg = IndexMsg(
                    content=strip_content(tg_message.raw_text),
                    url=url,
                    chat_id=chat_id,
                    post_time=datetime.fromtimestamp(tg_message.date.timestamp())
                )
                self._indexer.add_document(msg, writer)
                await call_back(tg_message.id)
        if chat_id not in self.indexed_chats:
            self.indexed_chats.append(chat_id)
        writer.commit()

    def clear(self, chat_ids: Optional[list[int]] = None):
        if chat_ids is not None:
            for chat_id in chat_ids:
                with self._indexer.ix.writer() as w:
                    w.delete_by_term('chat_id', chat_id)
        else:
            self._indexer.clear()

    async def get_stat(self):
        sb = [
            f'后端 "{self.id}" 总消息数: <b>{self._indexer.ix.doc_count()}</b>\n\n'
            f'总计 {len(self.indexed_chats)} 个对话被加入了索引：\n'
        ]  # string builder
        # TODO: print 'hidden' chats
        for chat_id, name in self._id_to_title_table.items():
            num = self._indexer.count(chat_id=chat_id)
            # TODO: handle PM URL
            sb.append(f'- <a href="https://t.me/c/{chat_id}/99999999">{html.escape(name)}</a>'
                      f' ({chat_id}) '
                      f'共 {num} 条消息\n')
        return ''.join(sb)

    def is_empty(self, chat_id=None):
        if chat_id is not None:
            with self._indexer.ix.searcher() as searcher:
                return not any(True for _ in searcher.document_numbers(chat_id=chat_id))
        else:
            return self._indexer.ix.is_empty()

    def _register_hooks(self):
        @self.client.on(events.NewMessage())
        async def client_message_handler(event: events.NewMessage.Event):
            if event.chat_id in self.indexed_chats and event.raw_text and len(event.raw_text.strip()) >= 0:
                share_id = get_share_id(event.chat_id)
                url = f'https://t.me/c/{share_id}/{event.id}'
                self._logger.info(f'New message {url}')
                msg = IndexMsg(
                    content=strip_content(event.raw_text),
                    url=url,
                    chat_id=share_id,
                    post_time=datetime.fromtimestamp(event.date.timestamp()),
                )
                self._indexer.add_document(msg)

        @self.client.on(events.MessageEdited())
        async def client_message_update_handler(event: events.MessageEdited.Event):
            if event.chat_id in self.indexed_chats and event.raw_text and len(event.raw_text.strip()) >= 0:
                share_id = get_share_id(event.chat_id)
                url = f'https://t.me/c/{share_id}/{event.id}'
                self._logger.info(f'Update message {url}')
                self._indexer.update(url=url, content=strip_content(event.raw_text))

        @self.client.on(events.MessageDeleted())
        async def client_message_delete_handler(event: events.MessageDeleted.Event):
            share_id = get_share_id(event.chat_id)
            if event.chat_id and event.chat_id in self.indexed_chats:
                for msg_id in event.deleted_ids:
                    url = f'https://t.me/c/{share_id}/{msg_id}'
                    self._logger.info(f'Delete message {url}')
                    self._indexer.delete(url=url)
