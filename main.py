import asyncio
import html
import logging
import os
import sys
from pathlib import Path
from time import time
import argparse
from datetime import datetime

import redis
import yaml
from telethon import TelegramClient, events, Button

from indexer import Indexer
from log import get_logger
from utils import strip_content, get_share_id, format_entity_name

os.chdir(Path(sys.argv[0]).parent)

#################################################################################
# Read arguments and configurations
#################################################################################


parser = argparse.ArgumentParser(description='A server to provide Telegram message searching')
parser.add_argument('-c', '--clear', action='store_const', const=True, default=False,
                    help='Build a new index from the scratch')
parser.add_argument('-f', '--config', action='store', default='searcher.yaml',
                    help='Specify where the configuration yaml file lies')

args = parser.parse_args()
will_clear = args.clear
config_path = args.config

with open(config_path, 'r', encoding='utf8') as fp:
    config = yaml.safe_load(fp)
redis_host = config.get('redis', {}).get('host', 'localhost')
redis_port = config.get('redis', {}).get('port', '6379')
api_id = config['telegram']['api_id']
api_hash = config['telegram']['api_hash']
bot_token = config['telegram']['bot_token']
admin_id = get_share_id(config['telegram']['admin_id'])
chat_ids = list(map(get_share_id, config['chat_id']))
page_len = config.get('search', {}).get('page_len', 10)
welcome_message = config.get('welcome_message', 'Welcome')

proxy_protocol = config.get('proxy', {}).get('protocol', None)
assert proxy_protocol in ('socks5', 'socks4', 'http')
proxy_host = config.get('proxy', {}).get('host', None)
proxy_port = config.get('proxy', {}).get('port', None)

runtime_dir = Path(config.get('runtime_dir', '.'))

proxy = None
if proxy_protocol and proxy_host and proxy_port:
    proxy = (proxy_protocol, proxy_host, proxy_port)


name = config.get('name', '')
private_mode = config.get('private_mode', False)
private_whitelist = config.get('private_whitelist', [])

random_mode = config.get('random_mode', False)

#################################################################################
# Prepare loggers, client connections, and 
#################################################################################

if not (Path(runtime_dir) / name).exists():
    (Path(runtime_dir) / name).mkdir()
session_dir = Path(runtime_dir) / name / 'session'
index_dir = Path(runtime_dir) / name / 'index'
if not session_dir.exists():
    session_dir.mkdir()
if not index_dir.exists():
    index_dir.mkdir()

logging.basicConfig(level=logging.INFO)
logger = get_logger()
indexer = Indexer(pickle_path=index_dir, from_scratch=will_clear, index_name=f'{name}_index')

db = redis.Redis(host=redis_host, port=redis_port, decode_responses=True)

loop = asyncio.get_event_loop()

client = TelegramClient(str(session_dir / 'client.session'), api_id, api_hash, loop=loop, proxy=proxy).start()
bot = TelegramClient(str(session_dir / 'bot.session'), api_id, api_hash, loop=loop, proxy=proxy)\
    .start(bot_token=bot_token)

id_to_title = dict()  # a dictionary to translate chat id to chat title


#################################################################################
# Handle message from the channel
#################################################################################

@client.on(events.NewMessage(chats=chat_ids))
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
async def client_message_update_handler(event):
    if event.raw_text and len(event.raw_text.strip()) >= 0:
        share_id = get_share_id(event.chat_id)
        url = f'https://t.me/c/{share_id}/{event.id}'
        logger.info(f'Update message {url}')
        indexer.update(url=url, content=strip_content(event.raw_text))


@client.on(events.MessageDeleted())
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


async def download_history(min_id, max_id):
    for chat_id in chat_ids:
        await bot.send_message(admin_id, f'开始下载 {id_to_title[chat_id]} 的历史记录')
        logger.info(f'Downloading history from {chat_id} ({min_id=}, {max_id=})')
        with indexer.ix.writer() as writer:
            progress_msg = None
            async for message in client.iter_messages(chat_id, min_id=min_id, max_id=max_id):
                # FIXME: it seems that iterating over PM return nothing?
                if message.raw_text and len(message.raw_text.strip()) >= 0:
                    uid = message.id
                    remaining_msg_cnt = uid - min_id
                    if progress_msg is None:
                        progress_msg = await bot.send_message(admin_id, f'还需下载 {remaining_msg_cnt} 条消息')

                    if remaining_msg_cnt % 100 == 0:
                        await bot.edit_message(admin_id, progress_msg, f'还需下载 {remaining_msg_cnt} 条消息')
                    share_id = get_share_id(chat_id)
                    url = f'https://t.me/c/{share_id}/{message.id}'
                    writer.add_document(
                        content=strip_content(message.raw_text),
                        url=url,
                        chat_id=chat_id,
                        post_time=datetime.fromtimestamp(message.date.timestamp())
                    )
        logger.info(f'Complete downloading history from {chat_id}')
        await bot.send_message(admin_id, '下载完成')


@bot.on(events.CallbackQuery())
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
async def bot_message_handler(event):
    try:
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
            download_args = event.raw_text.split()
            min_id = max(int(download_args[1]), 1) if len(download_args) > 1 else 1
            max_id = int(download_args[2]) if len(download_args) > 2 else 1 << 31 - 1
            await event.respond('开始下载历史记录')
            if len(download_args) == 0:
                indexer.clear()
            await download_history(min_id=min_id, max_id=max_id)

        else:
            q = event.raw_text
            result = indexer.search(q, page_len=page_len, page_num=1)
            used_time = time() - start_time
            respond = render_respond_text(result, used_time, is_private)
            buttons = render_respond_buttons(result, 1)
            msg = await event.respond(respond, parse_mode='html', buttons=buttons)

            db.set('msg-' + str(msg.id) + '-q', q)
    except Exception as e:
        print(str(e))
        await event.reply(str(e))


#################################################################################
# Prepare main loop
#################################################################################


async def init_bot():
    # put some async initialization actions here
    for chat_id in chat_ids:
        entity = await client.get_entity(chat_id)
        id_to_title[chat_id] = format_entity_name(entity)
    logger.info('Bot started')
    await bot.send_message(admin_id, 'I am ready. ')


loop.run_until_complete(init_bot())

try:
    loop.run_forever()
except KeyboardInterrupt:
    pass
