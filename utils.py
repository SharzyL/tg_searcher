import html

from telethon.utils import resolve_id
from telethon.tl.types import User, Chat, Channel

def strip_content(content: str) -> str:
    return html.escape(content).replace('\n', ' ')

def get_share_id(chat_id: int) -> int:
    return resolve_id(chat_id)[0]

def format_entity_name(entity):
    if isinstance(entity, User):
        first_name = entity.first_name or ''
        last_name = entity.last_name or ''
        return (first_name + ' ' + last_name).strip()
    elif isinstance(entity, Chat) or isinstance(entity, Channel):
        return entity.title
    else:
        raise ValueError(f'Unknown entity {entity}')
