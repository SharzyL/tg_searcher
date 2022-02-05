from typing import Dict, List

from telethon.client import TelegramClient

from .common import get_logger, format_entity_name

class ClientSession(TelegramClient):
    def __init__(self, *args, name, **argv):
        super().__init__(*args, **argv)
        self.name = name
        self._logger = get_logger(f'session:{name}')
        self._id_to_title_table: Dict[int, str] = dict()

    async def start(self, *args, **argv) -> 'TelegramClient':
        ret = super().start(*args, **argv)
        if hasattr(ret, '__await__'):
            ret = await ret
        await self.refresh_translate_table()
        return ret

    async def translate_chat_id(self, chat_id: int) -> str:
        # TODO: unify dialog title / entity name
        if chat_id not in self._id_to_title_table:
            entity = await self.get_entity(await self.get_input_entity(chat_id))
            self._id_to_title_table[chat_id] = format_entity_name(entity)
        return self._id_to_title_table[chat_id]

    async def refresh_translate_table(self):
        self._logger.info(f'Start iterating dialogs')
        self._id_to_title_table.clear()
        async for dialog in self.iter_dialogs(ignore_migrated=True):
            self._id_to_title_table[dialog.entity.id] = dialog.name
        self._logger.info(f'End iterating dialogs, {len(self._id_to_title_table)} dialogs in total')

    async def find_chat_id(self, q: str) -> List[int]:
        chat_ids = []
        for chat_id, chat_name in self._id_to_title_table.items():
            if q.lower() in chat_name.lower():
                chat_ids.append(chat_id)
        return chat_ids
