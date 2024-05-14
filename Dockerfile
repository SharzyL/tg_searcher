ARG PYTHON_BASE=3.9-slim
FROM python:$PYTHON_BASE AS builder

# install PDM
RUN pip install -U pdm
ENV PDM_CHECK_UPDATE=false
COPY pyproject.toml pdm.lock README.md /app/
COPY tg_searcher /app/tg_searcher

# install dependencies and project into the local packages directory
WORKDIR /app
RUN pdm install --check --prod --no-editable

# run stage
FROM python:$PYTHON_BASE

# retrieve packages from build stage
COPY --from=builder /app/.venv/ /app/.venv
ENV PATH="/app/.venv/bin:$PATH"
COPY tg_searcher /app/tg_searcher
WORKDIR /app
ENTRYPOINT ["python", "tg_searcher/__main__.py"]
CMD ["-f", "./config/searcher.yaml"]

