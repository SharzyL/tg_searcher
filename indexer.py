from whoosh.index import create_in, exists_in, open_dir
from whoosh.fields import *
from whoosh.qparser import QueryParser
import whoosh.highlight as highlight
from pathlib import Path
import os
import re
from datetime import datetime
from jieba.analyse.analyzer import ChineseAnalyzer

import random

class Indexer:
    # A wrapper of whoosh

    def __init__(self, pickle_path='index', index_name='telegram_searcher', from_scratch=False):
        analyzer = ChineseAnalyzer()
        schema = Schema(
            content=TEXT(stored=True, analyzer=analyzer),
            url=ID(stored=True, unique=True),
            chat_id=STORED(),
            post_time=DATETIME(stored=True, sortable=True),
        )

        if not Path(pickle_path).exists():
            Path(pickle_path).mkdir()

        def _clear():
            pattern = re.compile(f'^_?{index_name}.*')
            for file in Path(pickle_path).iterdir():
                if pattern.match(file.name):
                    os.remove(str(file))
            self.ix = create_in(pickle_path, schema, index_name)

        if from_scratch:
            _clear()

        self.ix = open_dir(pickle_path, index_name) \
            if exists_in(pickle_path, index_name) \
            else create_in(pickle_path, schema, index_name)

        self._clear = _clear  # use closure to avoid introducing to much members
        self.query_parser = QueryParser('content', schema)
        self.highlighter = highlight.Highlighter()

    def index(self, content: str, url: str, chat_id: int, post_timestamp: int):
        with self.ix.writer() as writer:
            writer.add_document(
                content=content,
                url=url,
                chat_id=chat_id,
                post_time=datetime.fromtimestamp(post_timestamp)
            )

    def retrieve_random_document(self):
        with self.ix.searcher() as searcher:
            return random.choice(list(searcher.documents()))

    def search(self, query: str, page_len, page_num=1):
        q = self.query_parser.parse(query)
        with self.ix.searcher() as searcher:
            result_page = searcher.search_page(q, page_num, page_len,
                                               sortedby='post_time', reverse=True)

            return {
                'total': len(result_page),
                'is_last_page': result_page.is_last_page(),
                'hits': [{
                    'highlighted': self.highlighter.highlight_hit(hit, 'content'),
                    'url': hit['url'],
                    'chat_id': hit['chat_id'],
                    'post_time': hit['post_time']
                } for hit in result_page]
            }

    def delete(self, url: str):
        with self.ix.writer() as writer:
            writer.delete_by_term('url', url)

    def update(self, content: str, url: str):
        with self.ix.searcher() as searcher:
            document = searcher.document(url=url)
        with self.ix.writer() as writer:
            writer.update_document(
                content=content,
                url=url,
                chat_id=document['chat_id'],
                post_time=document['post_time'],
            )

    def clear(self):
        self._clear()
