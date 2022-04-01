import html
import urllib.parse as url_parse
from pathlib import Path
import logging
from typing import Optional

from telethon.utils import resolve_id
from telethon.tl.types import User, Chat, Channel

def get_logger(name: str):
    _logger = logging.getLogger(name)
    return _logger

def ensure_path_exists(path: Path):
    if not path.exists():
        path.mkdir()

def escape_content(content: str) -> str:
    return html.escape(content).replace('\n', ' ')

def remove_first_word(text: str) -> str:
    first_space = text.find(' ')
    if first_space < 0:
        return ''
    else:
        return text[first_space+1:]

def brief_content(content: str, trim_len: int = 20) -> str:
    if len(content) < trim_len:
        return content
    else:
        return content[:trim_len - 4] + '…' + content[-2:]

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

class CommonBotConfig:
    @staticmethod
    def _parse_proxy(proxy_str: str):
        url = url_parse.urlparse(proxy_str)
        return url.scheme, url.hostname, url.port

    def __init__(self, proxy: Optional[str], api_id: int, api_hash: str, runtime_dir: str, name: str):
        self.proxy: Optional[tuple] = proxy and self._parse_proxy(proxy)
        self.api_id: int = api_id
        self.api_hash: str = api_hash
        self.name: str = name
        self.runtime_dir: Path = Path(runtime_dir)
        self.session_dir: Path = self.runtime_dir / name / 'session'
        self.index_dir: Path = self.runtime_dir / name / 'index'
        ensure_path_exists(self.runtime_dir)
        ensure_path_exists(self.runtime_dir / name)
        ensure_path_exists(self.session_dir)
        ensure_path_exists(self.index_dir)

