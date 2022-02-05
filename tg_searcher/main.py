import logging

import yaml
from argparse import ArgumentParser
from pathlib import Path
import asyncio

from telethon.client import TelegramClient

from .frontend_bot import BotFrontend, BotFrontendConfig
from .backend_bot import BackendBot, BackendBotConfig
from .session import ClientSession
from .common import CommonBotConfig


async def a_main():
    parser = ArgumentParser(description='A server to provide Telegram message searching')
    parser.add_argument('-c', '--clear', action='store_const', const=True, default=False,
                        help='Clear existing index')
    parser.add_argument('-f', '--config', action='store', default='searcher.yaml',
                        help='Specify where the configuration yaml file lies')
    parser.add_argument('--debug', action='store_true', help='set loglevel to DEBUG')
    args = parser.parse_args()

    logging.basicConfig(level=logging.INFO)
    if args.debug:
        logging.basicConfig(level=logging.DEBUG)

    full_config = yaml.safe_load(Path(args.config).read_text())
    common_config = CommonBotConfig(**full_config['common'])

    sessions: dict[str, ClientSession] = dict()
    backends: dict[str, BackendBot] = dict()
    frontends: dict[str, BotFrontend] = dict()

    for session_yaml in full_config['sessions']:
        session_name = session_yaml['name']
        session = ClientSession(
            str(common_config.session_dir / f'{session_name}.session'),
            name=session_name,
            api_id=common_config.api_id,
            api_hash=common_config.api_hash,
            proxy=common_config.proxy,
        )
        await session.start(phone=lambda: session_yaml['phone'])
        sessions[session_name] = session

    async_tasks = []
    for backend_yaml in full_config['backends']:
        backend_id = backend_yaml['id']
        session_name = backend_yaml['use_session']
        backend_config = BackendBotConfig(**backend_yaml.get('config', {}))
        backend = BackendBot(common_config, backend_config, sessions[session_name], args.clear, backend_id)
        async_tasks.append(backend.start())
        if backend_id not in backends:
            backends[backend_id] = backend
        else:
            raise RuntimeError(f'Duplicated backend id: {backend_id}')

    for frontend_yaml in full_config['frontends']:
        backend_id = frontend_yaml['use_backend']
        frontend_id = frontend_yaml['id']
        frontend_config = BotFrontendConfig(**frontend_yaml.get('config', {}))
        frontend = BotFrontend(common_config, frontend_config,
                               frontend_id=frontend_id, backend=backends[backend_id])
        async_tasks.append(frontend.start())
        if frontend_id not in frontends:
            frontends[frontend_id] = frontend
        else:
            raise RuntimeError(f'Duplicated frontend id: {frontend_id}')

    for task in async_tasks:
        await task

    logging.info(f'Initialization ok')
    assert len(frontends) > 0
    for frontend in frontends.values():
        await frontend.bot.run_until_disconnected()


def main():
    asyncio.run(a_main())
