import html
from datetime import datetime

from common import CommonBotConfig
from typing import Optional

from telethon import TelegramClient, events

from indexer import Indexer, Message, SearchResult
from common import strip_content, get_share_id, get_logger, format_entity_name

class BackendBotConfig:
    def __init__(self, phone: Optional[str], indexed_chats: list):
        self.phone: Optional[str] = phone
        self.indexed_chats: list = indexed_chats


class BackendBot:
    def __init__(self, common_cfg: CommonBotConfig, cfg: BackendBotConfig, clean_db: bool):
        self.client = TelegramClient(
            str(common_cfg.session_dir / 'indexer.session'),
            api_id=common_cfg.api_id,
            api_hash=common_cfg.api_hash,
            proxy=common_cfg.proxy,
        )
        self._indexer: Indexer = Indexer(common_cfg.index_dir, common_cfg.name, clean_db)
        self._indexed_chats = cfg.indexed_chats
        self._logger = get_logger('indexer_bot')
        self._id_to_title_table: dict[int, str] = dict()
        self._cfg = cfg

    async def start(self):
        await self.client.start()

        await self.client.get_dialogs()  # fill in entity cache, to make sure that dialogs can be found by id
        for chat_id in self._cfg.indexed_chats:
            entity = await self.client.get_entity(await self.client.get_input_entity(chat_id))
            self._id_to_title_table[chat_id] = format_entity_name(entity)
            self._logger.info(f'ready to monitor "{self._id_to_title_table[chat_id]}" ({chat_id})')

        self._register_hooks()

    def translate_chat_id(self, chat_id: int):
        return self._id_to_title_table[chat_id]

    def search(self, q: str, in_chats: Optional[list[int]], page_len: int, page_num: int):
        return self._indexer.search(q, in_chats, page_len, page_num)

    def rand_msg(self) -> Message:
        return self._indexer.retrieve_random_document()

    def start_track_chat(self, chat_id: int) -> int:  # return chat_id
        # TODO
        ...

    async def download_history(self, min_id: int, max_id: int, call_back=None):
        writer = self._indexer.ix.writer()
        for chat_id in self._cfg.indexed_chats:
            self._logger.info(f'Downloading history from {chat_id} ({min_id=}, {max_id=})')
            async for tg_message in self.client.iter_messages(chat_id, min_id=min_id, max_id=max_id):
                # FIXME: it seems that iterating over PM return nothing?
                if tg_message.raw_text and len(tg_message.raw_text.strip()) >= 0:
                    share_id = get_share_id(chat_id)
                    url = f'https://t.me/c/{share_id}/{tg_message.id}'
                    msg = Message(
                        content=strip_content(tg_message.raw_text),
                        url=url,
                        chat_id=chat_id,
                        post_time=datetime.fromtimestamp(tg_message.date.timestamp())
                    )
                    self._indexer.add_document(msg, writer)
                    await call_back(chat_id, tg_message.id)
            await call_back(chat_id, -1)  # indicating the end
        writer.commit()

    def clear(self):
        self._indexer.clear()

    def get_stat(self):
        sb = []  # string builder
        sb.append(f'The status of backend:\n\n')
        sb.append(f'Count of messages: <b>{self._indexer.ix.doc_count()}</b>\n\n')
        sb.append(f'{len(self._indexed_chats)} chats are being monitored:\n')
        for chat_id, name in self._id_to_title_table.items():
            sb.append(f'  - <b>{html.escape(name)}</b> ({chat_id}):\n')
        return ''.join(sb)

    def is_empty(self):
        return self._indexer.ix.is_empty()

    def _register_hooks(self):
        @self.client.on(events.NewMessage(chats=self._indexed_chats))
        async def client_message_handler(event):
            if event.raw_text and len(event.raw_text.strip()) >= 0:
                share_id = get_share_id(event.chat_id)
                url = f'https://t.me/c/{share_id}/{event.id}'
                self._logger.info(f'New message {url}')
                msg = Message(
                    content=strip_content(event.raw_text),
                    url=url,
                    chat_id=share_id,
                    post_time=datetime.fromtimestamp(event.date.timestamp()),
                )
                self._indexer.add_document(msg)

        @self.client.on(events.MessageEdited(chats=self._indexed_chats))
        async def client_message_update_handler(event):
            if event.raw_text and len(event.raw_text.strip()) >= 0:
                share_id = get_share_id(event.chat_id)
                url = f'https://t.me/c/{share_id}/{event.id}'
                self._logger.info(f'Update message {url}')
                self._indexer.update(url=url, content=strip_content(event.raw_text))

        @self.client.on(events.MessageDeleted())
        async def client_message_delete_handler(event):
            share_id = get_share_id(event.chat_id)
            if event.chat_id and share_id in self._indexed_chats:
                for msg_id in event.deleted_ids:
                    url = f'https://t.me/c/{share_id}/{msg_id}'
                    self._logger.info(f'Delete message {url}')
                    self._indexer.delete(url=url)
