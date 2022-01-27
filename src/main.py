import logging

import yaml
from argparse import ArgumentParser
from pathlib import Path
import asyncio

from frontend_bot import BotFrontend, BotFrontendConfig
from backend_bot import BackendBot, BackendBotConfig
from common import CommonBotConfig


async def main():
    parser = ArgumentParser(description='A server to provide Telegram message searching')
    parser.add_argument('-c', '--clear', action='store_const', const=True, default=False,
                        help='Build a new index from the scratch')
    parser.add_argument('-f', '--config', action='store', default='searcher.yaml',
                        help='Specify where the configuration yaml file lies')
    parser.add_argument('--debug', action='store_true', help='set loglevel to DEBUG')
    args = parser.parse_args()

    logging.basicConfig(level=logging.INFO)
    if args.debug:
        logging.basicConfig(level=logging.DEBUG)

    full_config = yaml.safe_load(Path(args.config).read_text())
    common_config = CommonBotConfig(**full_config['common'])
    backends: dict[str, BackendBot] = dict()
    frontends: dict[str, BotFrontend] = dict()

    for backend_yaml in full_config['backends']:
        backend_id = backend_yaml['id']
        backend_config = BackendBotConfig(**backend_yaml['config'])
        backend = BackendBot(common_config, backend_config, args.clear, backend_id)
        await backend.start()
        if backend_id not in backends:
            backends[backend_id] = backend
        else:
            raise RuntimeError(f'Duplicated backend id: {backend_id}')

    for frontend_yaml in full_config['frontends']:
        backend_id = frontend_yaml['use_backend']
        frontend_id = frontend_yaml['id']
        frontend_config = BotFrontendConfig(**frontend_yaml['config'])
        frontend = BotFrontend(common_config, frontend_config,
                               frontend_id=frontend_id, backend=backends[backend_id])
        await frontend.start()
        if frontend_id not in frontends:
            frontends[frontend_id] = frontend
        else:
            raise RuntimeError(f'Duplicated frontend id: {frontend_id}')

    assert len(frontends) > 0
    for frontend in frontends.values():
        await frontend.bot.run_until_disconnected()


if __name__ == '__main__':
    asyncio.run(main())

