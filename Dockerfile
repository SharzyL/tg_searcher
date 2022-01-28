FROM python:3.9 AS BUILDER
# Because cryptg builds some native library
# use multi-stage build reduce image size

WORKDIR /app

COPY . /app

RUN pip install \
    --no-cache-dir \
    --trusted-host pypi.python.org \
    --use-feature=in-tree-build \
    --disable-pip-version-check \
    /app

FROM python:3.9-slim

RUN mkdir /usr/local/lib/python3.9 -p
COPY --from=BUILDER \
    /usr/local/lib/python3.9/site-packages \
    /usr/local/lib/python3.9/site-packages

ENTRYPOINT ["python", "-m", "tg_searcher"]
CMD ["-f", "./config/searcher.yaml"]
