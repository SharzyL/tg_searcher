from pathlib import Path
from datetime import datetime
import random
from typing import Optional, Union, List, Set

from whoosh import index
from whoosh.fields import Schema, TEXT, ID, DATETIME
from whoosh.qparser import QueryParser
from whoosh.writing import IndexWriter
from whoosh.query import Term, Or
import whoosh.highlight as highlight
from jieba.analyse.analyzer import ChineseAnalyzer


class IndexMsg:
    schema = Schema(
        content=TEXT(stored=True, analyzer=ChineseAnalyzer()),
        url=ID(stored=True, unique=True),
        # for `chat_id` we are using TEXT instead of NUMERIC here, because NUMERIC
        # do not support iterating all values of the field
        chat_id=TEXT(stored=True),
        post_time=DATETIME(stored=True, sortable=True),
        sender=TEXT(stored=True),
    )

    def __init__(self, content: str, url: str, chat_id: Union[int, str], post_time: datetime, sender: str):
        self.content = content
        self.url = url
        self.chat_id = int(chat_id)
        self.post_time = post_time
        self.sender = sender

    def as_dict(self):
        return {
            'content': self.content,
            'url': self.url,
            'chat_id': str(self.chat_id),
            'post_time': self.post_time,
            'sender': self.sender
        }

    def __str__(self):
        return f'IndexMsg' + ', '.join(f'{k}={repr(v)}' for k, v in self.as_dict().items())


class SearchHit:
    def __init__(self, msg: IndexMsg, highlighted: str):
        self.msg = msg
        self.highlighted = highlighted

    def __str__(self):
        return f'SearchHit(highlighted={repr(self.highlighted)}, msg={self.msg})'


class SearchResult:
    def __init__(self, hits: List[SearchHit], is_last_page: bool, total_results: int):
        self.hits = hits
        self.is_last_page = is_last_page
        self.total_results = total_results


class Indexer:
    # A wrapper of whoosh

    def __init__(self, index_dir: Path, from_scratch: bool = False):
        index_name = 'index'
        if not Path(index_dir).exists():
            Path(index_dir).mkdir()

        def _clear():
            import shutil
            shutil.rmtree(index_dir)
            index_dir.mkdir()
            self.ix = index.create_in(index_dir, IndexMsg.schema, index_name)

        if from_scratch:
            _clear()

        self.ix = index.open_dir(index_dir, index_name) \
            if index.exists_in(index_dir, index_name) \
            else index.create_in(index_dir, IndexMsg.schema, index_name)

        assert repr(self.ix.schema.names) == repr(IndexMsg.schema.names), \
            f"Incompatible schema in your index '{index_dir}'\n" \
            f"\tExpected: {IndexMsg.schema}\n" \
            f"\tOn disk:  {self.ix.schema}"

        self._clear = _clear  # use closure to avoid introducing too much members
        self.query_parser = QueryParser('content', IndexMsg.schema)
        self.highlighter = highlight.Highlighter()

    def retrieve_random_document(self) -> IndexMsg:
        with self.ix.searcher() as searcher:
            msg_dict = random.choice(list(searcher.documents()))
            return IndexMsg(**msg_dict)

    def add_document(self, message: IndexMsg, writer: Optional[IndexWriter] = None):
        if writer is not None:
            writer.add_document(**message.as_dict())
        else:
            with self.ix.writer() as writer:
                writer.add_document(**message.as_dict())

    def search(self, q_str: str, in_chats: Optional[List[int]], page_len: int, page_num: int = 1) -> SearchResult:
        q = self.query_parser.parse(q_str)
        with self.ix.searcher() as searcher:
            q_filter = in_chats and Or([Term('chat_id', str(chat_id)) for chat_id in in_chats])
            result_page = searcher.search_page(q, page_num, page_len, filter=q_filter,
                                               sortedby='post_time', reverse=True)

            hits = [SearchHit(IndexMsg(**msg), self.highlighter.highlight_hit(msg, 'content'))
                    for msg in result_page]
            return SearchResult(hits, result_page.is_last_page(), result_page.total)

    def list_indexed_chats(self) -> Set[int]:
        with self.ix.reader() as r:
            return set(int(chat_id) for chat_id in r.field_terms('chat_id'))

    def count_by_query(self, **kw):
        with self.ix.searcher() as s:
            return len(list(s.document_numbers(**kw)))

    def delete(self, url: str):
        with self.ix.writer() as writer:
            writer.delete_by_term('url', url)

    def update(self, content: str, url: str):
        with self.ix.searcher() as searcher:
            msg_dict = searcher.document(url=url)
            if msg_dict:
                with self.ix.writer() as writer:
                    msg_dict['content'] = content
                    writer.update_document(**msg_dict)

    def clear(self):
        self._clear()
