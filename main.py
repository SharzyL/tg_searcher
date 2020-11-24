import asyncio
import html
import logging
import os
import sys
from pathlib import Path
from time import time
import argparse

import redis
import yaml
from telethon import TelegramClient, events, Button

from indexer import Indexer
from log import get_logger, log_exception

os.chdir(Path(sys.argv[0]).parent)

#################################################################################
# Read arguments and configurations
#################################################################################


parser = argparse.ArgumentParser(description='A server to provide Telegram message searching')
parser.add_argument('-c', '--clear', action='store_const', const=True, default=False,
                    help='Build a new index from the scratch')
parser.add_argument('-f', '--config', action='store', default='searcher.yaml',
                    help='Specify where the configuration yaml file lies')
parser.add_argument('-l', '--log', action='store', default=None,
                    help='The path of logs')

args = parser.parse_args()
will_clear = args.clear
config_path = args.config
log_path = args.log

with open(config_path, 'r', encoding='utf8') as fp:
    config = yaml.safe_load(fp)
redis_host = config.get('redis', {}).get('host', 'localhost')
redis_port = config.get('redis', {}).get('port', '6379')
api_id = config['telegram']['api_id']
api_hash = config['telegram']['api_hash']
bot_token = config['telegram']['bot_token']
admin_id = config['telegram']['admin_id']
chat_ids = config['chat_id']
page_len = config.get('search', {}).get('page_len', 10)
welcome_message = config.get('welcome_message', 'Welcome')

name = config.get('name', '')
private_mode = config.get('private_mode', False)
private_whitelist = config.get('private_whitelist', [])

random_mode = config.get('random_mode', False)

#################################################################################
# Prepare loggers, client connections, and 
#################################################################################


logging.basicConfig(level=logging.INFO)
logger = get_logger(log_path)
indexer = Indexer(from_scratch=will_clear, index_name=f'{name}_index')

db = redis.Redis(host=redis_host, port=redis_port, decode_responses=True)

loop = asyncio.get_event_loop()

session_dir = Path(f'{name}_session')
if not session_dir.exists():
    session_dir.mkdir()

client = TelegramClient(f'{name}_session/client', api_id, api_hash, loop=loop).start()
bot = TelegramClient(f'{name}_session/bot', api_id, api_hash, loop=loop).start(bot_token=bot_token)

id_to_title = dict()  # a dictionary to translate chat id to chat title


#################################################################################
# Handle message from the channel
#################################################################################


def strip_content(content: str) -> str:
    return html.escape(content).replace('\n', ' ')


def get_share_id(chat_id: int) -> int:
    return chat_id if chat_id >= 0 else - chat_id - 1000000000000


@client.on(events.NewMessage(chats=chat_ids))
@log_exception(logger)
async def client_message_handler(event):
    if event.raw_text and len(event.raw_text.strip()) >= 0:
        share_id = get_share_id(event.chat_id)
        url = f'https://t.me/c/{share_id}/{event.id}'
        logger.info(f'New message {url}')
        indexer.index(
            content=strip_content(event.raw_text),
            url=url,
            chat_id=event.chat_id,
            post_timestamp=event.date.timestamp(),
        )


@client.on(events.MessageEdited(chats=chat_ids))
@log_exception(logger)
async def client_message_update_handler(event):
    if event.raw_text and len(event.raw_text.strip()) >= 0:
        share_id = get_share_id(event.chat_id)
        url = f'https://t.me/c/{share_id}/{event.id}'
        logger.info(f'Update message {url}')
        indexer.update(url=url, content=strip_content(event.raw_text))


@client.on(events.MessageDeleted())
@log_exception(logger)
async def client_message_delete_handler(event):
    if event.chat_id and event.chat_id in chat_ids:
        for msg_id in event.deleted_ids:
            share_id = get_share_id(event.chat_id)
            url = f'https://t.me/c/{share_id}/{msg_id}'
            logger.info(f'Delete message {url}')
            indexer.delete(url=url)


#################################################################################
# Handle bot behavior
#################################################################################


