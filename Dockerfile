FROM python:3.9-slim AS builder

WORKDIR /app

# initialize venv
RUN python -m venv /app/venv

# activate venv
ENV PATH="/app/venv/bin:$PATH"

# upgrade venv deps
RUN pip install --trusted-host pypi.python.org --upgrade pip setuptools wheel

# copy requirements.txt first to enable build caching
COPY requirements.txt /app

# install deps
RUN pip install --trusted-host pypi.python.org -r /app/requirements.txt

COPY . /app

#----------------------------------------

FROM python:3.9-slim

WORKDIR /app

COPY --from=builder /app /app

# activate venv
ENV PATH="/app/venv/bin:$PATH"

ENTRYPOINT ["python", "-u", "main.py"]
CMD ["-f", "./config/searcher.yaml"]
