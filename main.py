#!/usr/bin/python3.8
import asyncio
import html
import logging
import os
import sys
from pathlib import Path
from time import time
from typing import List
import argparse

import redis
import yaml
from telethon import TelegramClient, events, Button

from indexer import Indexer

os.chdir(Path(sys.argv[0]).parent)


#################################################################################
# Read arguments and configurations
#################################################################################


parser = argparse.ArgumentParser(description='A server to provide Telegram message searching')
parser.add_argument('-c', '--clear', action='store_const', const=True, default=False,
                    help='Build a new index from the scratch')
parser.add_argument('-f', '--config', action='store', default='searcher.yaml',
                    help='Specify where the configuraion yaml file lies')

args = parser.parse_args()
will_clear = args.clear
config_path = args.config


with open('searcher.yaml', 'r') as fp:
    config = yaml.safe_load(fp)
redis_host: str = config['redis']['host']
redis_port: str = config['redis']['port']
api_id: int = config['telegram']['api_id']
api_hash: str = config['telegram']['api_hash']
bot_token: str = config['telegram']['bot_token']
admin_id: int = config['telegram']['admin_id']
chat_ids: List[int] = config['chat_id']
log_path: str = config['log_path']
page_len: int = config['search']['page_len']
welcome_message: str = config['welcome_message']


#################################################################################
# Prepare loggers, client connections, and 
#################################################################################


def get_logger(name: str, _log_path: str, level=logging.INFO):
    log_fmt = logging.Formatter(
        f"%(asctime)s - %(levelname)s: {name}: %(message)s",
        "%Y %b %d %H:%M:%S"
    )
    _logger = logging.getLogger(name)
    _logger.setLevel(level)
    fh = logging.FileHandler(f'{_log_path}', encoding='utf8')
    fh.setLevel(level)
    fh.setFormatter(log_fmt)
    _logger.addHandler(fh)
    return _logger


logger = get_logger('bot', log_path, level=logging.INFO)
logger.info('Bot is activated')

indexer = Indexer(from_scratch=will_clear)

db = redis.Redis(host=redis_host, port=redis_port, decode_responses=True)

loop = asyncio.get_event_loop()

client = TelegramClient('session/client', api_id, api_hash, loop=loop)
client.start()

bot = TelegramClient('session/bot', api_id, api_hash, loop=loop)
bot.start(bot_token=bot_token)

id_to_title = dict()  # a dictionary to translate chat id to chat title


#################################################################################
# Handle message from the channel
#################################################################################


def strip_content(content: str) -> str:
    return html.escape(content).replace('\n', ' ')


def get_share_id(chat_id: int) -> int:
    return chat_id if chat_id >= 0 else - chat_id - 1000000000000


@client.on(events.NewMessage(from_users=chat_ids))
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


@client.on(events.MessageEdited(from_users=chat_ids))
async def client_message_update_handler(event):
    if event.raw_text and len(event.raw_text.strip()) >= 0:
        share_id = get_share_id(event.chat_id)
        url = f'https://t.me/c/{share_id}/{event.id}'
        logger.info(f'Update message {url}')
        indexer.update(url=url, content=strip_content(event.raw_text))


@client.on(events.MessageDeleted)
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


def render_respond_text(result, used_time):
    respond = f'共搜索到 {result["total"]} 个结果，用时 {used_time: .3} 秒：\n\n'
    for hit in result['hits']:
        respond += f'<b>{ id_to_title[hit["chat_id"]] } [{ hit["post_time"] }]</b>\n'
        respond += f'<a href="{ hit["url"] }">{ hit["highlighted"] }</a>\n'
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


async def download_history():
    for chat_id in chat_ids:
        await bot.send_message(admin_id, f'开始下载 {id_to_title[chat_id]} 的历史记录')
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


@bot.on(events.CallbackQuery)
async def bot_callback_handler(event):
    if event.data and event.data != b'-1':
        page_num = int(event.data)
        q = db.get('msg-' + str(event.message_id) + '-q')
        if q:
            start_time = time()
            result = indexer.search(q, page_len=page_len, page_num=page_num)
            used_time = time() - start_time
            respond = render_respond_text(result, used_time)
            buttons = render_respond_buttons(result, page_num)
            await event.edit(respond, parse_mode='html', buttons=buttons)
    await event.answer()


@bot.on(events.NewMessage)
async def bot_message_handler(event):
    text = event.raw_text
    logger.info(f'User {event.from_id} Queries [{text}]')

    if not (event.raw_text and event.raw_text.strip()):
        return

    elif event.raw_text.startswith('/start'):
        await event.respond(welcome_message, parse_mode='markdown')

    elif event.raw_text.startswith('/download_history') and event.chat_id == admin_id:
        await event.respond('开始下载历史记录', parse_mode='markdown')
        indexer.clear()
        await download_history()
        await event.respond('下载完成', parse_mode='markdown')

    else:
        start_time = time()
        q = event.raw_text
        result = indexer.search(q, page_len=page_len, page_num=1)
        used_time = time() - start_time
        respond = render_respond_text(result, used_time)
        buttons = render_respond_buttons(result, 1)
        msg = await event.respond(respond, parse_mode='html', buttons=buttons)

        db.set('msg-' + str(msg.id) + '-q', q)


#################################################################################
# Prepare main loop
#################################################################################


async def init_bot():
    # put some async initilization actions here
    for chat_id in chat_ids:
        entity = await client.get_entity(chat_id)
        id_to_title[chat_id] = entity.title
    await bot.send_message(admin_id, 'I am ready. ')

loop.run_until_complete(init_bot())

try:
    loop.run_forever()
except KeyboardInterrupt:
    pass
