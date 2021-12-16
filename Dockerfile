FROM python:3.9-slim

WORKDIR /app

# copy requirements.txt first to enable build caching
COPY requirements.txt /app

# install deps
RUN pip install --no-cache-dir --trusted-host pypi.python.org -r /app/requirements.txt

COPY . /app

ENTRYPOINT ["python", "-u", "main.py"]
CMD ["-f", "./config/searcher.yaml"]