def render_respond_text(result, used_time, is_private=False):
    respond = f'共搜索到 {result["total"]} 个结果，用时 {used_time: .3} 秒：\n\n'
    for hit in result['hits']:
        respond += f'<b>{id_to_title[hit["chat_id"]]} [{hit["post_time"]}]</b>\n'
        if is_private:
            respond += f'{hit["url"]}\n'
        else:
            respond += f'<a href="{hit["url"]}">{hit["highlighted"]}</a>\n'
    return respond


def render_respond_buttons(result, cur_page_num):
    former_page, former_text = ('-1', ' ') \
        if cur_page_num == 1 \
        else (str(cur_page_num - 1), '上一页⬅️')
    next_page, next_text = ('-1', ' ') \
        if result['is_last_page'] else \
        (str(cur_page_num + 1), '➡️下一页')
    total_pages = - (- result['total'] // page_len)  # use floor to simulate ceil function
    return [
        [
            Button.inline(former_text, former_page),
            Button.inline(f'{cur_page_num} / {total_pages}', '-1'),
            Button.inline(next_text, next_page),
        ]
    ]


@log_exception(logger)
async def download_history():
    for chat_id in chat_ids:
        await bot.send_message(admin_id, f'开始下载 {id_to_title[chat_id]} 的历史记录')
        logger.info(f'Downloading history from {chat_id}')
        async for message in client.iter_messages(chat_id):
            if message.raw_text and len(message.raw_text.strip()) >= 0:
                uid = message.id
                if uid % 100 == 0:
                    await bot.send_message(admin_id, f'还需下载 {uid} 条消息')
                share_id = get_share_id(chat_id)
                url = f'https://t.me/c/{share_id}/{message.id}'
                indexer.index(
                    content=strip_content(message.raw_text),
                    url=url,
                    chat_id=chat_id,
                    post_timestamp=message.date.timestamp(),
                )
        await bot.send_message(admin_id, '下载完成')


@bot.on(events.CallbackQuery())
@log_exception(logger)
async def bot_callback_handler(event):
    if event.data and event.data != b'-1':
        page_num = int(event.data)
        q = db.get('msg-' + str(event.message_id) + '-q')
        logger.info(f'Query [{q}] turned to page {page_num}')
        if q:
            start_time = time()
            result = indexer.search(q, page_len=page_len, page_num=page_num)
            used_time = time() - start_time
            respond = render_respond_text(result, used_time)
            buttons = render_respond_buttons(result, page_num)
            await event.edit(respond, parse_mode='html', buttons=buttons)
    await event.answer()


@bot.on(events.NewMessage())
@log_exception(logger)
async def bot_message_handler(event):
    text = event.raw_text
    is_private = private_mode and event.chat_id not in private_whitelist
    logger.info(f'User {event.chat_id} Queries [{text}]')
    start_time = time()

    if not (event.raw_text and event.raw_text.strip()):
        return

    elif event.raw_text.startswith('/start'):
        await event.respond(welcome_message, parse_mode='markdown')

    elif event.raw_text.startswith('/random') and random_mode:
        doc = indexer.retrieve_random_document()
        respond = f'Random message from <b>{id_to_title[doc["chat_id"]]} [{doc["post_time"]}]</b>\n'
        respond += f'{doc["url"]}\n'
        await event.respond(respond, parse_mode='html')

    elif event.raw_text.startswith('/download_history') and event.chat_id == admin_id:
        await event.respond('开始下载历史记录', parse_mode='markdown')
        indexer.clear()
        await download_history()

    else:
        q = event.raw_text
        result = indexer.search(q, page_len=page_len, page_num=1)
        used_time = time() - start_time
        respond = render_respond_text(result, used_time, is_private)
        buttons = render_respond_buttons(result, 1)
        msg = await event.respond(respond, parse_mode='html', buttons=buttons)

        db.set('msg-' + str(msg.id) + '-q', q)


#################################################################################
# Prepare main loop
#################################################################################


@log_exception(logger)
async def init_bot():
    # put some async initialization actions here
    for chat_id in chat_ids:
        print(chat_id)
        entity = await client.get_entity(chat_id)
        id_to_title[chat_id] = entity.title
    logger.info('Bot started')
    await bot.send_message(admin_id, 'I am ready. ')


loop.run_until_complete(init_bot())

try:
    loop.run_forever()
except KeyboardInterrupt:
    pass
