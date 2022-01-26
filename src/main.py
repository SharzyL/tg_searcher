import yaml
from argparse import ArgumentParser
from pathlib import Path
import asyncio

from frontend_bot import SingleUserFrontend, SingleUserFrontendConfig
from backend_bot import BackendBot, BackendBotConfig
from common import CommonBotConfig


async def main():
    parser = ArgumentParser(description='A server to provide Telegram message searching')
    parser.add_argument('-c', '--clear', action='store_const', const=True, default=False,
                        help='Build a new index from the scratch')
    parser.add_argument('-f', '--config', action='store', default='searcher.yaml',
                        help='Specify where the configuration yaml file lies')
    args = parser.parse_args()

    full_config = yaml.safe_load(Path(args.config).read_text())
    
    backend_config = BackendBotConfig(**full_config['indexer'])
    frontend_config = SingleUserFrontendConfig(**full_config['single_user_frontend'])
    common_config = CommonBotConfig(**full_config['common'])
    
    backend = BackendBot(common_config, backend_config, args.clear)
    frontend = SingleUserFrontend(common_config, frontend_config, backend)
    await backend.start()
    await frontend.start()

    try:
        asyncio.get_event_loop().run_forever()
    except KeyboardInterrupt:
        pass


if __name__ == '__main__':
    main()
