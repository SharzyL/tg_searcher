from common import CommonBotConfig
from typing import Optional

from telethon import TelegramClient, events

from indexer import Indexer
from common import strip_content, get_share_id, get_logger, format_entity_name

class BackendBotConfig:
    yaml_tag = u'!indexer'

    def __init__(self, phone: Optional[str], indexed_chats: list):
        self.phone: Optional[str] = phone
        self.indexed_chats: list = indexed_chats


class BackendBot:
    def __init__(self, common_cfg: CommonBotConfig, cfg: BackendBotConfig, clean_db: bool):
        self._client = TelegramClient(
            str(common_cfg.session_dir / 'indexer.session'),
            api_id=common_cfg.api_id,
            api_hash=common_cfg.api_hash,
            proxy=common_cfg.proxy,
        )
        if cfg.phone:
            self._client.start(phone=lambda _: cfg.phone)
        else:
            self._client.start()

        self._indexer: Indexer = Indexer(common_cfg.index_dir, common_cfg.name, clean_db)
        self._indexed_chats = cfg.indexed_chats
        self._logger = get_logger('indexer_bot')
        self._id_to_title_table: dict[int, str] = dict()

        await self._client.get_dialogs()  # fill in entity cache, to make sure that dialogs can be found by id
        for chat_id in cfg.indexed_chats:
            entity = await self._client.get_entity(await self._client.get_input_entity(chat_id))
            self._id_to_title_table[chat_id] = format_entity_name(entity)
            self._logger.info(f'ready to monitor "{self._id_to_title_table[chat_id]}" ({chat_id})')

        self._register_hooks()

    def query(self, key_word: str, page_num: int, page_len: int):
        # TODO
        ...

    def rand_msg(self):
        # TODO
        ...

    def start_track_chat(self, chat) -> int:  # return chat_id
        # TODO
        ...

    def download_history(self, chat_id: int, min_id: Optional[int], max_id: Optional[int], call_back):
        # TODO
        ...

    def clear(self):
        # TODO
        ...

    def get_stat(self):
        # TODO
        ...

    def _register_hooks(self):
        @self._client.on(events.NewMessage(chats=self._indexed_chats))
        async def client_message_handler(event):
            if event.raw_text and len(event.raw_text.strip()) >= 0:
                share_id = get_share_id(event.chat_id)
                url = f'https://t.me/c/{share_id}/{event.id}'
                self._logger.info(f'New message {url}')
                self._indexer.index(
                    content=strip_content(event.raw_text),
                    url=url,
                    chat_id=share_id,
                    post_timestamp=event.date.timestamp(),
                )

        @self._client.on(events.MessageEdited(chats=self._indexed_chats))
        async def client_message_update_handler(event):
            if event.raw_text and len(event.raw_text.strip()) >= 0:
                share_id = get_share_id(event.chat_id)
                url = f'https://t.me/c/{share_id}/{event.id}'
                self._logger.info(f'Update message {url}')
                self._indexer.update(url=url, content=strip_content(event.raw_text))

        @self._client.on(events.MessageDeleted())
        async def client_message_delete_handler(event):
            share_id = get_share_id(event.chat_id)
            if event.chat_id and share_id in self._indexed_chats:
                for msg_id in event.deleted_ids:
                    url = f'https://t.me/c/{share_id}/{msg_id}'
                    self._logger.info(f'Delete message {url}')
                    self._indexer.delete(url=url)

