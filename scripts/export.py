from pathlib import Path
from html import unescape

from tg_searcher.indexer import Indexer

def main():
    indexer = Indexer(Path("/home/sharzy/geist_idx"))
    with indexer.ix.reader() as reader, open("export", 'w') as output:
        all_doc = [doc for _, doc in reader.iter_docs()]
        all_doc.sort(key=lambda d: d["post_time"])
        for doc in all_doc:
            print(f'[{doc["post_time"]}]\n{unescape(doc["content"])}\n', file=output)


if __name__ == '__main__':
    main()
