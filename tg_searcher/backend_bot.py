import html
from datetime import datetime
from typing import Optional, Union, Iterable, List, Set, Dict

from telethon import TelegramClient, events
from telethon.tl.patched import Message as TgMessage
from telethon.tl.types import User

from .indexer import Indexer, IndexMsg
from .common import CommonBotConfig, strip_content, get_share_id, get_logger, format_entity_name

class BackendBotConfig:
    def __init__(self, **kw):
        self.monitor_all = kw.get('monitor_all', False)
        self.excluded_chats: Set[int] = set(get_share_id(chat_id)
                                            for chat_id in kw.get('exclude_chats', []))

class BackendBot:
    def __init__(self, common_cfg: CommonBotConfig, cfg: BackendBotConfig,
                 client: TelegramClient, clean_db: bool, backend_id: str):
        self.id: str = backend_id
        self.client = client

        self._logger = get_logger(f'bot-backend:{backend_id}')
        self._cfg = cfg
        if clean_db:
            self._logger.info(f'Index will be cleaned')
        self._indexer: Indexer = Indexer(common_cfg.index_dir / backend_id, clean_db)
        self._id_to_title_table: Dict[int, str] = dict()

        # on startup, all indexed chats are added to monitor list
        self.monitored_chats: Set[int] = self._indexer.list_indexed_chats()
        self.excluded_chats = cfg.excluded_chats

    async def start(self):
        self._logger.info(f'Init backend bot')

        self._logger.info(f'Start iterating dialogs')
        async for dialog in self.client.iter_dialogs(ignore_migrated=True):
            self._id_to_title_table[dialog.entity.id] = dialog.name
        self._logger.info(f'End iterating dialogs, {len(self._id_to_title_table)} dialogs in total')
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
        self._logger.info(f'Downloading history from {chat_id} ({min_id=}, {max_id=})')
        if chat_id not in self.monitored_chats:
            self.monitored_chats.add(chat_id)
        async for tg_message in self.client.iter_messages(chat_id, min_id=min_id, max_id=max_id):
            if tg_message.raw_text and len(tg_message.raw_text.strip()) >= 0:
                share_id = get_share_id(chat_id)
                url = f'https://t.me/c/{share_id}/{tg_message.id}'
                sender = await self._get_sender_name(tg_message)
                msg = IndexMsg(
                    content=strip_content(tg_message.raw_text),
                    url=url,
                    chat_id=chat_id,
                    post_time=datetime.fromtimestamp(tg_message.date.timestamp()),
                    sender=sender,
                )
                self._indexer.add_document(msg, writer)
                await call_back(tg_message.id)
        writer.commit()

    def clear(self, chat_ids: Optional[List[int]] = None):
        if chat_ids is not None:
            for chat_id in chat_ids:
                with self._indexer.ix.writer() as w:
                    w.delete_by_term('chat_id', str(chat_id))
        else:
            self._indexer.clear()

    async def find_chat_id(self, q: str) -> List[int]:
        chat_ids = []
        for chat_id, chat_name in self._id_to_title_table.items():
            if q.lower() in chat_name.lower():
                chat_ids.append(chat_id)
        return chat_ids

    async def get_index_status(self):
        # TODO: add session and frontend name
        sb = [  # string builder
            f'后端 "{self.id}" 总消息数: <b>{self._indexer.ix.doc_count()}</b>\n\n'
        ]
        if self._cfg.monitor_all:
            sb.append(f'{len(self.excluded_chats)} 个对话被禁止索引\n')
            for chat_id in self.excluded_chats:
                sb.append(f'- {await self.format_dialog_html(chat_id)}\n')
            sb.append('\n')

        sb.append(f'总计 {len(self.monitored_chats)} 个对话被加入了索引：\n')
        print_limit = 100  # since telegram can send at most 4096 chars per msg
        for chat_id in self.monitored_chats:
            if print_limit > 0:
                print_limit -= 1
            else:
                break
            num = self._indexer.count_by_query(chat_id=str(chat_id))
            sb.append(f'- {await self.format_dialog_html(chat_id)} '
                      f'共 {num} 条消息\n')
        if print_limit == 0:
            sb.append(f'\n由于 Telegram 的消息长度限制，最多只显示 100 个对话')
        return ''.join(sb)

    async def translate_chat_id(self, chat_id: int):
        if chat_id not in self._id_to_title_table:
            entity = await self.client.get_entity(await self.client.get_input_entity(chat_id))
            self._id_to_title_table[chat_id] = format_entity_name(entity)
        return self._id_to_title_table[chat_id]

    async def format_dialog_html(self, chat_id: int):
        # TODO: handle PM URL
        name = await self.translate_chat_id(chat_id)
        return f'<a href = "https://t.me/c/{chat_id}/99999999">{html.escape(name)}</a> ({chat_id})'

    def _should_monitor(self, chat_id: int):
        # tell if a chat should be monitored
        if self._cfg.monitor_all:
            return chat_id not in self.excluded_chats
        else:
            return chat_id in self.monitored_chats

    async def _get_sender_name(self, message: TgMessage) -> str:
        # empty string will be returned if no sender
        sender = await message.get_sender()
        if isinstance(sender, User):
            return format_entity_name(sender)
        else:
            return ''

    def _register_hooks(self):
        @self.client.on(events.NewMessage())
        async def client_message_handler(event: events.NewMessage.Event):
            if self._should_monitor(event.chat_id) and event.raw_text and len(event.raw_text.strip()) >= 0:
                share_id = get_share_id(event.chat_id)
                sender = await self._get_sender_name(event.message)
                url = f'https://t.me/c/{share_id}/{event.id}'
                self._logger.info(f'New message {url} from "{sender}"')
                msg = IndexMsg(
                    content=strip_content(event.raw_text),
                    url=url,
                    chat_id=share_id,
                    post_time=datetime.fromtimestamp(event.date.timestamp()),
                    sender=sender
                )
                self._indexer.add_document(msg)

        @self.client.on(events.MessageEdited())
        async def client_message_update_handler(event: events.MessageEdited.Event):
            if self._should_monitor(event.chat_id) and event.raw_text and len(event.raw_text.strip()) >= 0:
                share_id = get_share_id(event.chat_id)
                url = f'https://t.me/c/{share_id}/{event.id}'
                self._logger.info(f'Update message {url}')
                self._indexer.update(url=url, content=strip_content(event.raw_text))

        @self.client.on(events.MessageDeleted())
        async def client_message_delete_handler(event: events.MessageDeleted.Event):
            share_id = get_share_id(event.chat_id)
            if event.chat_id and self._should_monitor(event.chat_id):
                for msg_id in event.deleted_ids:
                    url = f'https://t.me/c/{share_id}/{msg_id}'
                    self._logger.info(f'Delete message {url}')
                    self._indexer.delete(url=url)
